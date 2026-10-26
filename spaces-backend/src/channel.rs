use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, close_code};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::sync::{
    Mutex,
    mpsc::{self, Receiver, Sender},
};

use crate::{location::Location, user_state::UserRegistry, ws_msg::WsMsg};

/// Represents the ID of a websocket connection.
pub type WsID = usize;

#[derive(Clone)]
struct WsConn {
    sender: Sender<Message>,
}
impl WsConn {
    fn new(sender: Sender<Message>) -> Self {
        Self { sender }
    }

    // FIXME: might be smart to buffer these messages and send every X seconds
    // currently this flushes on every send which is apparently a syscall
    fn send_msg(&self, msg: Message) {
        // Best effort send, write loop handles closing the connection when full
        _ = self.sender.try_send(msg);
    }
}

struct Sockets {
    map: HashMap<WsID, WsConn>,
    id_cursor: usize,
}

pub struct Channel {
    sockets: Mutex<Sockets>,
    state: Mutex<UserRegistry>,
}

impl Channel {
    const MSG_BUF_SIZE: usize = 200; // we'll close connections if this gets full

    pub fn new() -> Self {
        Self {
            sockets: Mutex::new(Sockets {
                map: HashMap::new(),
                id_cursor: 0,
            }),
            state: Mutex::new(UserRegistry::new()),
        }
    }

    pub async fn register_socket(self: Arc<Self>, ws: WebSocket, is_mobile_conn: bool) -> WsID {
        let id = {
            let mut sockets = self.sockets.lock().await;
            let id = sockets.id_cursor;
            sockets.id_cursor = sockets.id_cursor.wrapping_add(1);
            id
        };

        let (sink, stream) = ws.split();
        let (tx, rx) = mpsc::channel::<Message>(Channel::MSG_BUF_SIZE);

        let channel_clone = self.clone();
        // Start message write loop
        tokio::spawn(async move {
            let closure = ws_write_loop(sink, rx).await;
            if closure.is_err() {
                channel_clone.unregister_socket(id, closure).await;
            }
        });

        // Insert into map and handle case of duplicate ID
        if let Some(old) = self.sockets.lock().await.map.insert(id, WsConn::new(tx)) {
            old.send_msg(Message::Close(Some(CloseFrame {
                code: close_code::ERROR,
                reason: Utf8Bytes::from_static("received new connection with same id"),
            })));
            tracing::error!(
                ws_id = id,
                "ws ID already in socket map - closed the old socket",
            );
        }

        // NOTE: intentionally holding the state lock across all these steps
        // to ensure we enqueue the initial messages to the client in the correct order
        // and that any disconnects or updates received from other connections come after
        let user_state = {
            let mut state = self.state.lock().await;
            // Add user to state
            let user_state = state.register_user(id, is_mobile_conn);

            // Notify user of their ID
            self.send_to_user(id, WsMsg::Connected { id }).await;

            // Send current state to new user
            let users = state.users_snapshot();
            self.send_to_user(id, WsMsg::AllUsersState { users }).await;
            user_state
        };

        // Notify channel of new user
        self.broadcast(WsMsg::NewUser {
            id,
            state: user_state,
        })
        .await;

        // Start socket read loop
        // NOTE: HAVE do this last to ensure that we listen for socket closure AFTER
        // we've already added the socket, otherwise this would create a race condition
        // where a quickly closed socket would unregister first, then register, which creates
        // a ghost socket that never gets cleaned up - bad.
        let channel_clone = self.clone();
        tokio::spawn(async move {
            let closure = ws_read_loop(stream, channel_clone.clone(), id).await;
            tracing::debug!(ws_id = id, "dropping socket");
            channel_clone.unregister_socket(id, closure).await;
        });

        id
    }

    async fn unregister_socket(&self, id: WsID, closure: Result<(), &'static str>) {
        // If not already removed
        let removed = self.sockets.lock().await.map.remove(&id);
        // Do this in two steps to ensure the mutex lock is dropped ^
        if let Some(send) = removed {
            send.send_msg(Message::Close(Some(match closure {
                Ok(_) => CloseFrame {
                    code: close_code::NORMAL,
                    reason: Utf8Bytes::default(),
                },
                Err(e) => CloseFrame {
                    code: close_code::ERROR,
                    reason: Utf8Bytes::from_static(e),
                },
            })));
            drop(send);
            self.state.lock().await.remove_user(id);
            self.broadcast(WsMsg::Disconnect { id }).await;
        }
    }

    pub async fn add_location(&self, id: WsID, location: Location) {
        let location = Arc::new(location);
        let user_stil_connected = self
            .state
            .lock()
            .await
            .add_user_location(id, location.clone());
        if user_stil_connected {
            self.broadcast(WsMsg::LocationFound { id, loc: location })
                .await;
        }
    }

    async fn handle_user_message(&self, id: WsID, msg: WsMsg) {
        if let WsMsg::ClientMouseMove { mouse } = msg {
            self.state.lock().await.update_user_mouse(id, mouse);
            self.broadcast(WsMsg::MouseMove { id, mouse }).await;
        } else {
            tracing::error!(msg = ?msg, "client sent non client-mouse-move message");
            self.send_to_user(
                id,
                WsMsg::Error {
                    msg: "client must not send message types other that mouse_move".into(),
                },
            )
            .await;
        }
    }

    // The client will need to remember to filter out messages about itself. I'm not worried about
    // that...
    async fn broadcast(&self, msg: WsMsg) {
        let sockets = self
            .sockets
            .lock()
            .await
            .map
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let frame = match msg.as_ws_frame() {
            Ok(f) => f,
            Err(e) => {
                tracing::error!(error = ?e, "failed to serialize message");
                return;
            }
        };
        for socket in sockets {
            // Message is cheap to clone as it relies on Bytes type
            socket.send_msg(frame.clone());
        }
    }

    async fn send_to_user(&self, id: WsID, msg: WsMsg) {
        let Some(ws) = self.sockets.lock().await.map.get(&id).cloned() else {
            tracing::error!(ws_id = id, message = ?msg, "sending to non-existing socket");
            return;
        };
        let frame = match msg.as_ws_frame() {
            Ok(f) => f,
            Err(e) => {
                tracing::error!(error = ?e, "failed to serialize message");
                return;
            }
        };
        ws.send_msg(frame);
    }
}

async fn ws_write_loop(
    mut ws_sink: SplitSink<WebSocket, Message>,
    mut rx: Receiver<Message>,
) -> Result<(), &'static str> {
    async fn send_close_frame(ws_sink: &mut SplitSink<WebSocket, Message>, e: &'static str) {
        // Try to send the close frame
        // if it times out, oh well
        _ = tokio::time::timeout(
            Duration::from_secs(5),
            ws_sink.send(Message::Close(Some(CloseFrame {
                code: close_code::POLICY,
                reason: Utf8Bytes::from_static(e),
            }))),
        )
        .await;
    }

    while let Some(msg) = rx.recv().await {
        if rx.capacity() <= 1 {
            let reason = "client too slow";
            send_close_frame(&mut ws_sink, reason).await;
            return Err(reason);
        }
        if tokio::time::timeout(Duration::from_secs(5), ws_sink.send(msg))
            .await
            .is_err()
        {
            let reason = "client timed out";
            send_close_frame(&mut ws_sink, reason).await;
            return Err(reason);
        }
    }
    Ok(())
}

async fn ws_read_loop(
    mut ws_stream: SplitStream<WebSocket>,
    channel: Arc<Channel>,
    id: WsID,
) -> Result<(), &'static str> {
    while let Some(frame) = ws_stream.next().await {
        let Ok(frame) = frame else {
            tracing::warn!(id = id, "socket disconnected");
            return Ok(());
        };
        tracing::debug!(message = ?frame, "received message frame");

        match frame {
            Message::Text(utf8_bytes) => {
                match serde_json::from_slice::<WsMsg>(utf8_bytes.as_bytes()) {
                    Ok(msg) => channel.handle_user_message(id, msg).await,
                    Err(e) => {
                        tracing::error!(error = ?e,"deserialize socket text frame");
                        channel
                            .send_to_user(
                                id,
                                WsMsg::Error {
                                    msg: format!("failed to deserialize json message: {e}").into(),
                                },
                            )
                            .await;
                        return Err("invalid text message");
                    }
                };
            }
            Message::Binary(bytes) => match rmp_serde::from_slice::<WsMsg>(&bytes) {
                Ok(msg) => channel.handle_user_message(id, msg).await,
                Err(e) => {
                    tracing::error!(error = ?e, "deserialize socket binary frame");
                    channel
                        .send_to_user(
                            id,
                            WsMsg::Error {
                                msg: format!("failed to deserialize messagepack message: {e}")
                                    .into(),
                            },
                        )
                        .await;
                    return Err("invalid binary message");
                }
            },
            Message::Ping(_) | Message::Pong(_) => (), // server handles automatically
            Message::Close(_) => return Ok(()),
        }
    }
    Ok(())
}
