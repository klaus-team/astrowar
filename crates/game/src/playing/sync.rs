use crate::board::PLAY_AREA_X;
use crate::game_sync::{AsteroidState, BulletState, GameMessage, ShipState};
use crate::Session;
use bevy::prelude::*;
use std::collections::HashSet;
use super::prediction::reconcile_prediction;
use super::types::*;

pub fn snapshot(host: &HostSim) -> GameMessage {
    build_snapshot(host)
}

pub(crate) fn build_snapshot(host: &HostSim) -> GameMessage {
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
pub(crate) fn apply_snapshot_to_latest(latest: &mut LatestState, message: &GameMessage, as_host_render: bool) {
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
pub(crate) fn interpolate_ships(latest: &LatestState) -> Vec<ShipState> {
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
                let y = to_ship.y;
                x = (x + to_ship.move_x as f32 * SHIP_SPEED * extra).clamp(-PLAY_AREA_X, PLAY_AREA_X);
                ShipState {
                    player_id: to_ship.player_id.clone(),
                    nickname: to_ship.nickname.clone(),
                    x,
                    y,
                    move_x: to_ship.move_x,
                    move_y: 0,
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
