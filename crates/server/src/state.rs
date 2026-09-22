use crate::room::{RoomError, RoomStore};
use protocol::{
    GameDurationMinutes, GameMode, PlayerInfo, RoomInfo, ServerMessage,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub type ConnectionMap = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<ServerMessage>>>>;

#[derive(Clone)]
pub struct AppState {
    pub rooms: Arc<Mutex<RoomStore>>,
    pub connections: ConnectionMap,
}

impl AppState {
    pub fn new(room_code_length: usize, room_ttl_secs: u64) -> Self {
        Self {
            rooms: Arc::new(Mutex::new(RoomStore::new(room_code_length, room_ttl_secs))),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_room(
        &self,
        owner_id: String,
        nickname: String,
        mode: GameMode,
        duration_minutes: GameDurationMinutes,
    ) -> Result<RoomInfo, RoomError> {
        self.rooms
            .lock()
            .await
            .create(owner_id, nickname, mode, duration_minutes)
    }

    pub async fn join_room(
        &self,
        code: String,
        player_id: String,
        nickname: String,
    ) -> Result<RoomInfo, RoomError> {
        self.rooms.lock().await.join(code, player_id, nickname)
    }

    pub async fn start_game(&self, code: &str, player_id: &str) -> Result<RoomInfo, RoomError> {
        self.rooms.lock().await.start(code, player_id)
    }

    pub async fn leave_room(
        &self,
        code: &str,
        player_id: &str,
    ) -> Result<Option<(RoomInfo, PlayerInfo)>, RoomError> {
        self.rooms.lock().await.leave(code, player_id)
    }

    pub async fn relay(
        &self,
        code: &str,
        from_player_id: &str,
        payload: Vec<u8>,
    ) -> Result<(), RoomError> {
        let targets = {
            let rooms = self.rooms.lock().await;
            if !rooms.contains_player(code, from_player_id) {
                return Err(RoomError::NotInRoom);
            }
            rooms.player_ids(code)?
        };
        let connections = self.connections.lock().await;
        for target_id in targets {
            if target_id == from_player_id {
                continue;
            }
            if let Some(tx) = connections.get(&target_id) {
                let _ = tx.send(ServerMessage::Relayed {
                    from_player_id: from_player_id.to_string(),
                    payload: payload.clone(),
                });
            }
        }
        Ok(())
    }
}
