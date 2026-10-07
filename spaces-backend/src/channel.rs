use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, close_code};
use futures_util::{SinkExt, StreamExt, stream::SplitStream};
use tokio::sync::{
    Mutex,
    mpsc::{self, Sender},
};

use crate::{location::Location, user_state::UserRegistry, ws_msg::WsMsg};

/// Represents the ID of a websocket connection.
pub type WsID = usize;
type WsConn = Sender<Message>;

struct Sockets {
    map: HashMap<WsID, WsConn>,
    id_cursor: usize,
}

pub struct Channel {
    sockets: Mutex<Sockets>,
    state: Mutex<UserRegistry>,
}

impl Channel {
    pub fn new() -> Self {
        Self {
            sockets: Mutex::new(Sockets {
                map: HashMap::new(),
                id_cursor: 0,
            }),
            state: Mutex::new(UserRegistry::new()),
        }
    }

    pub async fn register_socket(self: Arc<Self>, ws: WebSocket) -> WsID {
        let id = {
            let mut sockets = self.sockets.lock().await;
            let id = sockets.id_cursor;
            sockets.id_cursor = sockets.id_cursor.wrapping_add(1);
            id
        };

        let (mut sink, stream) = ws.split();
        let (tx, mut rx) = mpsc::channel::<Message>(10);

        // Start read loop
        let channel_clone = self.clone();
        tokio::spawn(async move {
            let closure = ws_read_loop(stream, channel_clone.clone(), id).await;
            tracing::debug!(ws_id = id, "dropping socket");
            channel_clone.unregister_socket(id, closure).await;
        });
        // Start write loop
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                _ = sink.send(msg).await;
            }
        });

        // Insert into map and handle case of duplicate ID
        if let Some(old) = self.sockets.lock().await.map.insert(id, tx) {
            _ = old
                .send(Message::Close(Some(CloseFrame {
                    code: close_code::ERROR,
                    reason: Utf8Bytes::from_static("received new connection with same id"),
                })))
                .await;
            tracing::error!(
                ws_id = id,
                "ws ID already in socket map - closed the old socket",
            );
        }

        // Add user to state
        self.state.lock().await.register_user(id);

        // Send current state to new user
        let users = self.state.lock().await.users_snapshot();
        self.write_to_one(WsMsg::AllUsersState { users }, id).await;

        // Notify channel of new user
        self.send_message(WsMsg::NewUser { user_id: id }).await;

        id
    }

    async fn unregister_socket(&self, id: WsID, closure: Result<(), &'static str>) {
        // If not already removed
        let removed = self.sockets.lock().await.map.remove(&id);
        // Do this in two steps to ensure the mutex lock is dropped ^
        if let Some(send) = removed {
            _ = send
                .send(Message::Close(Some(match closure {
                    Ok(_) => CloseFrame {
                        code: close_code::NORMAL,
                        reason: Utf8Bytes::default(),
                    },
                    Err(e) => CloseFrame {
                        code: close_code::ERROR,
                        reason: Utf8Bytes::from_static(e),
                    },
                })))
                .await;
            drop(send);
            self.state.lock().await.remove_user(id);
            self.send_message(WsMsg::Disconnect { user_id: id }).await;
        }
    }

    pub async fn add_location(&self, id: WsID, location: Location) {
        let location = Arc::new(location);
        self.state
            .lock()
            .await
            .add_user_location(id, location.clone());
        self.write_broadcast(WsMsg::LocationFound {
            user_id: id,
            location,
        })
        .await;
    }

    async fn send_message(&self, msg: WsMsg) {
        match msg {
            WsMsg::NewUser { .. } | WsMsg::Disconnect { .. } => self.write_broadcast(msg).await,
            WsMsg::MouseMove { user_id: id, mouse } => {
                self.state.lock().await.update_user_mouse(id, mouse);
                self.write_broadcast(msg).await;
            }
            WsMsg::Error { user_id: id, .. } => self.write_to_one(msg, id).await,
            WsMsg::AllUsersState { .. } | WsMsg::LocationFound { .. } => {
                tracing::error!(msg = ?msg, "channel should not be sent a message of this type")
            }
        }
    }

    // The client will need to remember to filter out messages about itself. I'm not worried about
    // that...
    async fn write_broadcast(&self, msg: WsMsg) {
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
            // Message is cheap to clone as it relies on Bytes
            _ = socket.send(frame.clone()).await;
        }
    }

    async fn write_to_one(&self, msg: WsMsg, id: WsID) {
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
        _ = ws.send(frame).await;
    }
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
                    Ok(msg) => channel.send_message(msg).await,
                    Err(e) => {
                        tracing::error!(error = ?e,"deserialize socket text frame");
                        channel
                            .send_message(WsMsg::Error {
                                user_id: id,
                                message: format!("failed to deserialize json message: {e}"),
                            })
                            .await;
                        return Err("invalid text message");
                    }
                };
            }
            Message::Binary(bytes) => match rmp_serde::from_slice::<WsMsg>(&bytes) {
                Ok(msg) => channel.send_message(msg).await,
                Err(e) => {
                    tracing::error!(error = ?e, "deserialize socket binary frame");
                    channel
                        .send_message(WsMsg::Error {
                            user_id: id,
                            message: format!("failed to deserialize messagepack message: {e}"),
                        })
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
