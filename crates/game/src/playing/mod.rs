//! Gameplay simulation, prediction, and world presentation.

mod host;
mod phase;
mod prediction;
mod sync;
mod types;
mod world;

use crate::game_sync::GameMessage;
use crate::net_bridge::{NetBridge, NetCommand};
use crate::Session;

pub use host::{
    begin_host_sim, handle_relayed_game_message, mark_player_forfeit, playing_host_simulate,
    seed_ships_from_room,
};
pub use prediction::{playing_client_local_hits, playing_predict_local, playing_send_input};
pub use sync::{advance_interpolation, snapshot};
pub use types::{HostSim, HurtFlash, InputThrottle, LatestState, LocalPrediction, MatchOverReturn};
pub use world::{
    cleanup_playing, maybe_record_high_score, record_local_high_score, spawn_playing_hud,
    sync_world_sprites, watch_hurt_flash,
};

pub fn send_game(bridge: &NetBridge, session: &Session, message: &GameMessage) {
    if session.solo {
        return;
    }
    if let Ok(payload) = message.to_bytes() {
        bridge.send(NetCommand::Relay { payload });
    }
}
