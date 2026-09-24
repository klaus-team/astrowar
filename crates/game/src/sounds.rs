//! Procedural SFX bank (WAV in memory — no asset files required).

use bevy::audio::{AudioPlayer, AudioSource, GlobalVolume, PlaybackSettings, Volume};
use bevy::prelude::*;
use std::sync::Arc;
use std::time::Duration;

const SAMPLE_RATE: u32 = 22_050;

#[derive(Resource, Clone)]
pub struct SoundBank {
    pub shot: Handle<AudioSource>,
    pub destroy: Handle<AudioSource>,
    pub spinner: Handle<AudioSource>,
    pub shock: Handle<AudioSource>,
}

#[derive(Event, Clone, Copy, Debug)]
pub enum SfxTrigger {
    Shot,
    Destroy,
}

#[derive(Resource)]
pub struct SpinnerAlert {
    active: bool,
    timer: Timer,
}

impl Default for SpinnerAlert {
    fn default() -> Self {
        Self {
            active: false,
            timer: Timer::new(Duration::from_millis(280), TimerMode::Repeating),
        }
    }
}

#[derive(Resource, Default)]
pub struct AudioMuted(pub bool);

pub fn setup_sounds(mut commands: Commands, mut audio: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(SoundBank {
        shot: audio.add(make_shot()),
        destroy: audio.add(make_destroy()),
        spinner: audio.add(make_spinner_pulse()),
        shock: audio.add(make_shock()),
    });
    commands.init_resource::<SpinnerAlert>();
    commands.init_resource::<AudioMuted>();
}

/// Toggle mute for every sound in the game (`M`), including menu confirms.
pub fn toggle_mute_audio(
    keys: Res<ButtonInput<KeyCode>>,
    mut muted: ResMut<AudioMuted>,
    mut global_volume: ResMut<GlobalVolume>,
    playing: Query<Entity, With<AudioPlayer>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    muted.0 = !muted.0;
    *global_volume = if muted.0 {
        GlobalVolume {
            volume: Volume::ZERO,
        }
    } else {
        GlobalVolume::new(1.0)
    };
    if muted.0 {
        for entity in &playing {
            commands.entity(entity).despawn();
        }
    }
}

/// Emit a oneshot SFX unless the game is muted.
pub fn emit_sfx(sfx: &mut EventWriter<SfxTrigger>, muted: &AudioMuted, trigger: SfxTrigger) {
    if !muted.0 {
        sfx.send(trigger);
    }
}

pub fn play_sfx_triggers(
    mut commands: Commands,
    mut events: EventReader<SfxTrigger>,
    bank: Res<SoundBank>,
    muted: Res<AudioMuted>,
) {
    if muted.0 {
        for _ in events.read() {}
        return;
    }
    for trigger in events.read() {
        match trigger {
            SfxTrigger::Shot => {
                spawn_oneshot(&mut commands, &bank.shot, 0.32);
            }
            SfxTrigger::Destroy => {
                spawn_oneshot(&mut commands, &bank.destroy, 0.5);
            }
        }
    }
}

/// Keep a warning pulse while any spinner asteroid is on screen.
/// (Oneshoot pulses — Bevy LOOP + WAV decoder is unreliable for continuous hum.)
pub fn sync_spinner_hum(
    mut commands: Commands,
    bank: Res<SoundBank>,
    latest: Res<crate::playing::LatestState>,
    time: Res<Time>,
    muted: Res<AudioMuted>,
    mut alert: ResMut<SpinnerAlert>,
) {
    let wants = latest
        .asteroids
        .iter()
        .any(|a| a.kind == crate::game_sync::AsteroidKind::Spinner);

    if !wants {
        alert.active = false;
        alert.timer.reset();
        return;
    }

    if muted.0 {
        return;
    }

    if !alert.active {
        alert.active = true;
        alert.timer.reset();
        spawn_oneshot(&mut commands, &bank.spinner, 0.42);
        return;
    }

    alert.timer.tick(time.delta());
    if alert.timer.just_finished() {
        spawn_oneshot(&mut commands, &bank.spinner, 0.42);
    }
}

fn spawn_oneshot(commands: &mut Commands, source: &Handle<AudioSource>, volume: f32) {
    commands.spawn((
        AudioPlayer::new(source.clone()),
        PlaybackSettings::DESPAWN.with_volume(Volume::new(volume)),
    ));
}

fn make_shot() -> AudioSource {
    // Soft, short blip — also used for menu confirmation.
    let n = (SAMPLE_RATE as f32 * 0.055) as usize;
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = (1.0 - t / 0.055).clamp(0.0, 1.0).powf(1.6);
        let freq = 980.0 - 220.0 * (t / 0.055);
        let s = (t * freq * std::f32::consts::TAU).sin() * env * 0.45;
        samples.push(s);
    }
    pcm_wav(&samples)
}

fn make_destroy() -> AudioSource {
    // Short crack: noise burst + descending tone (Astroblast-ish).
    let n = (SAMPLE_RATE as f32 * 0.14) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut noise = 0xA5A5_u32;
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = (1.0 - t / 0.14).clamp(0.0, 1.0).powf(1.2);
        noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
        let nsample = ((noise >> 16) as i16 as f32) / i16::MAX as f32;
        let freq = 420.0 - 280.0 * (t / 0.14);
        let tone = (t * freq * std::f32::consts::TAU).sin();
        samples.push((nsample * 0.55 + tone * 0.45) * env * 0.7);
    }
    pcm_wav(&samples)
}

fn make_spinner_pulse() -> AudioSource {
    // Distinct warning chirp so the spinning hazard is obvious.
    let n = (SAMPLE_RATE as f32 * 0.12) as usize;
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = (1.0 - t / 0.12).clamp(0.0, 1.0).powf(0.8);
        let freq = 520.0 + 380.0 * (t / 0.12);
        let s = (t * freq * std::f32::consts::TAU).sin() * env * 0.65;
        samples.push(s);
    }
    pcm_wav(&samples)
}

fn make_shock() -> AudioSource {
    // Loud electric zap — high buzz + noise snap so it cuts through other SFX.
    let n = (SAMPLE_RATE as f32 * 0.22) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut noise = 0xBADC0DE_u32;
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let env = if t < 0.02 {
            t / 0.02
        } else {
            (1.0 - (t - 0.02) / 0.20).clamp(0.0, 1.0).powf(0.7)
        };
        noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
        let nsample = ((noise >> 16) as i16 as f32) / i16::MAX as f32;
        let buzz = (t * 1800.0 * std::f32::consts::TAU).sin()
            * (t * 55.0 * std::f32::consts::TAU).sin().abs();
        let snap = (t * 320.0 * std::f32::consts::TAU).sin() * (1.0 - t / 0.08).clamp(0.0, 1.0);
        samples.push((nsample * 0.45 + buzz * 0.55 + snap * 0.35) * env * 0.95);
    }
    pcm_wav(&samples)
}

fn pcm_wav(samples: &[f32]) -> AudioSource {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    let data_len = pcm.len() as u32;
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&pcm);
    AudioSource {
        bytes: Arc::<[u8]>::from(out),
    }
}
