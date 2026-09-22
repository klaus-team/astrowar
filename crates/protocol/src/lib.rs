use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_PLAYERS: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    Competitive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameDurationMinutes {
    Five = 5,
    Ten = 10,
    Fifteen = 15,
}

impl GameDurationMinutes {
    pub const ALL: [Self; 3] = [Self::Five, Self::Ten, Self::Fifteen];

    pub fn as_minutes(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomPhase {
    Lobby,
    Playing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: String,
    pub nickname: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub owner_id: String,
    pub phase: RoomPhase,
    pub mode: GameMode,
    pub duration_minutes: GameDurationMinutes,
    pub players: Vec<PlayerInfo>,
    pub max_players: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        protocol_version: u16,
    },
    CreateRoom {
        nickname: String,
        mode: GameMode,
        duration_minutes: GameDurationMinutes,
    },
    JoinRoom {
        code: String,
        nickname: String,
    },
    StartGame,
    Leave,
    Relay {
        payload: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        protocol_version: u16,
        player_id: String,
    },
    Error {
        message: String,
    },
    RoomUpdated {
        room: RoomInfo,
    },
    GameStarted {
        room: RoomInfo,
    },
    PlayerLeft {
        player_id: String,
        nickname: String,
        forfeited: bool,
    },
    Relayed {
        from_player_id: String,
        payload: Vec<u8>,
    },
}
