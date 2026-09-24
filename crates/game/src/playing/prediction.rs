use crate::board::PLAY_AREA_X;
use crate::game_sync::{GameMessage, ShipState};
use crate::net_bridge::NetBridge;
use crate::sounds::SfxTrigger;
use crate::Session;
use bevy::prelude::*;
use super::types::*;

pub fn collect_local_input(keys: &ButtonInput<KeyCode>) -> (i8, i8, bool) {
    // Sideways only (Astroblast-style): ←/→ or A/D.
    let mut move_x: i8 = 0;
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        move_x -= 1;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        move_x += 1;
    }
    let fire = keys.just_pressed(KeyCode::Space);
    (move_x, 0, fire)
}

pub(crate) fn integrate_ship(x: f32, y: f32, move_x: i8, _move_y: i8, dt: f32) -> (f32, f32) {
    let nx = (x + move_x as f32 * SHIP_SPEED * dt).clamp(-PLAY_AREA_X, PLAY_AREA_X);
    (nx, y)
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
                    ship.move_y = 0;
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
        super::send_game(
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
    mut sfx: EventWriter<SfxTrigger>,
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
        sfx.send(SfxTrigger::Shot);
    }
    for bullet in &mut prediction.local_bullets {
        bullet.y += BULLET_SPEED * dt;
    }
    prediction.local_bullets.retain(|b| b.y < 400.0);
}
pub fn playing_client_local_hits(
    session: Res<Session>,
    bridge: Res<NetBridge>,
    mut latest: ResMut<LatestState>,
    mut prediction: ResMut<LocalPrediction>,
    mut sfx: EventWriter<SfxTrigger>,
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
        super::send_game(&bridge, &session, &GameMessage::Hit { asteroid_id });
        sfx.send(SfxTrigger::Destroy);
    }
}
pub(crate) fn reconcile_prediction(prediction: &mut LocalPrediction, auth: &ShipState) {
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
