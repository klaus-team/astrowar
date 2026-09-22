use protocol::{
    GameDurationMinutes, GameMode, PlayerInfo, RoomInfo, RoomPhase, MAX_PLAYERS,
};
use rand::Rng;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RoomError {
    #[error("nickname must not be empty")]
    EmptyNickname,
    #[error("room not found")]
    NotFound,
    #[error("room is full")]
    Full,
    #[error("nickname already taken in this room")]
    NicknameTaken,
    #[error("only the room owner can start the game")]
    NotOwner,
    #[error("already in a room")]
    AlreadyInRoom,
    #[error("player not in room")]
    NotInRoom,
}

#[derive(Debug)]
pub struct Room {
    pub info: RoomInfo,
    pub created_at: Instant,
}

impl Room {
    pub fn new(
        code: String,
        owner_id: String,
        nickname: String,
        mode: GameMode,
        duration_minutes: GameDurationMinutes,
    ) -> Self {
        Self {
            info: RoomInfo {
                code,
                owner_id: owner_id.clone(),
                phase: RoomPhase::Lobby,
                mode,
                duration_minutes,
                players: vec![PlayerInfo {
                    id: owner_id,
                    nickname,
                }],
                max_players: MAX_PLAYERS,
            },
            created_at: Instant::now(),
        }
    }
}

pub fn generate_room_code(length: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..ALPHABET.len());
            ALPHABET[idx] as char
        })
        .collect()
}

pub fn validate_nickname(nickname: &str) -> Result<String, RoomError> {
    let trimmed = nickname.trim();
    if trimmed.is_empty() || trimmed.len() > 24 {
        return Err(RoomError::EmptyNickname);
    }
    Ok(trimmed.to_string())
}

pub struct RoomStore {
    rooms: HashMap<String, Room>,
    player_to_room: HashMap<String, String>,
    code_length: usize,
    ttl: Duration,
}

impl RoomStore {
    pub fn new(code_length: usize, ttl_secs: u64) -> Self {
        Self {
            rooms: HashMap::new(),
            player_to_room: HashMap::new(),
            code_length: code_length.max(4),
            ttl: Duration::from_secs(ttl_secs.max(60)),
        }
    }

    pub fn purge_expired(&mut self) {
        let ttl = self.ttl;
        let expired: Vec<String> = self
            .rooms
            .iter()
            .filter(|(_, room)| room.created_at.elapsed() > ttl)
            .map(|(code, _)| code.clone())
            .collect();
        for code in expired {
            if let Some(room) = self.rooms.remove(&code) {
                for player in room.info.players {
                    self.player_to_room.remove(&player.id);
                }
            }
        }
    }

    pub fn create(
        &mut self,
        owner_id: String,
        nickname: String,
        mode: GameMode,
        duration_minutes: GameDurationMinutes,
    ) -> Result<RoomInfo, RoomError> {
        self.purge_expired();
        if self.player_to_room.contains_key(&owner_id) {
            return Err(RoomError::AlreadyInRoom);
        }
        let nickname = validate_nickname(&nickname)?;
        let mut code = generate_room_code(self.code_length);
        while self.rooms.contains_key(&code) {
            code = generate_room_code(self.code_length);
        }
        let room = Room::new(code.clone(), owner_id.clone(), nickname, mode, duration_minutes);
        let info = room.info.clone();
        self.rooms.insert(code.clone(), room);
        self.player_to_room.insert(owner_id, code);
        Ok(info)
    }

    pub fn join(
        &mut self,
        code: String,
        player_id: String,
        nickname: String,
    ) -> Result<RoomInfo, RoomError> {
        self.purge_expired();
        if self.player_to_room.contains_key(&player_id) {
            return Err(RoomError::AlreadyInRoom);
        }
        let nickname = validate_nickname(&nickname)?;
        let code = code.trim().to_uppercase();
        let room = self.rooms.get_mut(&code).ok_or(RoomError::NotFound)?;
        if room.info.players.len() as u8 >= room.info.max_players {
            return Err(RoomError::Full);
        }
        if room
            .info
            .players
            .iter()
            .any(|p| p.nickname.eq_ignore_ascii_case(&nickname))
        {
            return Err(RoomError::NicknameTaken);
        }
        room.info.players.push(PlayerInfo {
            id: player_id.clone(),
            nickname,
        });
        self.player_to_room.insert(player_id, code);
        Ok(room.info.clone())
    }

    pub fn start(&mut self, code: &str, player_id: &str) -> Result<RoomInfo, RoomError> {
        let room = self.rooms.get_mut(code).ok_or(RoomError::NotFound)?;
        if room.info.owner_id != player_id {
            return Err(RoomError::NotOwner);
        }
        room.info.phase = RoomPhase::Playing;
        Ok(room.info.clone())
    }

    pub fn leave(
        &mut self,
        code: &str,
        player_id: &str,
    ) -> Result<Option<(RoomInfo, PlayerInfo)>, RoomError> {
        let room = self.rooms.get_mut(code).ok_or(RoomError::NotFound)?;
        let Some(index) = room.info.players.iter().position(|p| p.id == player_id) else {
            return Err(RoomError::NotInRoom);
        };
        let left = room.info.players.remove(index);
        self.player_to_room.remove(player_id);

        if room.info.players.is_empty() {
            let info = room.info.clone();
            self.rooms.remove(code);
            return Ok(Some((info, left)));
        }

        if room.info.owner_id == player_id {
            room.info.owner_id = room.info.players[0].id.clone();
        }

        Ok(Some((room.info.clone(), left)))
    }

    pub fn player_ids(&self, code: &str) -> Result<Vec<String>, RoomError> {
        let room = self.rooms.get(code).ok_or(RoomError::NotFound)?;
        Ok(room.info.players.iter().map(|p| p.id.clone()).collect())
    }

    pub fn contains_player(&self, code: &str, player_id: &str) -> bool {
        self.rooms
            .get(code)
            .map(|room| room.info.players.iter().any(|p| p.id == player_id))
            .unwrap_or(false)
    }
}
