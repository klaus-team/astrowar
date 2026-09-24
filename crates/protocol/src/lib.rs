use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 2;
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
    /// Solo-only: play until out of lives (no time limit).
    Endless = 0,
}

impl GameDurationMinutes {
    /// Timed matches (host / online rooms).
    pub const ALL: [Self; 3] = [Self::Five, Self::Ten, Self::Fifteen];
    /// Solo setup options, including endless (first = default).
    pub const SOLO: [Self; 4] = [Self::Endless, Self::Five, Self::Ten, Self::Fifteen];

    pub fn as_minutes(self) -> u8 {
        self as u8
    }

    pub fn is_endless(self) -> bool {
        matches!(self, Self::Endless)
    }

    pub fn label(self) -> String {
        match self {
            Self::Endless => "Until out of lives".into(),
            other => format!("{} min", other.as_minutes()),
        }
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
    /// Game payloads prefer WebSocket binary frames (see `encode_client_relay`).
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
    /// Game payloads prefer WebSocket binary frames (see `encode_server_relayed`).
    Relayed {
        from_player_id: String,
        payload: Vec<u8>,
    },
}

/// Client → server binary frame: raw game payload bytes.
pub fn encode_client_relay(payload: &[u8]) -> Vec<u8> {
    payload.to_vec()
}

pub fn decode_client_relay(bytes: &[u8]) -> Vec<u8> {
    bytes.to_vec()
}

/// Server → client binary frame: `[u8 id_len][id utf8][payload...]`.
pub fn encode_server_relayed(
    from_player_id: &str,
    payload: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let id = from_player_id.as_bytes();
    if id.len() > u8::MAX as usize {
        return Err("from_player_id too long");
    }
    let mut out = Vec::with_capacity(1 + id.len() + payload.len());
    out.push(id.len() as u8);
    out.extend_from_slice(id);
    out.extend_from_slice(payload);
    Ok(out)
}

pub fn decode_server_relayed(bytes: &[u8]) -> Result<(String, Vec<u8>), &'static str> {
    if bytes.is_empty() {
        return Err("empty relayed frame");
    }
    let id_len = bytes[0] as usize;
    let id_end = 1 + id_len;
    if bytes.len() < id_end {
        return Err("truncated relayed frame");
    }
    let from_player_id = std::str::from_utf8(&bytes[1..id_end])
        .map_err(|_| "invalid from_player_id utf8")?
        .to_string();
    Ok((from_player_id, bytes[id_end..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relayed_frame_roundtrip() {
        let payload = b"{\"type\":\"input\"}".to_vec();
        let frame = encode_server_relayed("abc-123", &payload).unwrap();
        let (from, got) = decode_server_relayed(&frame).unwrap();
        assert_eq!(from, "abc-123");
        assert_eq!(got, payload);
    }
}
