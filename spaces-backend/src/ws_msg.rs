use std::sync::Arc;

use axum::{body::Bytes, extract::ws::Message};
use serde::{Deserialize, Serialize};

use crate::{
    channel::WsID,
    location::Location,
    user_state::{MouseState, UserStateWithID},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMsg {
    AllUsersState {
        users: Arc<[UserStateWithID]>,
    },
    NewUser {
        user_id: WsID,
    },
    Disconnect {
        user_id: WsID,
    },
    MouseMove {
        user_id: WsID,
        mouse: MouseState,
    },
    LocationFound {
        user_id: WsID,
        location: Arc<Location>,
    },
    Error {
        user_id: WsID,
        message: String,
    },
}

#[derive(Debug)]
pub enum WsSeralizeErr {
    #[allow(unused)]
    Json(serde_json::Error),
    #[allow(unused)]
    Webpack(rmp_serde::encode::Error),
}
impl WsMsg {
    pub fn as_ws_frame(&self) -> Result<Message, WsSeralizeErr> {
        const SEND_BINARY: bool = false;

        if SEND_BINARY {
            let bytes = match rmp_serde::to_vec(self) {
                Ok(b) => b,
                Err(e) => {
                    return Err(WsSeralizeErr::Webpack(e));
                }
            };
            Ok(Message::Binary(Bytes::from(bytes)))
        } else {
            let bytes = match serde_json::to_vec(self) {
                Ok(b) => b,
                Err(e) => {
                    return Err(WsSeralizeErr::Json(e));
                }
            };
            Ok(Message::Binary(Bytes::from(bytes)))
        }
    }
}
