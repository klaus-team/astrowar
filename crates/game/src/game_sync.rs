use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameMessage {
    Input {
        seq: u32,
        move_x: i8,
        move_y: i8,
        fire: bool,
        x: f32,
        y: f32,
    },
    State {
        tick: u32,
        time_left_secs: u32,
        match_over: bool,
        ships: Vec<ShipState>,
        bullets: Vec<BulletState>,
        asteroids: Vec<AsteroidState>,
    },
    RequestSnapshot,
    /// Client-reported asteroid destroy (idea-2: each client sims hits locally).
    Hit {
        asteroid_id: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipState {
    pub player_id: String,
    pub nickname: String,
    pub x: f32,
    pub y: f32,
    pub move_x: i8,
    pub move_y: i8,
    pub last_input_seq: u32,
    pub score: i32,
    pub lives: u8,
    pub alive: bool,
    pub forfeited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletState {
    pub id: u32,
    pub owner_id: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsteroidState {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub vy: f32,
    pub radius: f32,
}

impl GameMessage {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
