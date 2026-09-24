use crate::board::PLAY_AREA_X;
use crate::game_sync::AsteroidKind;
use crate::highscores::HighScores;
use crate::shapes::ShapeMeshes;
use crate::sounds::{AudioMuted, SoundBank};
use crate::Session;
use bevy::audio::{AudioPlayer, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use super::phase::{leading_score_state, phase_from_score};
use super::sync::interpolate_ships;
use super::types::*;

pub fn watch_hurt_flash(
    mut commands: Commands,
    time: Res<Time>,
    session: Res<Session>,
    latest: Res<LatestState>,
    bank: Res<SoundBank>,
    muted: Res<AudioMuted>,
    mut hurt: ResMut<HurtFlash>,
) {
    if hurt.remaining > 0.0 {
        hurt.remaining = (hurt.remaining - time.delta_secs()).max(0.0);
    }

    let Some(local_id) = session.player_id.as_deref() else {
        return;
    };
    let Some(ship) = latest
        .to_ships
        .iter()
        .find(|s| s.player_id == local_id)
        .or_else(|| latest.from_ships.iter().find(|s| s.player_id == local_id))
    else {
        return;
    };

    match hurt.last_lives {
        Some(prev) if ship.lives < prev => {
            hurt.trigger();
            // Play immediately here — don't rely on SfxTrigger event ordering.
            if !muted.0 {
                commands.spawn((
                    AudioPlayer::new(bank.shock.clone()),
                    PlaybackSettings::DESPAWN.with_volume(Volume::new(0.9)),
                ));
            }
        }
        _ => {}
    }
    hurt.last_lives = Some(ship.lives);
}

pub fn spawn_playing_hud(
    mut commands: Commands,
    _session: Res<Session>,
    mut high_scores: ResMut<HighScores>,
    mut match_over_return: ResMut<MatchOverReturn>,
    mut hurt_flash: ResMut<HurtFlash>,
) {
    high_scores.reset_match_flag();
    match_over_return.reset();
    hurt_flash.reset();
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
pub fn sync_world_sprites(
    mut commands: Commands,
    latest: Res<LatestState>,
    prediction: Res<LocalPrediction>,
    shapes: Res<ShapeMeshes>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
    mut ships: Query<(
        Entity,
        &ShipSprite,
        &mut Transform,
        &MeshMaterial2d<ColorMaterial>,
    ), (
        Without<BulletSprite>,
        Without<LocalBulletSprite>,
        Without<AsteroidSprite>,
    )>,
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
    mut asteroids: Query<(
        Entity,
        &AsteroidSprite,
        &mut Transform,
        &MeshMaterial2d<ColorMaterial>,
    ), (
        Without<ShipSprite>,
        Without<BulletSprite>,
        Without<LocalBulletSprite>,
    )>,
    mut hud: Query<(&HudSlot, &mut Text), With<PlayingHud>>,
    session: Res<Session>,
    match_over_return: Res<MatchOverReturn>,
    hurt_flash: Res<HurtFlash>,
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

    let local_id = session.player_id.as_deref();
    let blink_hide = !hurt_flash.visible();
    let mut seen_ships = Vec::new();
    for ship in &rendered_ships {
        seen_ships.push(ship.player_id.clone());
        let is_local = local_id.is_some_and(|id| ship.player_id == id);
        let mut color = color_for_id(&ship.player_id);
        if !ship.alive || ship.forfeited {
            color = Color::srgba(0.35, 0.35, 0.4, 0.45);
        }
        if is_local && blink_hide && ship.alive && !ship.forfeited {
            color = Color::srgba(0.0, 0.0, 0.0, 0.0);
        }
        if let Some((_, _, mut transform, material)) = ships
            .iter_mut()
            .find(|(_, marker, _, _)| marker.player_id == ship.player_id)
        {
            transform.translation.x = ship.x;
            transform.translation.y = ship.y;
            if let Some(mat) = materials.get_mut(&material.0) {
                mat.color = color;
            }
        } else {
            let material = materials.add(ColorMaterial::from_color(color));
            commands.spawn((
                ShipSprite {
                    player_id: ship.player_id.clone(),
                },
                Mesh2d(shapes.ship.clone()),
                MeshMaterial2d(material),
                Transform::from_xyz(ship.x, ship.y, 2.0),
            ));
        }
    }
    for (entity, marker, _, _) in &ships {
        if !seen_ships.iter().any(|id| id == &marker.player_id) {
            commands.entity(entity).despawn();
        }
    }

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
        let color = color_for_asteroid(asteroid.kind);
        let spin = if asteroid.kind == AsteroidKind::Spinner {
            Quat::from_rotation_z(time.elapsed_secs() * 2.4 + asteroid.id as f32 * 0.7)
        } else {
            Quat::IDENTITY
        };
        if let Some((_, _, mut transform, material)) = asteroids
            .iter_mut()
            .find(|(_, marker, _, _)| marker.id == asteroid.id)
        {
            transform.translation.x = asteroid.x;
            transform.translation.y = asteroid.y;
            transform.rotation = spin;
            transform.scale = Vec3::splat(asteroid.radius);
            if let Some(mat) = materials.get_mut(&material.0) {
                mat.color = color;
            }
        } else {
            let material = materials.add(ColorMaterial::from_color(color));
            commands.spawn((
                AsteroidSprite {
                    id: asteroid.id,
                    kind: asteroid.kind,
                },
                Mesh2d(shapes.for_asteroid(asteroid.kind)),
                MeshMaterial2d(material),
                Transform::from_xyz(asteroid.x, asteroid.y, 1.0)
                    .with_rotation(spin)
                    .with_scale(Vec3::splat(asteroid.radius)),
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
    let phase = phase_from_score(leading_score_state(&rendered_ships));
    let mode = format!("Phase {phase}\n{mode}");

    let leave = if latest.match_over {
        match match_over_return.seconds_left() {
            Some(secs) => format!("[Esc] Leave · menu in {secs}s"),
            None => "[Esc] Leave".to_string(),
        }
    } else {
        "[Esc] Leave".to_string()
    };

    for (slot, mut text) in &mut hud {
        *text = Text::new(match slot {
            HudSlot::Title => title.clone(),
            HudSlot::Players => board.clone(),
            HudSlot::Mode => mode.clone(),
            HudSlot::Leave => leave.clone(),
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
        AsteroidKind::Large => Color::srgb(0.55, 0.32, 0.18),
        AsteroidKind::Small => Color::srgb(0.95, 0.55, 0.18),
        AsteroidKind::Spinner => Color::srgb(0.95, 0.18, 0.16),
        AsteroidKind::Zigzag => Color::srgb(0.95, 0.82, 0.22),
    }
}
