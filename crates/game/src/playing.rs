use crate::game_sync::{GameMessage, ShipState};
use crate::net_bridge::{NetBridge, NetCommand};
use crate::Session;
use bevy::prelude::*;
use protocol::RoomInfo;
use std::collections::HashMap;

pub const PLAY_AREA: f32 = 360.0;
const SHIP_SPEED: f32 = 220.0;
const STATE_HZ: f32 = 30.0;
const INPUT_HZ: f32 = 30.0;
const RECONCILE_SNAP: f32 = 64.0;
const RECONCILE_BLEND: f32 = 0.35;

#[derive(Component)]
pub struct ShipSprite {
    pub player_id: String,
}

#[derive(Component)]
pub struct PlayingHud;

#[derive(Resource, Default)]
pub struct HostSim {
    pub active: bool,
    pub tick: u32,
    pub ships: HashMap<String, SimShip>,
    pub broadcast_accum: f32,
}

#[derive(Clone)]
pub struct SimShip {
    pub nickname: String,
    pub x: f32,
    pub y: f32,
    pub move_x: i8,
    pub move_y: i8,
}

#[derive(Resource, Default)]
pub struct LatestState {
    pub tick: u32,
    pub from: Vec<ShipState>,
    pub to: Vec<ShipState>,
    pub age: f32,
    pub interval: f32,
}

#[derive(Resource, Default)]
pub struct InputThrottle {
    pub accum: f32,
    pub last_sent: (i8, i8),
}

#[derive(Resource, Default)]
pub struct LocalPrediction {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub nickname: String,
}

pub fn spawn_playing_hud(mut commands: Commands, session: Res<Session>) {
    let role = if session.is_owner { "HOST" } else { "CLIENT" };
    commands.spawn((
        PlayingHud,
        Text::new(format!(
            "Dummy sync ({role})\nArrows move  |  Esc leave\nWaiting for state..."
        )),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.85, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

pub fn cleanup_playing(
    mut commands: Commands,
    ships: Query<Entity, With<ShipSprite>>,
    hud: Query<Entity, With<PlayingHud>>,
    mut host: ResMut<HostSim>,
    mut latest: ResMut<LatestState>,
    mut input_throttle: ResMut<InputThrottle>,
    mut prediction: ResMut<LocalPrediction>,
) {
    for entity in &ships {
        commands.entity(entity).despawn();
    }
    for entity in &hud {
        commands.entity(entity).despawn();
    }
    *host = HostSim::default();
    *latest = LatestState::default();
    *input_throttle = InputThrottle::default();
    *prediction = LocalPrediction::default();
}

pub fn begin_host_sim(session: &Session, host: &mut HostSim) {
    host.active = session.is_owner;
    host.tick = 0;
    host.broadcast_accum = 0.0;
    host.ships.clear();
    if let Some(room) = &session.room {
        seed_ships_from_room(host, room);
    }
}

pub fn seed_ships_from_room(host: &mut HostSim, room: &RoomInfo) {
    for (index, player) in room.players.iter().enumerate() {
        host.ships.entry(player.id.clone()).or_insert_with(|| {
            let angle = index as f32 * std::f32::consts::TAU / room.players.len().max(1) as f32;
            SimShip {
                nickname: player.nickname.clone(),
                x: angle.cos() * 120.0,
                y: angle.sin() * 120.0,
                move_x: 0,
                move_y: 0,
            }
        });
    }
    let alive: Vec<String> = room.players.iter().map(|p| p.id.clone()).collect();
    host.ships.retain(|id, _| alive.iter().any(|a| a == id));
}

pub fn send_game(bridge: &NetBridge, message: &GameMessage) {
    if let Ok(payload) = message.to_bytes() {
        bridge.send(NetCommand::Relay { payload });
    }
}

pub fn collect_local_input(keys: &ButtonInput<KeyCode>) -> (i8, i8) {
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
    (move_x, move_y)
}

fn integrate(x: f32, y: f32, move_x: i8, move_y: i8, dt: f32) -> (f32, f32) {
    let nx = (x + move_x as f32 * SHIP_SPEED * dt).clamp(-PLAY_AREA, PLAY_AREA);
    let ny = (y + move_y as f32 * SHIP_SPEED * dt).clamp(-PLAY_AREA, PLAY_AREA);
    (nx, ny)
}

pub fn playing_send_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    bridge: Res<NetBridge>,
    mut host: ResMut<HostSim>,
    mut throttle: ResMut<InputThrottle>,
) {
    let (move_x, move_y) = collect_local_input(&keys);
    if session.is_owner {
        if let Some(player_id) = &session.player_id {
            if let Some(ship) = host.ships.get_mut(player_id) {
                ship.move_x = move_x;
                ship.move_y = move_y;
            }
        }
        return;
    }

    throttle.accum += time.delta_secs();
    let changed = throttle.last_sent != (move_x, move_y);
    let due = throttle.accum >= 1.0 / INPUT_HZ;
    if changed || due {
        throttle.accum = 0.0;
        throttle.last_sent = (move_x, move_y);
        send_game(&bridge, &GameMessage::Input { move_x, move_y });
    }
}

pub fn playing_predict_local(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    mut prediction: ResMut<LocalPrediction>,
) {
    if session.is_owner || !prediction.active {
        return;
    }
    let (move_x, move_y) = collect_local_input(&keys);
    let (x, y) = integrate(prediction.x, prediction.y, move_x, move_y, time.delta_secs());
    prediction.x = x;
    prediction.y = y;
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
    for ship in host.ships.values_mut() {
        let (x, y) = integrate(ship.x, ship.y, ship.move_x, ship.move_y, dt);
        ship.x = x;
        ship.y = y;
    }

    let render_ships = ships_snapshot(&host);
    latest.from = render_ships.clone();
    latest.to = render_ships;
    latest.age = 0.0;
    latest.interval = 1.0 / STATE_HZ;

    host.broadcast_accum += dt;
    if host.broadcast_accum >= 1.0 / STATE_HZ {
        host.broadcast_accum = 0.0;
        host.tick = host.tick.wrapping_add(1);
        latest.tick = host.tick;
        let ships = ships_snapshot(&host);
        send_game(
            &bridge,
            &GameMessage::State {
                tick: host.tick,
                ships,
            },
        );
    } else {
        latest.tick = host.tick;
    }
}

pub fn snapshot(host: &HostSim) -> GameMessage {
    GameMessage::State {
        tick: host.tick,
        ships: ships_snapshot(host),
    }
}

fn ships_snapshot(host: &HostSim) -> Vec<ShipState> {
    host.ships
        .iter()
        .map(|(player_id, ship)| ShipState {
            player_id: player_id.clone(),
            nickname: ship.nickname.clone(),
            x: ship.x,
            y: ship.y,
        })
        .collect()
}

pub fn apply_authoritative_state(
    latest: &mut LatestState,
    prediction: &mut LocalPrediction,
    local_player_id: Option<&str>,
    tick: u32,
    ships: Vec<ShipState>,
) {
    if tick < latest.tick && latest.tick.wrapping_sub(tick) < 1000 {
        return;
    }
    let current = interpolate_ships(latest);
    latest.from = current;
    latest.to = ships.clone();
    latest.tick = tick;
    latest.age = 0.0;
    latest.interval = 1.0 / STATE_HZ;

    if let Some(local_id) = local_player_id {
        if let Some(auth) = ships.iter().find(|ship| ship.player_id == local_id) {
            reconcile_prediction(prediction, auth);
        }
    }
}

fn reconcile_prediction(prediction: &mut LocalPrediction, auth: &ShipState) {
    prediction.nickname = auth.nickname.clone();
    if !prediction.active {
        prediction.active = true;
        prediction.x = auth.x;
        prediction.y = auth.y;
        return;
    }
    let dx = auth.x - prediction.x;
    let dy = auth.y - prediction.y;
    let error = (dx * dx + dy * dy).sqrt();
    if error >= RECONCILE_SNAP {
        prediction.x = auth.x;
        prediction.y = auth.y;
    } else {
        prediction.x += dx * RECONCILE_BLEND;
        prediction.y += dy * RECONCILE_BLEND;
    }
}

fn interpolate_ships(latest: &LatestState) -> Vec<ShipState> {
    if latest.to.is_empty() {
        return latest.from.clone();
    }
    if latest.from.is_empty() {
        return latest.to.clone();
    }
    let t = if latest.interval <= f32::EPSILON {
        1.0
    } else {
        (latest.age / latest.interval).clamp(0.0, 1.0)
    };
    latest
        .to
        .iter()
        .map(|to_ship| {
            if let Some(from_ship) = latest
                .from
                .iter()
                .find(|ship| ship.player_id == to_ship.player_id)
            {
                ShipState {
                    player_id: to_ship.player_id.clone(),
                    nickname: to_ship.nickname.clone(),
                    x: from_ship.x + (to_ship.x - from_ship.x) * t,
                    y: from_ship.y + (to_ship.y - from_ship.y) * t,
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
    latest.age += time.delta_secs();
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
        GameMessage::Input { move_x, move_y } => {
            if !session.is_owner {
                return;
            }
            if let Some(ship) = host.ships.get_mut(from_player_id) {
                ship.move_x = move_x;
                ship.move_y = move_y;
            }
        }
        GameMessage::State { tick, ships } => {
            if session.is_owner {
                return;
            }
            apply_authoritative_state(
                latest,
                prediction,
                session.player_id.as_deref(),
                tick,
                ships,
            );
        }
        GameMessage::RequestSnapshot => {
            if !session.is_owner || !host.active {
                return;
            }
            if let GameMessage::State { tick, ships } = snapshot(host) {
                latest.from = ships.clone();
                latest.to = ships.clone();
                latest.tick = tick;
                latest.age = 0.0;
                latest.interval = 1.0 / STATE_HZ;
                send_game(&bridge, &GameMessage::State { tick, ships });
            }
        }
    }
}

pub fn sync_ship_sprites(
    mut commands: Commands,
    latest: Res<LatestState>,
    prediction: Res<LocalPrediction>,
    mut ships: Query<(Entity, &ShipSprite, &mut Transform)>,
    mut hud: Query<&mut Text, With<PlayingHud>>,
    session: Res<Session>,
) {
    let mut rendered = if session.is_owner {
        latest.to.clone()
    } else {
        interpolate_ships(&latest)
    };

    if !session.is_owner && prediction.active {
        if let Some(local_id) = &session.player_id {
            if let Some(local) = rendered
                .iter_mut()
                .find(|ship| &ship.player_id == local_id)
            {
                local.x = prediction.x;
                local.y = prediction.y;
                if !prediction.nickname.is_empty() {
                    local.nickname = prediction.nickname.clone();
                }
            } else {
                rendered.push(ShipState {
                    player_id: local_id.clone(),
                    nickname: prediction.nickname.clone(),
                    x: prediction.x,
                    y: prediction.y,
                });
            }
        }
    }

    let mut seen = Vec::new();
    for ship in &rendered {
        seen.push(ship.player_id.clone());
        let color = color_for_id(&ship.player_id);
        if let Some((_, _, mut transform)) = ships
            .iter_mut()
            .find(|(_, sprite, _)| sprite.player_id == ship.player_id)
        {
            transform.translation.x = ship.x;
            transform.translation.y = ship.y;
        } else {
            commands.spawn((
                ShipSprite {
                    player_id: ship.player_id.clone(),
                },
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(28.0)),
                    ..default()
                },
                Transform::from_xyz(ship.x, ship.y, 1.0),
            ));
        }
    }

    for (entity, sprite, _) in &ships {
        if !seen.iter().any(|id| id == &sprite.player_id) {
            commands.entity(entity).despawn();
        }
    }

    let role = if session.is_owner { "HOST" } else { "CLIENT" };
    let roster = rendered
        .iter()
        .map(|ship| format!("{} ({:.0},{:.0})", ship.nickname, ship.x, ship.y))
        .collect::<Vec<_>>()
        .join("  |  ");
    for mut text in &mut hud {
        *text = Text::new(format!(
            "Dummy sync ({role}) net tick {}\nArrows move  |  Esc leave\n{roster}",
            latest.tick
        ));
    }
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
