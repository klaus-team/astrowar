use crate::game_sync::{AsteroidKind, AsteroidState, BulletState, ShipState};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) const SHIP_Y_MIN: f32 = -330.0;
pub(crate) const SHIP_SPEED: f32 = 280.0;
pub(crate) const BULLET_SPEED: f32 = 520.0;
pub(crate) const STATE_HZ: f32 = 20.0;
pub(crate) const INPUT_HZ: f32 = 30.0;
pub(crate) const FIRE_COOLDOWN: f32 = 0.22;
pub(crate) const ASTEROID_SPAWN_BASE: f32 = 1.1;
pub(crate) const HORIZON_Y: f32 = SHIP_Y_MIN - 24.0;
/// Visual dashed line ~½ ship-height above the resting shooter's top edge.
pub(crate) const SHIP_HEIGHT: f32 = 30.0;
pub(crate) const SHIP_REST_Y: f32 = SHIP_Y_MIN + 20.0;
pub(crate) const HORIZON_LINE_Y: f32 = SHIP_REST_Y + SHIP_HEIGHT; // top of ship + ½ block
pub(crate) const HORIZON_DASH_W: f32 = 16.0;
pub(crate) const HORIZON_DASH_GAP: f32 = 10.0;
pub(crate) const HORIZON_DASH_H: f32 = 2.0;
pub(crate) const PENDING_INPUT_CAP: usize = 180;
pub(crate) const STARTING_LIVES: u8 = 3;

#[derive(Component)]
pub struct ShipSprite {
    pub player_id: String,
}

#[derive(Component)]
pub struct BulletSprite {
    pub id: u32,
}

#[derive(Component)]
pub struct LocalBulletSprite {
    pub id: u32,
}

#[derive(Component)]
pub struct AsteroidSprite {
    pub id: u32,
    #[allow(dead_code)]
    pub kind: AsteroidKind,
}

#[derive(Component)]
pub struct PlayingHud;

#[derive(Component)]
pub struct HorizonDash;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudSlot {
    Title,
    Players,
    Mode,
    Leave,
}

#[derive(Resource, Default)]
pub struct HostSim {
    pub active: bool,
    pub tick: u32,
    pub time_left: f32,
    pub elapsed: f32,
    pub endless: bool,
    pub match_over: bool,
    pub ships: HashMap<String, SimShip>,
    pub bullets: Vec<SimBullet>,
    pub asteroids: Vec<SimAsteroid>,
    pub broadcast_accum: f32,
    pub spawn_accum: f32,
    pub next_bullet_id: u32,
    pub next_asteroid_id: u32,
    pub fire_cooldown: HashMap<String, f32>,
    pub pending_fire: HashMap<String, bool>,
    pub recent_destroyed: HashMap<u32, i32>,
}

#[derive(Clone)]
pub struct SimShip {
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

#[derive(Clone)]
pub struct SimBullet {
    pub id: u32,
    pub owner_id: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone)]
pub struct SimAsteroid {
    pub id: u32,
    pub kind: AsteroidKind,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub radius: f32,
    pub points: i32,
}

#[derive(Resource, Default)]
pub struct LatestState {
    pub tick: u32,
    pub time_left_secs: u32,
    pub match_over: bool,
    pub from_ships: Vec<ShipState>,
    pub to_ships: Vec<ShipState>,
    pub bullets: Vec<BulletState>,
    pub asteroids: Vec<AsteroidState>,
    pub age: f32,
    pub interval: f32,
    /// Asteroids destroyed locally while waiting for host State to drop them.
    pub pending_destroyed: HashSet<u32>,
}

#[derive(Resource, Default)]
pub struct InputThrottle {
    pub accum: f32,
    pub last_sent: (i8, i8, bool),
}

#[derive(Clone, Copy)]
pub struct PendingInput {
    pub seq: u32,
}

#[derive(Resource, Default)]
pub struct LocalPrediction {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub nickname: String,
    pub next_seq: u32,
    pub pending: VecDeque<PendingInput>,
    pub fire_cooldown: f32,
    pub local_bullets: Vec<LocalBullet>,
    pub next_local_bullet_id: u32,
}

#[derive(Clone, Copy)]
pub struct LocalBullet {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

/// After match over, return to the main menu if the player stays idle.
#[derive(Resource)]
pub struct MatchOverReturn {
    pub timer: Option<Timer>,
}

impl Default for MatchOverReturn {
    fn default() -> Self {
        Self { timer: None }
    }
}

impl MatchOverReturn {
    pub const IDLE_SECS: f32 = 30.0;

    pub fn reset(&mut self) {
        self.timer = None;
    }

    pub fn seconds_left(&self) -> Option<u32> {
        self.timer
            .as_ref()
            .map(|t| t.remaining_secs().ceil().max(0.0) as u32)
    }
}

/// Local ship blink + shock after losing a life.
#[derive(Resource)]
pub struct HurtFlash {
    pub remaining: f32,
    pub(crate) last_lives: Option<u8>,
}

impl Default for HurtFlash {
    fn default() -> Self {
        Self {
            remaining: 0.0,
            last_lives: None,
        }
    }
}

impl HurtFlash {
    const DURATION: f32 = 1.0;

    pub fn reset(&mut self) {
        self.remaining = 0.0;
        self.last_lives = None;
    }

    pub fn trigger(&mut self) {
        self.remaining = Self::DURATION;
    }

    pub fn visible(&self) -> bool {
        if self.remaining <= 0.0 {
            return true;
        }
        // ~6.25 Hz blink
        ((self.remaining * 12.5) as i32) % 2 == 0
    }
}

