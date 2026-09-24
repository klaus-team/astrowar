use crate::board::PLAY_AREA_X;
use crate::game_sync::{AsteroidKind, AsteroidState, BulletState, GameMessage, ShipState};
use crate::highscores::HighScores;
use crate::net_bridge::{NetBridge, NetCommand};
use crate::Session;
use bevy::prelude::*;
use protocol::{GameDurationMinutes, RoomInfo};
use std::collections::{HashMap, HashSet, VecDeque};

const SHIP_Y_MIN: f32 = -330.0;
const SHIP_Y_MAX: f32 = -220.0;
const SHIP_SPEED: f32 = 280.0;
const BULLET_SPEED: f32 = 520.0;
const STATE_HZ: f32 = 20.0;
const INPUT_HZ: f32 = 30.0;
const FIRE_COOLDOWN: f32 = 0.22;
const ASTEROID_SPAWN_BASE: f32 = 1.1;
const HORIZON_Y: f32 = SHIP_Y_MIN - 24.0;
/// Visual dashed line ~½ ship-height above the resting shooter's top edge.
const SHIP_HEIGHT: f32 = 30.0;
const SHIP_REST_Y: f32 = SHIP_Y_MIN + 20.0;
const HORIZON_LINE_Y: f32 = SHIP_REST_Y + SHIP_HEIGHT; // top of ship + ½ block
const HORIZON_DASH_W: f32 = 16.0;
const HORIZON_DASH_GAP: f32 = 10.0;
const HORIZON_DASH_H: f32 = 2.0;
const PENDING_INPUT_CAP: usize = 180;
const STARTING_LIVES: u8 = 3;

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

pub fn spawn_playing_hud(
    mut commands: Commands,
    _session: Res<Session>,
    mut high_scores: ResMut<HighScores>,
) {
    high_scores.reset_match_flag();
    let font = TextFont {
        font_size: 20.0,
        ..default()
    };
    let color = TextColor(Color::srgb(0.8, 0.85, 0.95));

    let slots = [
        (
            HudSlot::Title,
            "AstroWar\n--:--",
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
        ),
        (
            HudSlot::Players,
            "",
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                ..default()
            },
        ),
        (
            HudSlot::Mode,
            "",
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
        ),
        (
            HudSlot::Leave,
            "[Esc] Leave",
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                right: Val::Px(16.0),
                ..default()
            },
        ),
    ];

    for (slot, initial, node) in slots {
        commands.spawn((
            PlayingHud,
            slot,
            Text::new(initial),
            font.clone(),
            color.clone(),
            node,
        ));
    }

    spawn_horizon_line(&mut commands);
}

fn spawn_horizon_line(commands: &mut Commands) {
    let color = Color::srgba(0.42, 0.45, 0.52, 0.45);
    let mut x = -PLAY_AREA_X;
    while x < PLAY_AREA_X {
        let remaining = PLAY_AREA_X - x;
        let w = HORIZON_DASH_W.min(remaining);
        if w < 2.0 {
            break;
        }
        commands.spawn((
            HorizonDash,
            Sprite {
                color,
                custom_size: Some(Vec2::new(w, HORIZON_DASH_H)),
                ..default()
            },
            Transform::from_xyz(x + w * 0.5, HORIZON_LINE_Y, 0.4),
        ));
        x += HORIZON_DASH_W + HORIZON_DASH_GAP;
    }
}

pub fn cleanup_playing(
    mut commands: Commands,
    ships: Query<Entity, With<ShipSprite>>,
    bullets: Query<Entity, With<BulletSprite>>,
    local_bullets: Query<Entity, With<LocalBulletSprite>>,
    asteroids: Query<Entity, With<AsteroidSprite>>,
    hud: Query<Entity, With<PlayingHud>>,
    horizon: Query<Entity, With<HorizonDash>>,
    mut host: ResMut<HostSim>,
    mut latest: ResMut<LatestState>,
    mut input_throttle: ResMut<InputThrottle>,
    mut prediction: ResMut<LocalPrediction>,
) {
    for entity in ships
        .iter()
        .chain(bullets.iter())
        .chain(local_bullets.iter())
        .chain(asteroids.iter())
        .chain(hud.iter())
        .chain(horizon.iter())
    {
        commands.entity(entity).despawn();
    }
    *host = HostSim::default();
    *latest = LatestState::default();
    *input_throttle = InputThrottle::default();
    *prediction = LocalPrediction::default();
}

pub fn begin_host_sim(session: &Session, host: &mut HostSim, duration: GameDurationMinutes) {
    host.active = session.is_owner;
    host.tick = 0;
    host.endless = duration.is_endless();
    host.elapsed = 0.0;
    host.time_left = if host.endless {
        0.0
    } else {
        f32::from(duration.as_minutes()) * 60.0
    };
    host.match_over = false;
    host.broadcast_accum = 0.0;
    host.spawn_accum = 0.0;
    host.next_bullet_id = 1;
    host.next_asteroid_id = 1;
    host.ships.clear();
    host.bullets.clear();
    host.asteroids.clear();
    host.fire_cooldown.clear();
    host.pending_fire.clear();
    host.recent_destroyed.clear();
    if let Some(room) = &session.room {
        seed_ships_from_room(host, room);
    }
}

pub fn seed_ships_from_room(host: &mut HostSim, room: &RoomInfo) {
    for (index, player) in room.players.iter().enumerate() {
        host.ships.entry(player.id.clone()).or_insert_with(|| {
            let offset = (index as f32 - (room.players.len() as f32 - 1.0) * 0.5) * 90.0;
            SimShip {
                nickname: player.nickname.clone(),
                x: offset,
                y: SHIP_REST_Y,
                move_x: 0,
                move_y: 0,
                last_input_seq: 0,
                score: 0,
                lives: STARTING_LIVES,
                alive: true,
                forfeited: false,
            }
        });
        host.fire_cooldown.entry(player.id.clone()).or_insert(0.0);
        host.pending_fire.entry(player.id.clone()).or_insert(false);
    }
    let alive: Vec<String> = room.players.iter().map(|p| p.id.clone()).collect();
    host.ships.retain(|id, _| alive.iter().any(|a| a == id));
}

pub fn mark_player_forfeit(host: &mut HostSim, player_id: &str) {
    if let Some(ship) = host.ships.get_mut(player_id) {
        ship.forfeited = true;
        ship.alive = false;
        ship.move_x = 0;
        ship.move_y = 0;
    }
}

pub fn send_game(bridge: &NetBridge, session: &Session, message: &GameMessage) {
    if session.solo {
        return;
    }
    if let Ok(payload) = message.to_bytes() {
        bridge.send(NetCommand::Relay { payload });
    }
}

pub fn collect_local_input(keys: &ButtonInput<KeyCode>) -> (i8, i8, bool) {
    let mut move_x: i8 = 0;
    let mut move_y: i8 = 0;
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        move_x -= 1;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        move_x += 1;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        move_y -= 1;
    }
    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        move_y += 1;
    }
    let fire = keys.just_pressed(KeyCode::Space);
    (move_x, move_y, fire)
}

fn integrate_ship(x: f32, y: f32, move_x: i8, move_y: i8, dt: f32) -> (f32, f32) {
    let nx = (x + move_x as f32 * SHIP_SPEED * dt).clamp(-PLAY_AREA_X, PLAY_AREA_X);
    let ny = (y + move_y as f32 * SHIP_SPEED * dt).clamp(SHIP_Y_MIN, SHIP_Y_MAX);
    (nx, ny)
}

pub fn playing_send_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    bridge: Res<NetBridge>,
    mut host: ResMut<HostSim>,
    mut throttle: ResMut<InputThrottle>,
    prediction: Res<LocalPrediction>,
    latest: Res<LatestState>,
) {
    if latest.match_over {
        return;
    }
    let (move_x, move_y, fire) = collect_local_input(&keys);
    if session.is_owner {
        if let Some(player_id) = &session.player_id {
            if let Some(ship) = host.ships.get_mut(player_id) {
                if ship.alive && !ship.forfeited {
                    ship.move_x = move_x;
                    ship.move_y = move_y;
                    if fire {
                        *host.pending_fire.entry(player_id.clone()).or_default() = true;
                    }
                }
            }
        }
        return;
    }

    throttle.accum += time.delta_secs();
    let moving = move_x != 0 || move_y != 0;
    let changed = {
        let (lx, ly, _) = throttle.last_sent;
        lx != move_x || ly != move_y || fire
    };
    let due = throttle.accum >= 1.0 / INPUT_HZ;
    if fire || changed || (moving && due) || (due && throttle.last_sent != (0, 0, false)) {
        throttle.accum = 0.0;
        throttle.last_sent = (move_x, move_y, false);
        send_game(
            &bridge,
            &session,
            &GameMessage::Input {
                seq: prediction.next_seq,
                move_x,
                move_y,
                fire,
                x: prediction.x,
                y: prediction.y,
            },
        );
    }
}

pub fn playing_predict_local(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    mut prediction: ResMut<LocalPrediction>,
    latest: Res<LatestState>,
) {
    if session.is_owner || !prediction.active || latest.match_over {
        return;
    }
    let dt = time.delta_secs();
    prediction.fire_cooldown = (prediction.fire_cooldown - dt).max(0.0);
    let (move_x, move_y, fire) = collect_local_input(&keys);
    prediction.next_seq = prediction.next_seq.wrapping_add(1);
    let seq = prediction.next_seq;
    prediction.pending.push_back(PendingInput { seq });
    while prediction.pending.len() > PENDING_INPUT_CAP {
        prediction.pending.pop_front();
    }
    let (x, y) = integrate_ship(prediction.x, prediction.y, move_x, move_y, dt);
    prediction.x = x;
    prediction.y = y;

    if fire && prediction.fire_cooldown <= 0.0 {
        prediction.fire_cooldown = FIRE_COOLDOWN;
        let id = prediction.next_local_bullet_id;
        prediction.next_local_bullet_id = prediction.next_local_bullet_id.wrapping_add(1);
        let bx = prediction.x;
        let by = prediction.y + 18.0;
        prediction.local_bullets.push(LocalBullet {
            id,
            x: bx,
            y: by,
        });
    }
    for bullet in &mut prediction.local_bullets {
        bullet.y += BULLET_SPEED * dt;
    }
    prediction.local_bullets.retain(|b| b.y < 400.0);
}

pub fn playing_host_simulate(
    time: Res<Time>,
    session: Res<Session>,
    mut host: ResMut<HostSim>,
    mut latest: ResMut<LatestState>,
    bridge: Res<NetBridge>,
) {
    if !session.is_owner || !host.active {
        return;
    }
    let dt = time.delta_secs();
    if !host.match_over {
        host.elapsed += dt;
        if !host.endless {
            host.time_left = (host.time_left - dt).max(0.0);
            if host.time_left <= 0.0 {
                host.match_over = true;
                for ship in host.ships.values_mut() {
                    ship.move_x = 0;
                    ship.move_y = 0;
                }
            }
        }
    }

    if !host.match_over {
        for ship in host.ships.values_mut() {
            if !ship.alive || ship.forfeited {
                ship.move_x = 0;
                ship.move_y = 0;
                continue;
            }
            let (x, y) = integrate_ship(ship.x, ship.y, ship.move_x, ship.move_y, dt);
            ship.x = x;
            ship.y = y;
        }
        for cooldown in host.fire_cooldown.values_mut() {
            *cooldown = (*cooldown - dt).max(0.0);
        }

        let mut fires = Vec::new();
        for (player_id, pending) in host.pending_fire.iter_mut() {
            if *pending {
                *pending = false;
                fires.push(player_id.clone());
            }
        }
        for player_id in fires {
            try_fire(&mut host, &player_id);
        }

        for bullet in &mut host.bullets {
            bullet.y += BULLET_SPEED * dt;
        }
        host.bullets.retain(|b| b.y < 380.0);

        for asteroid in &mut host.asteroids {
            integrate_asteroid(asteroid, dt);
        }
        resolve_collisions(&mut host, session.player_id.as_deref());
        if spinner_crossed_horizon(&host.asteroids) {
            host.match_over = true;
            for ship in host.ships.values_mut() {
                ship.move_x = 0;
                ship.move_y = 0;
            }
        }
        if all_players_eliminated(&host) {
            host.match_over = true;
            for ship in host.ships.values_mut() {
                ship.move_x = 0;
                ship.move_y = 0;
            }
        }
        host.asteroids.retain(|a| a.y > HORIZON_Y - a.radius);

        host.spawn_accum += dt;
        let (total, remaining) = if host.endless {
            // Ramp difficulty over ~10 minutes of survival.
            (600.0_f32, (600.0 - host.elapsed).max(0.0))
        } else {
            let total = session
                .room
                .as_ref()
                .map(|r| f32::from(r.duration_minutes.as_minutes()) * 60.0)
                .unwrap_or(300.0)
                .max(1.0);
            (total, host.time_left)
        };
        let elapsed_factor = 1.0 - (remaining / total).clamp(0.0, 1.0);
        let interval = (ASTEROID_SPAWN_BASE - elapsed_factor * 0.55).clamp(0.4, 1.2);
        if host.spawn_accum >= interval {
            host.spawn_accum = 0.0;
            spawn_asteroid(&mut host);
        }
    }

    let snapshot = build_snapshot(&host);
    apply_snapshot_to_latest(&mut latest, &snapshot, true);

    host.broadcast_accum += dt;
    let interval = 1.0 / STATE_HZ;
    if host.broadcast_accum >= interval {
        host.broadcast_accum %= interval;
        host.tick = host.tick.wrapping_add(1);
        let mut message = build_snapshot(&host);
        if let GameMessage::State { tick, .. } = &mut message {
            *tick = host.tick;
        }
        latest.tick = host.tick;
        send_game(&bridge, &session, &message);
    }
}

fn try_fire(host: &mut HostSim, player_id: &str) {
    let Some(ship) = host.ships.get(player_id) else {
        return;
    };
    if !ship.alive || ship.forfeited {
        return;
    }
    let cooldown = host.fire_cooldown.entry(player_id.to_string()).or_insert(0.0);
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = FIRE_COOLDOWN;
    let id = host.next_bullet_id;
    host.next_bullet_id = host.next_bullet_id.wrapping_add(1);
    host.bullets.push(SimBullet {
        id,
        owner_id: player_id.to_string(),
        x: ship.x,
        y: ship.y + 18.0,
    });
}

fn spawn_asteroid(host: &mut HostSim) {
    let id = host.next_asteroid_id;
    host.next_asteroid_id = host.next_asteroid_id.wrapping_add(1);
    let x = pseudo_rand(id) * PLAY_AREA_X * 2.0 - PLAY_AREA_X;
    let roll = pseudo_rand(id.wrapping_mul(3));
    let (kind, radius, points, speed, vx) = if roll > 0.88 {
        (
            AsteroidKind::Spinner,
            14.0,
            40,
            150.0 + pseudo_rand(id.wrapping_mul(11)) * 40.0,
            0.0,
        )
    } else if roll > 0.72 {
        let dir = if pseudo_rand(id.wrapping_mul(5)) > 0.5 {
            1.0
        } else {
            -1.0
        };
        (
            AsteroidKind::Zigzag,
            13.0,
            80,
            120.0 + pseudo_rand(id.wrapping_mul(13)) * 35.0,
            dir * (140.0 + pseudo_rand(id.wrapping_mul(17)) * 60.0),
        )
    } else if roll > 0.40 {
        (
            AsteroidKind::Small,
            12.0,
            20,
            160.0 + pseudo_rand(id.wrapping_mul(7)) * 50.0,
            0.0,
        )
    } else {
        (
            AsteroidKind::Large,
            28.0,
            10,
            90.0 + pseudo_rand(id.wrapping_mul(7)) * 35.0,
            0.0,
        )
    };
    host.asteroids.push(SimAsteroid {
        id,
        kind,
        x,
        y: 360.0 + radius,
        vx,
        vy: speed,
        radius,
        points,
    });
}

fn integrate_asteroid(asteroid: &mut SimAsteroid, dt: f32) {
    asteroid.y -= asteroid.vy * dt;
    if asteroid.vx.abs() > f32::EPSILON {
        asteroid.x += asteroid.vx * dt;
        let limit = PLAY_AREA_X - asteroid.radius;
        if asteroid.x > limit {
            asteroid.x = limit;
            asteroid.vx = -asteroid.vx.abs();
        } else if asteroid.x < -limit {
            asteroid.x = -limit;
            asteroid.vx = asteroid.vx.abs();
        }
    }
}

fn integrate_asteroid_state(asteroid: &mut AsteroidState, dt: f32) {
    asteroid.y -= asteroid.vy * dt;
    if asteroid.vx.abs() > f32::EPSILON {
        asteroid.x += asteroid.vx * dt;
        let limit = PLAY_AREA_X - asteroid.radius;
        if asteroid.x > limit {
            asteroid.x = limit;
            asteroid.vx = -asteroid.vx.abs();
        } else if asteroid.x < -limit {
            asteroid.x = -limit;
            asteroid.vx = asteroid.vx.abs();
        }
    }
}

fn spinner_crossed_horizon(asteroids: &[SimAsteroid]) -> bool {
    asteroids
        .iter()
        .any(|a| a.kind == AsteroidKind::Spinner && a.y - a.radius <= HORIZON_Y)
}

fn pseudo_rand(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    x ^= x >> 16;
    (x & 0xffff) as f32 / 65535.0
}

fn resolve_collisions(host: &mut HostSim, host_player_id: Option<&str>) {
    let mut hit_bullets = Vec::new();
    let mut scored = Vec::new();

    for bullet in &host.bullets {
        if host_player_id.is_some_and(|id| bullet.owner_id != id) {
            continue;
        }
        for asteroid in &host.asteroids {
            let dx = bullet.x - asteroid.x;
            let dy = bullet.y - asteroid.y;
            if dx * dx + dy * dy <= asteroid.radius * asteroid.radius {
                scored.push((bullet.owner_id.clone(), asteroid.id));
                hit_bullets.push(bullet.id);
                break;
            }
        }
    }
    for (owner_id, asteroid_id) in scored {
        award_asteroid_hit(host, &owner_id, asteroid_id);
    }
    host.bullets.retain(|b| !hit_bullets.contains(&b.id));

    let mut ship_hits = Vec::new();
    for (player_id, ship) in &host.ships {
        if !ship.alive || ship.forfeited {
            continue;
        }
        for asteroid in &host.asteroids {
            let dx = ship.x - asteroid.x;
            let dy = ship.y - asteroid.y;
            let hit_r = asteroid.radius + 14.0;
            if dx * dx + dy * dy <= hit_r * hit_r {
                ship_hits.push((player_id.clone(), asteroid.id));
                break;
            }
        }
    }
    for (player_id, asteroid_id) in ship_hits {
        let kind = host
            .asteroids
            .iter()
            .find(|a| a.id == asteroid_id)
            .map(|a| a.kind);
        host.asteroids.retain(|a| a.id != asteroid_id);
        if kind == Some(AsteroidKind::Spinner) {
            host.match_over = true;
            for ship in host.ships.values_mut() {
                ship.move_x = 0;
                ship.move_y = 0;
            }
        }
        if let Some(ship) = host.ships.get_mut(&player_id) {
            if ship.lives > 0 {
                ship.lives -= 1;
            }
            if ship.lives == 0 {
                ship.alive = false;
                ship.move_x = 0;
                ship.move_y = 0;
            }
        }
    }
}

fn award_asteroid_hit(host: &mut HostSim, player_id: &str, asteroid_id: u32) {
    let removed = host
        .asteroids
        .iter()
        .position(|a| a.id == asteroid_id)
        .map(|index| host.asteroids.remove(index));

    let points = if let Some(asteroid) = removed {
        host.recent_destroyed.insert(asteroid_id, asteroid.points);
        if asteroid.kind == AsteroidKind::Large {
            split_large_rock(host, &asteroid);
        }
        asteroid.points
    } else {
        host.recent_destroyed
            .get(&asteroid_id)
            .copied()
            .unwrap_or(0)
    };
    if points <= 0 {
        return;
    }
    if let Some(ship) = host.ships.get_mut(player_id) {
        if ship.alive && !ship.forfeited {
            ship.score += points;
        }
    }
}

fn split_large_rock(host: &mut HostSim, parent: &SimAsteroid) {
    for i in 0..2 {
        let id = host.next_asteroid_id;
        host.next_asteroid_id = host.next_asteroid_id.wrapping_add(1);
        let offset = if i == 0 { -18.0 } else { 18.0 };
        let dir = if i == 0 { -1.0 } else { 1.0 };
        let spread = 110.0 + pseudo_rand(id) * 50.0;
        host.asteroids.push(SimAsteroid {
            id,
            kind: AsteroidKind::Small,
            x: (parent.x + offset).clamp(-PLAY_AREA_X, PLAY_AREA_X),
            y: parent.y,
            vx: dir * spread,
            vy: parent.vy + 40.0 + pseudo_rand(id.wrapping_mul(3)) * 30.0,
            radius: 12.0,
            points: 20,
        });
    }
}

pub fn playing_client_local_hits(
    session: Res<Session>,
    bridge: Res<NetBridge>,
    mut latest: ResMut<LatestState>,
    mut prediction: ResMut<LocalPrediction>,
) {
    if session.is_owner || latest.match_over || !prediction.active {
        return;
    }

    let mut hit_asteroids = Vec::new();
    let mut hit_bullets = Vec::new();
    for bullet in &prediction.local_bullets {
        for asteroid in &latest.asteroids {
            if hit_asteroids.contains(&asteroid.id) {
                continue;
            }
            let dx = bullet.x - asteroid.x;
            let dy = bullet.y - asteroid.y;
            if dx * dx + dy * dy <= asteroid.radius * asteroid.radius {
                hit_asteroids.push(asteroid.id);
                hit_bullets.push(bullet.id);
                break;
            }
        }
    }
    if hit_asteroids.is_empty() {
        return;
    }

    prediction
        .local_bullets
        .retain(|b| !hit_bullets.contains(&b.id));
    latest
        .asteroids
        .retain(|a| !hit_asteroids.contains(&a.id));

    for asteroid_id in hit_asteroids {
        latest.pending_destroyed.insert(asteroid_id);
        send_game(&bridge, &session, &GameMessage::Hit { asteroid_id });
    }
}

pub fn snapshot(host: &HostSim) -> GameMessage {
    build_snapshot(host)
}

fn build_snapshot(host: &HostSim) -> GameMessage {
    let time_left_secs = if host.endless {
        host.elapsed.floor() as u32
    } else {
        host.time_left.ceil() as u32
    };
    GameMessage::State {
        tick: host.tick,
        time_left_secs,
        match_over: host.match_over,
        ships: host
            .ships
            .iter()
            .map(|(player_id, ship)| ShipState {
                player_id: player_id.clone(),
                nickname: ship.nickname.clone(),
                x: ship.x,
                y: ship.y,
                move_x: ship.move_x,
                move_y: ship.move_y,
                last_input_seq: ship.last_input_seq,
                score: ship.score,
                lives: ship.lives,
                alive: ship.alive,
                forfeited: ship.forfeited,
            })
            .collect(),
        bullets: host
            .bullets
            .iter()
            .map(|b| BulletState {
                id: b.id,
                owner_id: b.owner_id.clone(),
                x: b.x,
                y: b.y,
            })
            .collect(),
        asteroids: host
            .asteroids
            .iter()
            .map(|a| AsteroidState {
                id: a.id,
                kind: a.kind,
                x: a.x,
                y: a.y,
                vx: a.vx,
                vy: a.vy,
                radius: a.radius,
            })
            .collect(),
    }
}

fn apply_snapshot_to_latest(latest: &mut LatestState, message: &GameMessage, as_host_render: bool) {
    let GameMessage::State {
        tick,
        time_left_secs,
        match_over,
        ships,
        bullets,
        asteroids,
    } = message
    else {
        return;
    };
    if as_host_render {
        latest.from_ships = ships.clone();
        latest.to_ships = ships.clone();
        latest.age = 0.0;
    }
    latest.tick = *tick;
    latest.time_left_secs = *time_left_secs;
    latest.match_over = *match_over;
    latest.bullets = bullets.clone();
    latest.asteroids = asteroids.clone();
    latest.interval = 1.0 / STATE_HZ;
}

pub fn apply_authoritative_state(
    latest: &mut LatestState,
    prediction: &mut LocalPrediction,
    local_player_id: Option<&str>,
    tick: u32,
    time_left_secs: u32,
    match_over: bool,
    ships: Vec<ShipState>,
    bullets: Vec<BulletState>,
    asteroids: Vec<AsteroidState>,
) {
    if tick < latest.tick && latest.tick.wrapping_sub(tick) < 1000 {
        return;
    }
    let current = interpolate_ships(latest);
    latest.from_ships = current;
    latest.to_ships = ships.clone();
    latest.tick = tick;
    latest.time_left_secs = time_left_secs;
    latest.match_over = match_over;
    // Host owns spawn/despawn; clients simulate motion locally between snapshots.
    merge_by_id(&mut latest.bullets, bullets, |b| b.id);
    merge_asteroids(
        &mut latest.asteroids,
        asteroids,
        &mut latest.pending_destroyed,
    );
    latest.age = 0.0;
    latest.interval = 1.0 / STATE_HZ;

    if let Some(local_id) = local_player_id {
        if let Some(auth) = ships.iter().find(|ship| ship.player_id == local_id) {
            reconcile_prediction(prediction, auth);
        }
    }
}

fn merge_asteroids(
    local: &mut Vec<AsteroidState>,
    auth: Vec<AsteroidState>,
    pending_destroyed: &mut HashSet<u32>,
) {
    // Drop suppressions once the host State no longer lists that asteroid.
    pending_destroyed.retain(|id| auth.iter().any(|a| a.id == *id));

    // Existence only: new ids spawn at host pose; surviving ids keep client motion.
    // Never revive an id we already destroyed locally while a stale State still carries it.
    local.retain(|a| {
        auth.iter().any(|r| r.id == a.id) && !pending_destroyed.contains(&a.id)
    });
    for remote in auth {
        if pending_destroyed.contains(&remote.id) {
            continue;
        }
        if !local.iter().any(|a| a.id == remote.id) {
            local.push(remote);
        }
    }
}

fn merge_by_id<T, F>(local: &mut Vec<T>, auth: Vec<T>, id_of: F)
where
    F: Fn(&T) -> u32,
{
    local.retain(|item| auth.iter().any(|r| id_of(r) == id_of(item)));
    for remote in auth {
        if !local.iter().any(|item| id_of(item) == id_of(&remote)) {
            local.push(remote);
        }
    }
}

fn reconcile_prediction(prediction: &mut LocalPrediction, auth: &ShipState) {
    prediction.nickname = auth.nickname.clone();
    if !prediction.active {
        prediction.active = true;
        prediction.x = auth.x;
        prediction.y = auth.y;
        prediction.pending.clear();
        return;
    }
    if !auth.alive || auth.forfeited {
        prediction.x = auth.x;
        prediction.y = auth.y;
        prediction.pending.clear();
        return;
    }
    // Idea-2 lite: trust local motion for the ship; auth only drives score/lives elsewhere.
    prediction
        .pending
        .retain(|input| input.seq > auth.last_input_seq);
}

fn interpolate_ships(latest: &LatestState) -> Vec<ShipState> {
    if latest.to_ships.is_empty() {
        return latest.from_ships.clone();
    }
    if latest.from_ships.is_empty() {
        return latest.to_ships.clone();
    }
    let t = if latest.interval <= f32::EPSILON {
        1.0
    } else {
        (latest.age / latest.interval).clamp(0.0, 1.0)
    };
    let extra = if latest.interval <= f32::EPSILON {
        0.0
    } else {
        (latest.age - latest.interval).max(0.0).min(0.1)
    };
    latest
        .to_ships
        .iter()
        .map(|to_ship| {
            if let Some(from_ship) = latest
                .from_ships
                .iter()
                .find(|ship| ship.player_id == to_ship.player_id)
            {
                let mut x = from_ship.x + (to_ship.x - from_ship.x) * t;
                let mut y = from_ship.y + (to_ship.y - from_ship.y) * t;
                x = (x + to_ship.move_x as f32 * SHIP_SPEED * extra).clamp(-PLAY_AREA_X, PLAY_AREA_X);
                y = (y + to_ship.move_y as f32 * SHIP_SPEED * extra).clamp(SHIP_Y_MIN, SHIP_Y_MAX);
                ShipState {
                    player_id: to_ship.player_id.clone(),
                    nickname: to_ship.nickname.clone(),
                    x,
                    y,
                    move_x: to_ship.move_x,
                    move_y: to_ship.move_y,
                    last_input_seq: to_ship.last_input_seq,
                    score: to_ship.score,
                    lives: to_ship.lives,
                    alive: to_ship.alive,
                    forfeited: to_ship.forfeited,
                }
            } else {
                to_ship.clone()
            }
        })
        .collect()
}

pub fn advance_interpolation(
    time: Res<Time>,
    mut latest: ResMut<LatestState>,
    session: Res<Session>,
) {
    if session.is_owner {
        return;
    }
    let dt = time.delta_secs();
    latest.age += dt;
    for asteroid in &mut latest.asteroids {
        integrate_asteroid_state(asteroid, dt);
    }
    for bullet in &mut latest.bullets {
        bullet.y += BULLET_SPEED * dt;
    }
    latest.bullets.retain(|b| b.y < 400.0);
}

pub fn handle_relayed_game_message(
    from_player_id: &str,
    payload: &[u8],
    session: &Session,
    host: &mut HostSim,
    latest: &mut LatestState,
    prediction: &mut LocalPrediction,
    bridge: &NetBridge,
) {
    let Ok(message) = GameMessage::from_bytes(payload) else {
        return;
    };
    match message {
        GameMessage::Input {
            seq,
            move_x,
            move_y,
            fire,
            x,
            y,
        } => {
            if !session.is_owner || host.match_over {
                return;
            }
            if let Some(ship) = host.ships.get_mut(from_player_id) {
                if ship.alive && !ship.forfeited {
                    if seq >= ship.last_input_seq || ship.last_input_seq.wrapping_sub(seq) > 1000 {
                        ship.last_input_seq = seq;
                        ship.move_x = move_x;
                        ship.move_y = move_y;
                        // Client is visual authority for its ship in idea-2 lite.
                        ship.x = x.clamp(-PLAY_AREA_X, PLAY_AREA_X);
                        ship.y = y.clamp(SHIP_Y_MIN, SHIP_Y_MAX);
                    }
                    if fire {
                        *host.pending_fire.entry(from_player_id.to_string()).or_default() = true;
                    }
                }
            }
        }
        GameMessage::State {
            tick,
            time_left_secs,
            match_over,
            ships,
            bullets,
            asteroids,
        } => {
            if session.is_owner {
                return;
            }
            apply_authoritative_state(
                latest,
                prediction,
                session.player_id.as_deref(),
                tick,
                time_left_secs,
                match_over,
                ships,
                bullets,
                asteroids,
            );
        }
        GameMessage::RequestSnapshot => {
            if !session.is_owner || !host.active {
                return;
            }
            let message = build_snapshot(host);
            apply_snapshot_to_latest(latest, &message, true);
            send_game(bridge, session, &message);
        }
        GameMessage::Hit { asteroid_id } => {
            if !session.is_owner || host.match_over {
                return;
            }
            award_asteroid_hit(host, from_player_id, asteroid_id);
        }
    }
}

pub fn sync_world_sprites(
    mut commands: Commands,
    latest: Res<LatestState>,
    prediction: Res<LocalPrediction>,
    mut ships: Query<
        (Entity, &ShipSprite, &mut Transform, &mut Sprite),
        (
            Without<BulletSprite>,
            Without<LocalBulletSprite>,
            Without<AsteroidSprite>,
        ),
    >,
    mut bullets: Query<
        (Entity, &BulletSprite, &mut Transform),
        (
            Without<ShipSprite>,
            Without<LocalBulletSprite>,
            Without<AsteroidSprite>,
        ),
    >,
    mut local_bullets: Query<
        (Entity, &LocalBulletSprite, &mut Transform),
        (
            Without<ShipSprite>,
            Without<BulletSprite>,
            Without<AsteroidSprite>,
        ),
    >,
    mut asteroids: Query<
        (Entity, &AsteroidSprite, &mut Transform, &mut Sprite),
        (
            Without<ShipSprite>,
            Without<BulletSprite>,
            Without<LocalBulletSprite>,
        ),
    >,
    mut hud: Query<(&HudSlot, &mut Text), With<PlayingHud>>,
    session: Res<Session>,
) {
    let mut rendered_ships = if session.is_owner {
        latest.to_ships.clone()
    } else {
        interpolate_ships(&latest)
    };

    if !session.is_owner && prediction.active {
        if let Some(local_id) = &session.player_id {
            if let Some(local) = rendered_ships
                .iter_mut()
                .find(|ship| &ship.player_id == local_id)
            {
                if local.alive && !local.forfeited {
                    local.x = prediction.x;
                    local.y = prediction.y;
                }
                if !prediction.nickname.is_empty() {
                    local.nickname = prediction.nickname.clone();
                }
            }
        }
    }

    let mut seen_ships = Vec::new();
    for ship in &rendered_ships {
        seen_ships.push(ship.player_id.clone());
        let mut color = color_for_id(&ship.player_id);
        if !ship.alive || ship.forfeited {
            color = Color::srgba(0.35, 0.35, 0.4, 0.45);
        }
        if let Some((_, _, mut transform, mut sprite)) = ships
            .iter_mut()
            .find(|(_, marker, _, _)| marker.player_id == ship.player_id)
        {
            transform.translation.x = ship.x;
            transform.translation.y = ship.y;
            sprite.color = color;
        } else {
            commands.spawn((
                ShipSprite {
                    player_id: ship.player_id.clone(),
                },
                Sprite {
                    color,
                    custom_size: Some(Vec2::new(26.0, 30.0)),
                    ..default()
                },
                Transform::from_xyz(ship.x, ship.y, 2.0),
            ));
        }
    }
    for (entity, marker, _, _) in &ships {
        if !seen_ships.iter().any(|id| id == &marker.player_id) {
            commands.entity(entity).despawn();
        }
    }

    let local_id = session.player_id.as_deref();
    let hide_own_net_bullets = !session.is_owner;
    let mut seen_bullets = Vec::new();
    for bullet in &latest.bullets {
        if hide_own_net_bullets && local_id.is_some_and(|id| bullet.owner_id == id) {
            continue;
        }
        seen_bullets.push(bullet.id);
        if let Some((_, _, mut transform)) = bullets
            .iter_mut()
            .find(|(_, marker, _)| marker.id == bullet.id)
        {
            transform.translation.x = bullet.x;
            transform.translation.y = bullet.y;
        } else {
            commands.spawn((
                BulletSprite { id: bullet.id },
                Sprite {
                    color: Color::srgb(0.95, 0.9, 0.4),
                    custom_size: Some(Vec2::new(4.0, 12.0)),
                    ..default()
                },
                Transform::from_xyz(bullet.x, bullet.y, 1.5),
            ));
        }
    }
    for (entity, marker, _) in &bullets {
        if !seen_bullets.contains(&marker.id) {
            commands.entity(entity).despawn();
        }
    }

    let mut seen_local = Vec::new();
    if !session.is_owner {
        for bullet in &prediction.local_bullets {
            seen_local.push(bullet.id);
            if let Some((_, _, mut transform)) = local_bullets
                .iter_mut()
                .find(|(_, marker, _)| marker.id == bullet.id)
            {
                transform.translation.x = bullet.x;
                transform.translation.y = bullet.y;
            } else {
                commands.spawn((
                    LocalBulletSprite { id: bullet.id },
                    Sprite {
                        color: Color::srgb(0.95, 0.9, 0.4),
                        custom_size: Some(Vec2::new(4.0, 12.0)),
                        ..default()
                    },
                    Transform::from_xyz(bullet.x, bullet.y, 1.6),
                ));
            }
        }
    }
    for (entity, marker, _) in &local_bullets {
        if !seen_local.contains(&marker.id) {
            commands.entity(entity).despawn();
        }
    }

    let mut seen_asteroids = Vec::new();
    for asteroid in &latest.asteroids {
        seen_asteroids.push(asteroid.id);
        let size = Vec2::splat(asteroid.radius * 2.0);
        let color = color_for_asteroid(asteroid.kind);
        if let Some((_, _, mut transform, mut sprite)) = asteroids
            .iter_mut()
            .find(|(_, marker, _, _)| marker.id == asteroid.id)
        {
            transform.translation.x = asteroid.x;
            transform.translation.y = asteroid.y;
            sprite.custom_size = Some(size);
            sprite.color = color;
        } else {
            commands.spawn((
                AsteroidSprite { id: asteroid.id },
                Sprite {
                    color,
                    custom_size: Some(size),
                    ..default()
                },
                Transform::from_xyz(asteroid.x, asteroid.y, 1.0),
            ));
        }
    }
    for (entity, marker, _, _) in &asteroids {
        if !seen_asteroids.contains(&marker.id) {
            commands.entity(entity).despawn();
        }
    }

    let mut scores = rendered_ships.clone();
    scores.sort_by(|a, b| b.score.cmp(&a.score));
    let board = scores
        .iter()
        .map(|ship| {
            let flag = if ship.forfeited {
                " FORFEIT"
            } else if !ship.alive {
                " OUT"
            } else {
                ""
            };
            format!(
                "{} {} pts / {} lives{flag}",
                ship.nickname, ship.score, ship.lives
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let minutes = latest.time_left_secs / 60;
    let seconds = latest.time_left_secs % 60;
    let endless = session
        .room
        .as_ref()
        .map(|r| r.duration_minutes.is_endless())
        .unwrap_or(false);
    let title = if endless {
        format!("AstroWar\n{minutes:02}:{seconds:02} elapsed")
    } else {
        format!("AstroWar\n{minutes:02}:{seconds:02}")
    };
    let mode = if latest.match_over {
        let winner = scores
            .iter()
            .find(|s| !s.forfeited)
            .map(|s| s.nickname.as_str())
            .unwrap_or("nobody");
        format!("MATCH OVER\nleader: {winner}")
    } else if session.solo {
        if endless {
            "Solo · until out".to_string()
        } else {
            "Solo".to_string()
        }
    } else {
        "Competitive".to_string()
    };

    for (slot, mut text) in &mut hud {
        *text = Text::new(match slot {
            HudSlot::Title => title.clone(),
            HudSlot::Players => board.clone(),
            HudSlot::Mode => mode.clone(),
            HudSlot::Leave => "[Esc] Leave".to_string(),
        });
    }
}

/// Persist the local player's score once when the match ends.
pub fn maybe_record_high_score(
    session: Res<Session>,
    latest: Res<LatestState>,
    host: Res<HostSim>,
    mut high_scores: ResMut<HighScores>,
) {
    let match_over = latest.match_over || (session.is_owner && host.match_over);
    if !match_over {
        return;
    }
    record_local_high_score(&session, &latest, &host, &mut high_scores);
}

pub fn record_local_high_score(
    session: &Session,
    latest: &LatestState,
    host: &HostSim,
    high_scores: &mut HighScores,
) {
    let Some(player_id) = session.player_id.as_deref() else {
        return;
    };

    let (nickname, score) = if let Some(ship) = latest
        .to_ships
        .iter()
        .chain(latest.from_ships.iter())
        .find(|s| s.player_id == player_id)
    {
        (ship.nickname.clone(), ship.score)
    } else if let Some(ship) = host.ships.get(player_id) {
        (ship.nickname.clone(), ship.score)
    } else {
        return;
    };

    high_scores.consider_score(&nickname, score);
}

fn all_players_eliminated(host: &HostSim) -> bool {
    !host.ships.is_empty()
        && host
            .ships
            .values()
            .all(|ship| !ship.alive || ship.forfeited)
}

fn color_for_id(player_id: &str) -> Color {
    let mut hash: u32 = 2166136261;
    for byte in player_id.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16777619);
    }
    let r = 0.35 + ((hash & 0xff) as f32 / 255.0) * 0.55;
    let g = 0.35 + (((hash >> 8) & 0xff) as f32 / 255.0) * 0.55;
    let b = 0.35 + (((hash >> 16) & 0xff) as f32 / 255.0) * 0.55;
    Color::srgb(r, g, b)
}

fn color_for_asteroid(kind: AsteroidKind) -> Color {
    match kind {
        AsteroidKind::Large => Color::srgb(0.55, 0.5, 0.48),
        AsteroidKind::Small => Color::srgb(0.7, 0.62, 0.55),
        AsteroidKind::Spinner => Color::srgb(0.95, 0.92, 0.85),
        AsteroidKind::Zigzag => Color::srgb(0.35, 0.75, 0.95),
    }
}
