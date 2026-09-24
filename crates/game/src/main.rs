mod board;
mod game_sync;
mod highscores;
mod net_bridge;
mod nickname;
mod playing;
mod shapes;
mod sounds;
mod storage;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use board::{setup_board_camera, update_board_viewport};
use game_sync::GameMessage;
use highscores::HighScores;
use net_bridge::{NetBridge, NetCommand, NetEvent};
use nickname::{load_last_nickname, resolve_or_default, save_last_nickname};
use shapes::setup_shape_meshes;
use sounds::{play_sfx_triggers, setup_sounds, sync_spinner_hum, SfxTrigger};
use playing::{
    advance_interpolation, begin_host_sim, cleanup_playing, handle_relayed_game_message,
    mark_player_forfeit, maybe_record_high_score, playing_client_local_hits,
    playing_host_simulate, playing_predict_local, playing_send_input, record_local_high_score,
    seed_ships_from_room, send_game, snapshot, spawn_playing_hud, sync_world_sprites, HostSim,
    InputThrottle, LatestState, LocalPrediction,
};
use protocol::{
    GameDurationMinutes, GameMode, PlayerInfo, RoomInfo, RoomPhase, MAX_PLAYERS,
};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.01, 0.02, 0.05)))
        .insert_resource(ClientSettings {
            server_url: net::default_server_url(),
            nickname: load_last_nickname().unwrap_or_default(),
        })
        .insert_resource(HostForm::default())
        .insert_resource(JoinForm::default())
        .insert_resource(Session::default())
        .insert_resource(StatusMessage::default())
        .insert_resource(ConnectIntent::default())
        .insert_resource(HostSim::default())
        .insert_resource(LatestState::default())
        .insert_resource(InputThrottle::default())
        .insert_resource(LocalPrediction::default())
        .insert_resource(HighScores::default())
        .add_event::<SfxTrigger>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "AstroWar".into(),
                resolution: (960.0_f32, 720.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_systems(
            Startup,
            (setup_board_camera, setup_net_bridge, setup_shape_meshes, setup_sounds),
        )
        .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
        .add_systems(OnEnter(AppState::SoloSetup), spawn_solo_setup)
        .add_systems(OnEnter(AppState::HostSetup), spawn_host_setup)
        .add_systems(OnEnter(AppState::JoinSetup), spawn_join_setup)
        .add_systems(OnEnter(AppState::Connecting), spawn_connecting)
        .add_systems(OnEnter(AppState::Lobby), spawn_lobby)
        .add_systems(OnEnter(AppState::Playing), spawn_playing_hud)
        .add_systems(OnExit(AppState::MainMenu), cleanup_ui_root)
        .add_systems(OnExit(AppState::SoloSetup), cleanup_ui_root)
        .add_systems(OnExit(AppState::HostSetup), cleanup_ui_root)
        .add_systems(OnExit(AppState::JoinSetup), cleanup_ui_root)
        .add_systems(OnExit(AppState::Connecting), cleanup_ui_root)
        .add_systems(OnExit(AppState::Lobby), cleanup_ui_root)
        .add_systems(OnExit(AppState::Playing), cleanup_playing)
        .add_systems(
            Update,
            (
                update_board_viewport,
                handle_main_menu_input.run_if(in_state(AppState::MainMenu)),
                handle_solo_setup_input.run_if(in_state(AppState::SoloSetup)),
                handle_host_setup_input.run_if(in_state(AppState::HostSetup)),
                handle_join_setup_input.run_if(in_state(AppState::JoinSetup)),
                handle_lobby_input.run_if(in_state(AppState::Lobby)),
                handle_connecting_input.run_if(in_state(AppState::Connecting)),
                handle_playing_input.run_if(in_state(AppState::Playing)),
                refresh_solo_setup_ui.run_if(in_state(AppState::SoloSetup)),
                refresh_host_setup_ui.run_if(in_state(AppState::HostSetup)),
                refresh_join_setup_ui.run_if(in_state(AppState::JoinSetup)),
                refresh_lobby_ui.run_if(in_state(AppState::Lobby)),
                poll_net_events,
                playing_predict_local.run_if(in_state(AppState::Playing)),
                playing_send_input.run_if(in_state(AppState::Playing)),
                playing_host_simulate.run_if(in_state(AppState::Playing)),
            ),
        )
        .add_systems(
            Update,
            (
                advance_interpolation.run_if(in_state(AppState::Playing)),
                playing_client_local_hits.run_if(in_state(AppState::Playing)),
                maybe_record_high_score.run_if(in_state(AppState::Playing)),
                sync_spinner_hum.run_if(in_state(AppState::Playing)),
                sync_world_sprites.run_if(in_state(AppState::Playing)),
                play_sfx_triggers,
            ),
        )
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    MainMenu,
    SoloSetup,
    HostSetup,
    JoinSetup,
    Connecting,
    Lobby,
    Playing,
}

#[derive(Resource)]
struct ClientSettings {
    server_url: String,
    nickname: String,
}

#[derive(Resource)]
struct HostForm {
    nickname: String,
    duration_index: usize,
    focus_nickname: bool,
}

impl Default for HostForm {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            duration_index: 0,
            focus_nickname: true,
        }
    }
}

impl HostForm {
    fn duration(&self) -> GameDurationMinutes {
        GameDurationMinutes::ALL[self.duration_index % GameDurationMinutes::ALL.len()]
    }

    fn solo_duration(&self) -> GameDurationMinutes {
        GameDurationMinutes::SOLO[self.duration_index % GameDurationMinutes::SOLO.len()]
    }
}

#[derive(Resource)]
struct JoinForm {
    nickname: String,
    code: String,
    focus_nickname: bool,
}

impl Default for JoinForm {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            code: String::new(),
            focus_nickname: true,
        }
    }
}

#[derive(Resource, Clone, Copy, PartialEq, Eq, Default)]
enum ConnectIntent {
    #[default]
    None,
    Host,
    Join,
}

#[derive(Resource, Default)]
pub struct Session {
    pub player_id: Option<String>,
    pub room: Option<RoomInfo>,
    pub is_owner: bool,
    pub accept_connection: bool,
    /// Offline solo match — never talks to the relay.
    pub solo: bool,
}

#[derive(Resource, Default)]
struct StatusMessage {
    text: String,
}

#[derive(Component)]
struct UiRoot;

#[derive(Component)]
struct DynamicText;

fn setup_net_bridge(mut commands: Commands) {
    commands.insert_resource(NetBridge::spawn());
}

fn spawn_screen(commands: &mut Commands, title: &str, body: String) {
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
                TextColor(Color::srgb(0.85, 0.9, 1.0)),
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

fn spawn_main_menu(
    mut commands: Commands,
    settings: Res<ClientSettings>,
    status: Res<StatusMessage>,
    high_scores: Res<HighScores>,
) {
    let status_line = if status.text.is_empty() {
        String::new()
    } else {
        format!("\nLast status: {}\n", status.text)
    };
    let scores = high_scores.menu_block();
    spawn_screen(
        &mut commands,
        "ASTROWAR",
        format!(
            "Server: {}{status_line}\n[1] Solo (offline)\n[2] Host internet room\n[3] Join with code\n[Esc] Quit{scores}",
            settings.server_url
        ),
    );
}

fn spawn_solo_setup(
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

fn spawn_host_setup(
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

fn spawn_join_setup(
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

fn spawn_connecting(mut commands: Commands, status: Res<StatusMessage>) {
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

fn spawn_lobby(
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
        format!("\nStatus: {}\n", status.text)
    };
    format!(
        "Server: {}{status_line}\n{nick_mark} Nickname: {}\n{dur_mark} Duration: {}\n\n[Tab] Switch field\n[Left/Right] Change duration\n[Enter] Create room\n[Esc] Back",
        settings.server_url,
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
        format!("\nStatus: {}\n", status.text)
    };
    format!(
        "Server: {}{status_line}\n{nick_mark} Nickname: {}\n{code_mark} Room code: {}\n\nNicknames must be unique in the room.\n[Tab] Switch field\n[Enter] Join room\n[Esc] Back",
        settings.server_url, form.nickname, form.code,
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

fn cleanup_ui_root(mut commands: Commands, query: Query<Entity, With<UiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn set_dynamic_text(query: &mut Query<&mut Text, With<DynamicText>>, body: String) {
    for mut text in query.iter_mut() {
        *text = Text::new(body.clone());
    }
}

fn refresh_solo_setup_ui(
    form: Res<HostForm>,
    status: Res<StatusMessage>,
    mut query: Query<&mut Text, With<DynamicText>>,
) {
    if !(form.is_changed() || status.is_changed()) {
        return;
    }
    set_dynamic_text(&mut query, solo_setup_body(&form, &status));
}

fn refresh_host_setup_ui(
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

fn refresh_join_setup_ui(
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

fn refresh_lobby_ui(
    session: Res<Session>,
    status: Res<StatusMessage>,
    mut query: Query<&mut Text, With<DynamicText>>,
) {
    if !(session.is_changed() || status.is_changed()) {
        return;
    }
    set_dynamic_text(&mut query, lobby_body(&session, &status));
}

fn handle_main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
    mut status: ResMut<StatusMessage>,
    mut host_form: ResMut<HostForm>,
    mut join_form: ResMut<JoinForm>,
    mut intent: ResMut<ConnectIntent>,
    settings: Res<ClientSettings>,
    mut sfx: EventWriter<SfxTrigger>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        status.text.clear();
        *intent = ConnectIntent::None;
        // Prefill only when a saved nick exists; otherwise keep blank.
        host_form.nickname = settings.nickname.clone();
        host_form.focus_nickname = true;
        sfx.send(SfxTrigger::Shot);
        next_state.set(AppState::SoloSetup);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        status.text.clear();
        *intent = ConnectIntent::Host;
        host_form.nickname = settings.nickname.clone();
        host_form.duration_index %= GameDurationMinutes::ALL.len();
        host_form.focus_nickname = true;
        sfx.send(SfxTrigger::Shot);
        next_state.set(AppState::HostSetup);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        status.text.clear();
        *intent = ConnectIntent::Join;
        join_form.nickname = settings.nickname.clone();
        join_form.code.clear();
        join_form.focus_nickname = true;
        sfx.send(SfxTrigger::Shot);
        next_state.set(AppState::JoinSetup);
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }
}

fn handle_solo_setup_input(
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
        sfx.send(SfxTrigger::Shot);
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

fn handle_host_setup_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mut form: ResMut<HostForm>,
    mut settings: ResMut<ClientSettings>,
    mut status: ResMut<StatusMessage>,
    mut session: ResMut<Session>,
    mut intent: ResMut<ConnectIntent>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
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
        sfx.send(SfxTrigger::Shot);
        bridge.send(NetCommand::ConnectAndCreate {
            url: settings.server_url.clone(),
            nickname,
            duration: form.duration(),
        });
        next_state.set(AppState::Connecting);
    }
}

fn handle_join_setup_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mut form: ResMut<JoinForm>,
    mut settings: ResMut<ClientSettings>,
    mut status: ResMut<StatusMessage>,
    mut session: ResMut<Session>,
    mut intent: ResMut<ConnectIntent>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
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
        sfx.send(SfxTrigger::Shot);
        bridge.send(NetCommand::ConnectAndJoin {
            url: settings.server_url.clone(),
            nickname,
            code,
        });
        next_state.set(AppState::Connecting);
    }
}

fn handle_connecting_input(
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

fn handle_lobby_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut status: ResMut<StatusMessage>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
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
        sfx.send(SfxTrigger::Shot);
        bridge.send(NetCommand::StartGame);
    }
}

fn handle_playing_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut status: ResMut<StatusMessage>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
    latest: Res<LatestState>,
    host: Res<HostSim>,
    mut high_scores: ResMut<HighScores>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        // Capture score before clearing the session (leave happens this frame).
        record_local_high_score(&session, &latest, &host, &mut high_scores);
        let was_solo = session.solo;
        session.accept_connection = false;
        status.text.clear();
        if !was_solo {
            bridge.send(NetCommand::Leave);
        }
        session.player_id = None;
        session.room = None;
        session.is_owner = false;
        session.solo = false;
        next_state.set(AppState::MainMenu);
    }
}

fn start_solo_match(
    session: &mut Session,
    host: &mut HostSim,
    latest: &mut LatestState,
    prediction: &mut LocalPrediction,
    nickname: String,
    duration: GameDurationMinutes,
) {
    let player_id = "local-player".to_string();
    session.player_id = Some(player_id.clone());
    session.is_owner = true;
    session.accept_connection = false;
    session.solo = true;
    session.room = Some(RoomInfo {
        code: "SOLO".into(),
        owner_id: player_id.clone(),
        phase: RoomPhase::Playing,
        mode: GameMode::Competitive,
        duration_minutes: duration,
        players: vec![PlayerInfo {
            id: player_id,
            nickname,
        }],
        max_players: MAX_PLAYERS,
    });
    *prediction = LocalPrediction::default();
    begin_host_sim(session, host, duration);
    apply_host_snapshot_locally(host, latest);
}

fn enter_playing_as_host(
    session: &Session,
    host: &mut HostSim,
    latest: &mut LatestState,
    bridge: &NetBridge,
) {
    let duration = session
        .room
        .as_ref()
        .map(|room| room.duration_minutes)
        .unwrap_or(GameDurationMinutes::Five);
    begin_host_sim(session, host, duration);
    apply_host_snapshot_locally(host, latest);
    send_game(bridge, session, &snapshot(host));
}

fn apply_host_snapshot_locally(host: &HostSim, latest: &mut LatestState) {
    let message = snapshot(host);
    if let GameMessage::State {
        tick,
        time_left_secs,
        match_over,
        ships,
        bullets,
        asteroids,
    } = &message
    {
        latest.from_ships = ships.clone();
        latest.to_ships = ships.clone();
        latest.tick = *tick;
        latest.time_left_secs = *time_left_secs;
        latest.match_over = *match_over;
        latest.bullets = bullets.clone();
        latest.asteroids = asteroids.clone();
        latest.age = 0.0;
        latest.interval = 1.0 / 20.0;
        latest.pending_destroyed.clear();
    }
}

fn poll_net_events(
    bridge: Res<NetBridge>,
    mut session: ResMut<Session>,
    mut status: ResMut<StatusMessage>,
    mut settings: ResMut<ClientSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    current: Res<State<AppState>>,
    intent: Res<ConnectIntent>,
    mut host: ResMut<HostSim>,
    mut latest: ResMut<LatestState>,
    mut prediction: ResMut<LocalPrediction>,
    mut sfx: EventWriter<SfxTrigger>,
) {
    while let Some(event) = bridge.poll() {
        match event {
            NetEvent::Connected { player_id } => {
                if session.accept_connection {
                    session.player_id = Some(player_id);
                }
            }
            NetEvent::RoomUpdated(room) => {
                if !session.accept_connection
                    && *current.get() != AppState::Lobby
                    && *current.get() != AppState::Playing
                {
                    continue;
                }
                let player_id = session.player_id.clone().unwrap_or_default();
                session.is_owner = room.owner_id == player_id;
                session.solo = false;
                remember_room_nickname(&mut settings, &player_id, &room);
                let phase = room.phase;
                session.room = Some(room.clone());
                status.text.clear();
                if host.active && session.is_owner {
                    seed_ships_from_room(&mut host, &room);
                }
                if *current.get() == AppState::Connecting {
                    if phase == RoomPhase::Playing {
                        if session.is_owner {
                            enter_playing_as_host(&session, &mut host, &mut latest, &bridge);
                        } else {
                            send_game(&bridge, &session, &GameMessage::RequestSnapshot);
                        }
                        next_state.set(AppState::Playing);
                    } else {
                        next_state.set(AppState::Lobby);
                    }
                }
            }
            NetEvent::GameStarted(room) => {
                if !session.accept_connection && *current.get() != AppState::Lobby {
                    continue;
                }
                let player_id = session.player_id.clone().unwrap_or_default();
                session.is_owner = room.owner_id == player_id;
                session.solo = false;
                remember_room_nickname(&mut settings, &player_id, &room);
                session.room = Some(room);
                status.text.clear();
                if session.is_owner {
                    enter_playing_as_host(&session, &mut host, &mut latest, &bridge);
                } else {
                    send_game(&bridge, &session, &GameMessage::RequestSnapshot);
                }
                next_state.set(AppState::Playing);
            }
            NetEvent::PlayerLeft {
                player_id,
                nickname,
                forfeited,
            } => {
                let kind = if forfeited { "forfeited" } else { "left" };
                status.text = format!("{nickname} {kind}");
                if host.active {
                    mark_player_forfeit(&mut host, &player_id);
                }
            }
            NetEvent::Relayed {
                from_player_id,
                payload,
            } => {
                handle_relayed_game_message(
                    &from_player_id,
                    &payload,
                    &session,
                    &mut host,
                    &mut latest,
                    &mut prediction,
                    &bridge,
                    &mut sfx,
                );
            }
            NetEvent::Error(message) => {
                status.text = message;
                if *current.get() == AppState::Connecting {
                    session.accept_connection = false;
                    match intent.as_ref() {
                        ConnectIntent::Host => next_state.set(AppState::HostSetup),
                        ConnectIntent::Join => next_state.set(AppState::JoinSetup),
                        ConnectIntent::None => next_state.set(AppState::MainMenu),
                    }
                }
            }
            NetEvent::Disconnected => {
                if *current.get() == AppState::Lobby || *current.get() == AppState::Playing {
                    session.player_id = None;
                    session.room = None;
                    session.is_owner = false;
                    session.solo = false;
                    session.accept_connection = false;
                    status.text = "Disconnected from relay".into();
                    next_state.set(AppState::MainMenu);
                }
            }
        }
    }
}

fn remember_room_nickname(settings: &mut ClientSettings, player_id: &str, room: &RoomInfo) {
    if let Some(player) = room.players.iter().find(|p| p.id == player_id) {
        settings.nickname = player.nickname.clone();
        save_last_nickname(&player.nickname);
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
