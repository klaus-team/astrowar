use crate::board::PLAY_AREA_X;
use crate::game_sync::{AsteroidKind, ShipState};
use super::types::{HostSim, SimAsteroid, SimShip};
use std::collections::HashMap;

pub(crate) const PHASE_SCORE_STEP: i32 = 500;
pub(crate) const MAX_PHASE_EFFECT: u8 = 6;
const SPEED_FAST_MUL: f32 = 1.35;
const ZIGZAG_VX_FAST_MUL: f32 = 1.25;

pub(crate) struct PhaseMods {
    pub(crate) weight_large: f32,
    pub(crate) weight_small: f32,
    pub(crate) weight_zigzag: f32,
    pub(crate) weight_spinner: f32,
    pub(crate) large_small_fast: bool,
    pub(crate) zigzag_fast: bool,
    pub(crate) spinner_fast: bool,
    pub(crate) spawn_interval_mul: f32,
}

pub(crate) fn leading_score_sim(ships: &HashMap<String, SimShip>) -> i32 {
    ships
        .values()
        .filter(|s| !s.forfeited)
        .map(|s| s.score)
        .max()
        .unwrap_or(0)
}

pub(crate) fn leading_score_state(ships: &[ShipState]) -> i32 {
    ships
        .iter()
        .filter(|s| !s.forfeited)
        .map(|s| s.score)
        .max()
        .unwrap_or(0)
}

pub(crate) fn phase_from_score(score: i32) -> u8 {
    let score = score.max(0);
    // Saturating add so huge endless scores still display a phase number.
    (1u32.saturating_add((score as u32) / PHASE_SCORE_STEP as u32)).min(u32::from(u8::MAX)) as u8
}

pub(crate) fn phase_mods(phase: u8) -> PhaseMods {
    // Baseline weights ≈ current threshold shares: L40 / S32 / Z16 / Sp12.
    let effect = phase.min(MAX_PHASE_EFFECT);
    let mut mods = PhaseMods {
        weight_large: 40.0,
        weight_small: 32.0,
        weight_zigzag: 16.0,
        weight_spinner: 12.0,
        large_small_fast: false,
        zigzag_fast: false,
        spinner_fast: false,
        spawn_interval_mul: 1.0,
    };
    if effect >= 2 {
        mods.weight_large = 52.0;
        mods.weight_small = 40.0;
    }
    if effect >= 3 {
        mods.large_small_fast = true;
    }
    if effect >= 4 {
        mods.weight_zigzag = 28.0;
    }
    if effect >= 5 {
        mods.zigzag_fast = true;
    }
    if effect >= 6 {
        mods.weight_spinner = 20.0;
        mods.spawn_interval_mul = 0.85;
    }
    mods
}
pub(crate) fn spawn_asteroid(host: &mut HostSim, mods: &PhaseMods) {
    let id = host.next_asteroid_id;
    host.next_asteroid_id = host.next_asteroid_id.wrapping_add(1);
    let x = pseudo_rand(id) * PLAY_AREA_X * 2.0 - PLAY_AREA_X;
    let kind = pick_asteroid_kind(id, mods);
    let (radius, points, mut speed, mut vx) = match kind {
        AsteroidKind::Spinner => (
            14.0,
            40,
            150.0 + pseudo_rand(id.wrapping_mul(11)) * 40.0,
            0.0,
        ),
        AsteroidKind::Zigzag => {
            let dir = if pseudo_rand(id.wrapping_mul(5)) > 0.5 {
                1.0
            } else {
                -1.0
            };
            (
                13.0,
                80,
                120.0 + pseudo_rand(id.wrapping_mul(13)) * 35.0,
                dir * (140.0 + pseudo_rand(id.wrapping_mul(17)) * 60.0),
            )
        }
        AsteroidKind::Small => (
            12.0,
            20,
            160.0 + pseudo_rand(id.wrapping_mul(7)) * 50.0,
            0.0,
        ),
        AsteroidKind::Large => (
            28.0,
            10,
            90.0 + pseudo_rand(id.wrapping_mul(7)) * 35.0,
            0.0,
        ),
    };

    let fast = match kind {
        AsteroidKind::Large | AsteroidKind::Small => mods.large_small_fast,
        AsteroidKind::Zigzag => mods.zigzag_fast,
        AsteroidKind::Spinner => mods.spinner_fast,
    };
    if fast {
        speed *= SPEED_FAST_MUL;
        if kind == AsteroidKind::Zigzag {
            vx *= ZIGZAG_VX_FAST_MUL;
        }
    }

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

pub(crate) fn pick_asteroid_kind(id: u32, mods: &PhaseMods) -> AsteroidKind {
    let total = mods.weight_large
        + mods.weight_small
        + mods.weight_zigzag
        + mods.weight_spinner;
    let mut roll = pseudo_rand(id.wrapping_mul(3)) * total;
    if roll < mods.weight_large {
        return AsteroidKind::Large;
    }
    roll -= mods.weight_large;
    if roll < mods.weight_small {
        return AsteroidKind::Small;
    }
    roll -= mods.weight_small;
    if roll < mods.weight_zigzag {
        return AsteroidKind::Zigzag;
    }
    AsteroidKind::Spinner
}
pub(crate) fn pseudo_rand(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    x ^= x >> 16;
    (x & 0xffff) as f32 / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_from_score_steps_every_500() {
        assert_eq!(phase_from_score(0), 1);
        assert_eq!(phase_from_score(499), 1);
        assert_eq!(phase_from_score(500), 2);
        assert_eq!(phase_from_score(2999), 6);
        assert_eq!(phase_from_score(3000), 7);
    }

    #[test]
    fn phase_mods_clamp_effects_at_six() {
        let p6 = phase_mods(6);
        let p7 = phase_mods(7);
        assert_eq!(p6.spawn_interval_mul, 0.85);
        assert_eq!(p7.spawn_interval_mul, p6.spawn_interval_mul);
        assert_eq!(p6.weight_spinner, p7.weight_spinner);

        assert!(!phase_mods(2).large_small_fast);
        assert!(phase_mods(3).large_small_fast);
        assert!(!phase_mods(4).zigzag_fast);
        assert!(phase_mods(5).zigzag_fast);
    }

    #[test]
    fn pick_asteroid_kind_respects_forced_weights() {
        let only_large = PhaseMods {
            weight_large: 1.0,
            weight_small: 0.0,
            weight_zigzag: 0.0,
            weight_spinner: 0.0,
            large_small_fast: false,
            zigzag_fast: false,
            spinner_fast: false,
            spawn_interval_mul: 1.0,
        };
        let only_spinner = PhaseMods {
            weight_large: 0.0,
            weight_small: 0.0,
            weight_zigzag: 0.0,
            weight_spinner: 1.0,
            large_small_fast: false,
            zigzag_fast: false,
            spinner_fast: false,
            spawn_interval_mul: 1.0,
        };
        for seed in 0..64 {
            assert_eq!(pick_asteroid_kind(seed, &only_large), AsteroidKind::Large);
            assert_eq!(pick_asteroid_kind(seed, &only_spinner), AsteroidKind::Spinner);
        }
    }
}
