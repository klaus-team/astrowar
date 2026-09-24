use crate::board::PLAY_AREA_X;
use crate::game_sync::{AsteroidKind, GameMessage};
use crate::net_bridge::NetBridge;
use crate::sounds::SfxTrigger;
use crate::Session;
use bevy::prelude::*;
use protocol::{GameDurationMinutes, RoomInfo};
use super::phase::{
    leading_score_sim, phase_from_score, phase_mods, pseudo_rand, spawn_asteroid, MAX_PHASE_EFFECT,
};
use super::prediction::integrate_ship;
use super::sync::{apply_authoritative_state, apply_snapshot_to_latest, build_snapshot};
use super::types::*;

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
pub fn playing_host_simulate(
    time: Res<Time>,
    session: Res<Session>,
    mut host: ResMut<HostSim>,
    mut latest: ResMut<LatestState>,
    bridge: Res<NetBridge>,
    mut sfx: EventWriter<SfxTrigger>,
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
            if try_fire(&mut host, &player_id)
                && session.player_id.as_deref() == Some(player_id.as_str())
            {
                sfx.send(SfxTrigger::Shot);
            }
        }

        for bullet in &mut host.bullets {
            bullet.y += BULLET_SPEED * dt;
        }
        host.bullets.retain(|b| b.y < 380.0);

        for asteroid in &mut host.asteroids {
            integrate_asteroid(asteroid, dt);
        }
        let destroyed = resolve_collisions(&mut host, session.player_id.as_deref());
        for _ in 0..destroyed {
            sfx.send(SfxTrigger::Destroy);
        }
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
        let display_phase = phase_from_score(leading_score_sim(&host.ships));
        let mods = phase_mods(display_phase.min(MAX_PHASE_EFFECT));
        let interval = ((ASTEROID_SPAWN_BASE - elapsed_factor * 0.55) * mods.spawn_interval_mul)
            .clamp(0.35, 1.2);
        if host.spawn_accum >= interval {
            host.spawn_accum = 0.0;
            spawn_asteroid(&mut host, &mods);
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
        super::send_game(&bridge, &session, &message);
    }
}

fn try_fire(host: &mut HostSim, player_id: &str) -> bool {
    let Some(ship) = host.ships.get(player_id) else {
        return false;
    };
    if !ship.alive || ship.forfeited {
        return false;
    }
    let cooldown = host.fire_cooldown.entry(player_id.to_string()).or_insert(0.0);
    if *cooldown > 0.0 {
        return false;
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
    true
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

fn spinner_crossed_horizon(asteroids: &[SimAsteroid]) -> bool {
    asteroids
        .iter()
        .any(|a| a.kind == AsteroidKind::Spinner && a.y - a.radius <= HORIZON_Y)
}
fn resolve_collisions(host: &mut HostSim, host_player_id: Option<&str>) -> u32 {
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
    let mut destroyed = 0u32;
    for (owner_id, asteroid_id) in scored {
        if award_asteroid_hit(host, &owner_id, asteroid_id) {
            destroyed += 1;
        }
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
    destroyed
}

fn award_asteroid_hit(host: &mut HostSim, player_id: &str, asteroid_id: u32) -> bool {
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
        return false;
    }
    if let Some(ship) = host.ships.get_mut(player_id) {
        if ship.alive && !ship.forfeited {
            ship.score += points;
        }
    }
    true
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
pub fn handle_relayed_game_message(
    from_player_id: &str,
    payload: &[u8],
    session: &Session,
    host: &mut HostSim,
    latest: &mut LatestState,
    prediction: &mut LocalPrediction,
    bridge: &NetBridge,
    sfx: &mut EventWriter<SfxTrigger>,
) {
    let Ok(message) = GameMessage::from_bytes(payload) else {
        return;
    };
    match message {
        GameMessage::Input {
            seq,
            move_x,
            move_y: _,
            fire,
            x,
            y: _,
        } => {
            if !session.is_owner || host.match_over {
                return;
            }
            if let Some(ship) = host.ships.get_mut(from_player_id) {
                if ship.alive && !ship.forfeited {
                    if seq >= ship.last_input_seq || ship.last_input_seq.wrapping_sub(seq) > 1000 {
                        ship.last_input_seq = seq;
                        ship.move_x = move_x;
                        ship.move_y = 0;
                        // Client is visual authority for its ship in idea-2 lite.
                        // Sideways only — ignore vertical pose from clients.
                        ship.x = x.clamp(-PLAY_AREA_X, PLAY_AREA_X);
                        ship.y = SHIP_REST_Y;
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
            super::send_game(bridge, session, &message);
        }
        GameMessage::Hit { asteroid_id } => {
            if !session.is_owner || host.match_over {
                return;
            }
            if award_asteroid_hit(host, from_player_id, asteroid_id) {
                sfx.send(SfxTrigger::Destroy);
            }
        }
    }
}
fn all_players_eliminated(host: &HostSim) -> bool {
    !host.ships.is_empty()
        && host
            .ships
            .values()
            .all(|ship| !ship.alive || ship.forfeited)
}
