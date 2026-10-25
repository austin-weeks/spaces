use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{channel::WsID, location::Location};

pub struct UserRegistry(HashMap<WsID, UserState>);
impl UserRegistry {
    pub fn new() -> Self {
        UserRegistry(HashMap::new())
    }

    pub fn register_user(&mut self, id: WsID) {
        self.0.insert(
            id,
            UserState {
                location: None,
                mouse_pos: MouseState { x: 0.0, y: 0.0 },
            },
        );
    }

    pub fn remove_user(&mut self, id: WsID) {
        self.0.remove(&id);
    }

    pub fn update_user_mouse(&mut self, id: WsID, mouse_pos: MouseState) {
        self.0
            .entry(id)
            .and_modify(|user| user.mouse_pos = mouse_pos);
    }

    /// Returns true if the user is still in the registry or false if not found.
    pub fn add_user_location(&mut self, id: WsID, location: Arc<Location>) -> bool {
        if let Some(user) = self.0.get_mut(&id) {
            user.location = Some(location);
            true
        } else {
            false
        }
    }

    pub fn users_snapshot(&self) -> Arc<[UserStateWithID]> {
        self.0
            .iter()
            .map(|(k, v)| UserStateWithID {
                id: *k,
                state: v.clone(),
            })
            .collect()
    }
}

/// Mouse x,y position in range [0, 1].
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct MouseState {
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserState {
    pub location: Option<Arc<Location>>,
    pub mouse_pos: MouseState,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserStateWithID {
    pub id: WsID,
    pub state: UserState,
}
