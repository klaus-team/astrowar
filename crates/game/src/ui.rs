use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use protocol::GameDurationMinutes;

use crate::highscores::HighScores;
use crate::net_bridge::{NetBridge, NetCommand};
use crate::nickname::{resolve_or_default, save_last_nickname};
use crate::playing::{HostSim, LatestState, LocalPrediction};
use crate::sounds::{emit_sfx, AudioMuted, SfxTrigger};
use crate::{
    start_solo_match, AppState, ClientSettings, ConnectIntent, HostForm, JoinForm, Session,
    StatusMessage,
};

#[derive(Component)]
pub(crate) struct UiRoot;

#[derive(Component)]
pub(crate) struct DynamicText;

fn spawn_screen(commands: &mut Commands, title: &str, body: String) {
    let title_color = if title.eq_ignore_ascii_case("ASTROWAR") {
        Color::srgb(1.0, 1.0, 1.0)
    } else {
        Color::srgb(0.85, 0.9, 1.0)
    };
    commands
        .spawn((
            UiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(title.to_string()),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(title_color),
            ));
            parent.spawn((
                DynamicText,
                Text::new(body),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.75, 0.85)),
            ));
        });
}

/// Show only host/domain from a WebSocket URL (no scheme, port, or path).
fn server_host_label(url: &str) -> String {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("ws://")
        .or_else(|| trimmed.strip_prefix("wss://"))
        .or_else(|| trimmed.strip_prefix("http://"))
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    let hostport = without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .trim();
    if hostport.is_empty() {
        return trimmed.to_string();
    }
    if let Some(rest) = hostport.strip_prefix('[') {
        // IPv6: [2001:db8::1]:8080
        return rest.split(']').next().unwrap_or(rest).to_string();
    }
    match hostport.rfind(':') {
        Some(i) if hostport[i + 1..].chars().all(|c| c.is_ascii_digit()) => {
            hostport[..i].to_string()
        }
        _ => hostport.to_string(),
    }
}

pub(crate) fn spawn_main_menu(
    mut commands: Commands,
    status: Res<StatusMessage>,
    high_scores: Res<HighScores>,
    muted: Res<AudioMuted>,
) {
    spawn_screen(
        &mut commands,
        "ASTROWAR",
        main_menu_body(&status, &high_scores, &muted),
    );
}

fn main_menu_body(status: &StatusMessage, high_scores: &HighScores, muted: &AudioMuted) -> String {
    let status_line = if status.text.is_empty() {
        String::new()
    } else {
        format!("\nLast status: {}\n", status.text)
    };
    let scores = high_scores.menu_block();
    // [on] = sound audible, [off] = muted
    let mute_toggle = if muted.0 { "[off]" } else { "[on]" };
    format!(
        "{status_line}[1] Solo (offline)\n[2] Host internet room\n[3] Join with code\n\n[M] Mute {mute_toggle}\n[Esc] Quit{scores}"
    )
}

pub(crate) fn spawn_solo_setup(
    mut commands: Commands,
    form: Res<HostForm>,
    status: Res<StatusMessage>,
) {
    spawn_screen(
        &mut commands,
        "Solo play",
        solo_setup_body(&form, &status),
    );
}

pub(crate) fn spawn_host_setup(
    mut commands: Commands,
    settings: Res<ClientSettings>,
    form: Res<HostForm>,
    status: Res<StatusMessage>,
) {
    spawn_screen(
        &mut commands,
        "Host room",
        host_setup_body(&settings, &form, &status),
    );
}

pub(crate) fn spawn_join_setup(
    mut commands: Commands,
    settings: Res<ClientSettings>,
    form: Res<JoinForm>,
    status: Res<StatusMessage>,
) {
    spawn_screen(
        &mut commands,
        "Join room",
        join_setup_body(&settings, &form, &status),
    );
}

pub(crate) fn spawn_connecting(mut commands: Commands, status: Res<StatusMessage>) {
    let detail = if status.text.is_empty() {
        "Connecting to relay...".to_string()
    } else {
        status.text.clone()
    };
    spawn_screen(
        &mut commands,
        "Connecting",
        format!("{detail}\n\n[Esc] Cancel"),
    );
}

pub(crate) fn spawn_lobby(
    mut commands: Commands,
    session: Res<Session>,
    status: Res<StatusMessage>,
) {
    spawn_screen(&mut commands, "Lobby", lobby_body(&session, &status));
}

fn solo_setup_body(form: &HostForm, status: &StatusMessage) -> String {
    let nick_mark = if form.focus_nickname { ">" } else { " " };
    let dur_mark = if form.focus_nickname { " " } else { ">" };
    let status_line = if status.text.is_empty() {
        String::new()
    } else {
        format!("\nStatus: {}\n", status.text)
    };
    format!(
        "Offline — no server needed{status_line}\n{nick_mark} Nickname: {}\n{dur_mark} Duration: {}\n\n[Tab] Switch field\n[Left/Right] Change duration\n[Enter] Start\n[Esc] Back",
        form.nickname,
        form.solo_duration().label(),
    )
}

fn host_setup_body(
    settings: &ClientSettings,
    form: &HostForm,
    status: &StatusMessage,
) -> String {
    let nick_mark = if form.focus_nickname { ">" } else { " " };
    let dur_mark = if form.focus_nickname { " " } else { ">" };
    let status_line = if status.text.is_empty() {
        String::new()
    } else {
        format!("\nStatus: {}", status.text)
    };
    format!(
        "Server: {}{status_line}\n\n{nick_mark} Nickname: {}\n{dur_mark} Duration: {}\n\n[Tab] Switch field\n[Left/Right] Change duration\n[Enter] Create room\n[Esc] Back",
        server_host_label(&settings.server_url),
        form.nickname,
        form.duration().label(),
    )
}

fn join_setup_body(
    settings: &ClientSettings,
    form: &JoinForm,
    status: &StatusMessage,
) -> String {
    let nick_mark = if form.focus_nickname { ">" } else { " " };
    let code_mark = if form.focus_nickname { " " } else { ">" };
    let status_line = if status.text.is_empty() {
        String::new()
    } else {
        format!("\nStatus: {}", status.text)
    };
    format!(
        "Server: {}{status_line}\n\n{nick_mark} Nickname: {}\n{code_mark} Room code: {}\n\nNicknames must be unique in the room.\n[Tab] Switch field\n[Enter] Join room\n[Esc] Back",
        server_host_label(&settings.server_url),
        form.nickname,
        form.code,
    )
}

fn lobby_body(session: &Session, status: &StatusMessage) -> String {
    let Some(room) = &session.room else {
        return "Waiting for room data...\n\n[Esc] Leave".into();
    };
    let players = room
        .players
        .iter()
        .map(|player| {
            let owner = if player.id == room.owner_id {
                " (owner)"
            } else {
                ""
            };
            format!("- {}{owner}", player.nickname)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let owner_help = if session.is_owner {
        "[Enter] Start match (even alone)\n"
    } else {
        "Waiting for owner to start...\n"
    };
    let status_line = if status.text.is_empty() {
        String::new()
    } else {
        format!("Status: {}\n\n", status.text)
    };
    format!(
        "{status_line}Code: {}\nPhase: {:?}\nDuration: {}\nPlayers ({}/{}):\n{}\n\n{owner_help}[Esc] Leave",
        room.code,
        room.phase,
        room.duration_minutes.label(),
        room.players.len(),
        room.max_players,
        players,
    )
}

pub(crate) fn cleanup_ui_root(mut commands: Commands, query: Query<Entity, With<UiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn set_dynamic_text(query: &mut Query<&mut Text, With<DynamicText>>, body: String) {
    for mut text in query.iter_mut() {
        *text = Text::new(body.clone());
    }
}

pub(crate) fn refresh_main_menu_ui(
    muted: Res<AudioMuted>,
    status: Res<StatusMessage>,
    high_scores: Res<HighScores>,
    mut query: Query<&mut Text, With<DynamicText>>,
) {
    if !(muted.is_changed() || status.is_changed() || high_scores.is_changed()) {
        return;
    }
    set_dynamic_text(&mut query, main_menu_body(&status, &high_scores, &muted));
}

pub(crate) fn refresh_solo_setup_ui(
    form: Res<HostForm>,
    status: Res<StatusMessage>,
    mut query: Query<&mut Text, With<DynamicText>>,
) {
    if !(form.is_changed() || status.is_changed()) {
        return;
    }
    set_dynamic_text(&mut query, solo_setup_body(&form, &status));
}

pub(crate) fn refresh_host_setup_ui(
    settings: Res<ClientSettings>,
    form: Res<HostForm>,
    status: Res<StatusMessage>,
    mut query: Query<&mut Text, With<DynamicText>>,
) {
    if !(form.is_changed() || status.is_changed() || settings.is_changed()) {
        return;
    }
    set_dynamic_text(&mut query, host_setup_body(&settings, &form, &status));
}

pub(crate) fn refresh_join_setup_ui(
    settings: Res<ClientSettings>,
    form: Res<JoinForm>,
    status: Res<StatusMessage>,
    mut query: Query<&mut Text, With<DynamicText>>,
) {
    if !(form.is_changed() || status.is_changed() || settings.is_changed()) {
        return;
    }
    set_dynamic_text(&mut query, join_setup_body(&settings, &form, &status));
}

pub(crate) fn refresh_lobby_ui(
    session: Res<Session>,
    status: Res<StatusMessage>,
    mut query: Query<&mut Text, With<DynamicText>>,
) {
    if !(session.is_changed() || status.is_changed()) {
        return;
    }
    set_dynamic_text(&mut query, lobby_body(&session, &status));
}

pub(crate) fn handle_main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
    mut status: ResMut<StatusMessage>,
    mut host_form: ResMut<HostForm>,
    mut join_form: ResMut<JoinForm>,
    mut intent: ResMut<ConnectIntent>,
    settings: Res<ClientSettings>,
    muted: Res<AudioMuted>,
    mut sfx: EventWriter<SfxTrigger>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        status.text.clear();
        *intent = ConnectIntent::None;
        // Prefill only when a saved nick exists; otherwise keep blank.
        host_form.nickname = settings.nickname.clone();
        host_form.focus_nickname = true;
        emit_sfx(&mut sfx, &muted, SfxTrigger::Shot);
        next_state.set(AppState::SoloSetup);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        status.text.clear();
        *intent = ConnectIntent::Host;
        host_form.nickname = settings.nickname.clone();
        host_form.duration_index %= GameDurationMinutes::ALL.len();
        host_form.focus_nickname = true;
        emit_sfx(&mut sfx, &muted, SfxTrigger::Shot);
        next_state.set(AppState::HostSetup);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        status.text.clear();
        *intent = ConnectIntent::Join;
        join_form.nickname = settings.nickname.clone();
        join_form.code.clear();
        join_form.focus_nickname = true;
        emit_sfx(&mut sfx, &muted, SfxTrigger::Shot);
        next_state.set(AppState::JoinSetup);
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }
}

pub(crate) fn handle_solo_setup_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mut form: ResMut<HostForm>,
    mut settings: ResMut<ClientSettings>,
    mut status: ResMut<StatusMessage>,
    mut session: ResMut<Session>,
    mut host: ResMut<HostSim>,
    mut latest: ResMut<LatestState>,
    mut prediction: ResMut<LocalPrediction>,
    mut next_state: ResMut<NextState<AppState>>,
    muted: Res<AudioMuted>,
    mut sfx: EventWriter<SfxTrigger>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::MainMenu);
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        form.focus_nickname = !form.focus_nickname;
    }
    if !form.focus_nickname {
        if keys.just_pressed(KeyCode::ArrowLeft) {
            let len = GameDurationMinutes::SOLO.len();
            form.duration_index = (form.duration_index + len - 1) % len;
        }
        if keys.just_pressed(KeyCode::ArrowRight) {
            form.duration_index = (form.duration_index + 1) % GameDurationMinutes::SOLO.len();
        }
    }
    for event in key_events.read() {
        if !event.state.is_pressed() || event.repeat {
            continue;
        }
        if form.focus_nickname {
            apply_text_edit(&mut form.nickname, &event.logical_key, 24, false);
        }
    }
    if keys.just_pressed(KeyCode::Enter) {
        let nickname = resolve_or_default(&form.nickname);
        settings.nickname = nickname.clone();
        save_last_nickname(&nickname);
        status.text.clear();
        emit_sfx(&mut sfx, &muted, SfxTrigger::Shot);
        start_solo_match(
            &mut session,
            &mut host,
            &mut latest,
            &mut prediction,
            nickname,
            form.solo_duration(),
        );
        next_state.set(AppState::Playing);
    }
}

pub(crate) fn handle_host_setup_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mut form: ResMut<HostForm>,
    mut settings: ResMut<ClientSettings>,
    mut status: ResMut<StatusMessage>,
    mut session: ResMut<Session>,
    mut intent: ResMut<ConnectIntent>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
    muted: Res<AudioMuted>,
    mut sfx: EventWriter<SfxTrigger>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        *intent = ConnectIntent::None;
        next_state.set(AppState::MainMenu);
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        form.focus_nickname = !form.focus_nickname;
    }
    if !form.focus_nickname {
        if keys.just_pressed(KeyCode::ArrowLeft) {
            let len = GameDurationMinutes::ALL.len();
            form.duration_index = (form.duration_index + len - 1) % len;
        }
        if keys.just_pressed(KeyCode::ArrowRight) {
            form.duration_index = (form.duration_index + 1) % GameDurationMinutes::ALL.len();
        }
    }
    for event in key_events.read() {
        if !event.state.is_pressed() || event.repeat {
            continue;
        }
        if form.focus_nickname {
            apply_text_edit(&mut form.nickname, &event.logical_key, 24, false);
        }
    }
    if keys.just_pressed(KeyCode::Enter) {
        let nickname = resolve_or_default(&form.nickname);
        settings.nickname = nickname.clone();
        save_last_nickname(&nickname);
        session.accept_connection = true;
        *intent = ConnectIntent::Host;
        status.text = "Creating room...".into();
        emit_sfx(&mut sfx, &muted, SfxTrigger::Shot);
        bridge.send(NetCommand::ConnectAndCreate {
            url: settings.server_url.clone(),
            nickname,
            duration: form.duration(),
        });
        next_state.set(AppState::Connecting);
    }
}

pub(crate) fn handle_join_setup_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mut form: ResMut<JoinForm>,
    mut settings: ResMut<ClientSettings>,
    mut status: ResMut<StatusMessage>,
    mut session: ResMut<Session>,
    mut intent: ResMut<ConnectIntent>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
    muted: Res<AudioMuted>,
    mut sfx: EventWriter<SfxTrigger>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        *intent = ConnectIntent::None;
        next_state.set(AppState::MainMenu);
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        form.focus_nickname = !form.focus_nickname;
    }
    for event in key_events.read() {
        if !event.state.is_pressed() || event.repeat {
            continue;
        }
        if form.focus_nickname {
            apply_text_edit(&mut form.nickname, &event.logical_key, 24, false);
        } else {
            apply_text_edit(&mut form.code, &event.logical_key, 8, true);
        }
    }
    if keys.just_pressed(KeyCode::Enter) {
        let nickname = form.nickname.trim().to_string();
        let code = form.code.trim().to_uppercase();
        if code.len() < 4 {
            status.text = "Room code is required".into();
            return;
        }
        // Empty → server assigns Player / Player2 / …
        if !nickname.is_empty() {
            settings.nickname = nickname.clone();
            save_last_nickname(&nickname);
        }
        session.accept_connection = true;
        *intent = ConnectIntent::Join;
        status.text = format!("Joining {code}...");
        emit_sfx(&mut sfx, &muted, SfxTrigger::Shot);
        bridge.send(NetCommand::ConnectAndJoin {
            url: settings.server_url.clone(),
            nickname,
            code,
        });
        next_state.set(AppState::Connecting);
    }
}

pub(crate) fn handle_connecting_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut status: ResMut<StatusMessage>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        session.accept_connection = false;
        session.player_id = None;
        session.room = None;
        session.is_owner = false;
        session.solo = false;
        status.text = "Connection cancelled".into();
        bridge.send(NetCommand::Leave);
        next_state.set(AppState::MainMenu);
    }
}

pub(crate) fn handle_lobby_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut status: ResMut<StatusMessage>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
    muted: Res<AudioMuted>,
    mut sfx: EventWriter<SfxTrigger>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        session.accept_connection = false;
        status.text.clear();
        bridge.send(NetCommand::Leave);
        session.player_id = None;
        session.room = None;
        session.is_owner = false;
        session.solo = false;
        next_state.set(AppState::MainMenu);
        return;
    }
    if keys.just_pressed(KeyCode::Enter) && session.is_owner {
        status.text = "Starting match...".into();
        emit_sfx(&mut sfx, &muted, SfxTrigger::Shot);
        bridge.send(NetCommand::StartGame);
    }
}

fn apply_text_edit(buffer: &mut String, key: &Key, max_len: usize, uppercase_alnum: bool) {
    match key {
        Key::Backspace => {
            buffer.pop();
        }
        Key::Space if !uppercase_alnum => {
            if buffer.len() < max_len {
                buffer.push(' ');
            }
        }
        Key::Character(text) => {
            for ch in text.chars() {
                if buffer.len() >= max_len {
                    break;
                }
                if uppercase_alnum {
                    if ch.is_ascii_alphanumeric() {
                        buffer.push(ch.to_ascii_uppercase());
                    }
                } else if !ch.is_control() {
                    buffer.push(ch);
                }
            }
        }
        _ => {}
    }
}
