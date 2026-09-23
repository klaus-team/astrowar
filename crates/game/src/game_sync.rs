use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameMessage {
    Input { move_x: i8, move_y: i8 },
    State { tick: u32, ships: Vec<ShipState> },
    RequestSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipState {
    pub player_id: String,
    pub nickname: String,
    pub x: f32,
    pub y: f32,
}

impl GameMessage {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
