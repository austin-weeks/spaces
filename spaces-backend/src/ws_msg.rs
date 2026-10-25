use std::{borrow::Cow, sync::Arc};

use axum::{
    body::Bytes,
    extract::ws::{Message, Utf8Bytes},
};
use serde::{Deserialize, Serialize};

use crate::{
    channel::WsID,
    location::Location,
    user_state::{MouseState, UserStateWithID},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMsg {
    AllUsersState { users: Arc<[UserStateWithID]> },
    NewUser { id: WsID },
    Connected { id: WsID },
    Disconnect { id: WsID },
    MouseMove { id: WsID, mouse: MouseState },
    ClientMouseMove { mouse: MouseState },
    LocationFound { id: WsID, loc: Arc<Location> },
    Error { msg: Cow<'static, str> },
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
        const SEND_BINARY: bool = true;

        if SEND_BINARY {
            let bytes = match rmp_serde::to_vec_named(self) {
                Ok(b) => b,
                Err(e) => {
                    return Err(WsSeralizeErr::Webpack(e));
                }
            };
            Ok(Message::Binary(Bytes::from(bytes)))
        } else {
            let json = match serde_json::to_string(self) {
                Ok(b) => b,
                Err(e) => {
                    return Err(WsSeralizeErr::Json(e));
                }
            };
            Ok(Message::Text(Utf8Bytes::from(json)))
        }
    }
}
