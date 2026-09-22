mod net_bridge;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use net_bridge::{NetBridge, NetCommand, NetEvent};
use protocol::{GameDurationMinutes, RoomInfo};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.02, 0.04, 0.1)))
        .insert_resource(ClientSettings {
            server_url: net::default_server_url(),
            nickname: "Player".into(),
        })
        .insert_resource(HostForm::default())
        .insert_resource(JoinForm::default())
        .insert_resource(Session::default())
        .insert_resource(StatusMessage::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "AstroWar".into(),
                resolution: (960.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_systems(Startup, (setup_camera, setup_net_bridge))
        .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
        .add_systems(OnEnter(AppState::HostSetup), spawn_host_setup)
        .add_systems(OnEnter(AppState::JoinSetup), spawn_join_setup)
        .add_systems(OnEnter(AppState::Connecting), spawn_connecting)
        .add_systems(OnEnter(AppState::Lobby), spawn_lobby)
        .add_systems(OnExit(AppState::MainMenu), cleanup_ui_root)
        .add_systems(OnExit(AppState::HostSetup), cleanup_ui_root)
        .add_systems(OnExit(AppState::JoinSetup), cleanup_ui_root)
        .add_systems(OnExit(AppState::Connecting), cleanup_ui_root)
        .add_systems(OnExit(AppState::Lobby), cleanup_ui_root)
        .add_systems(
            Update,
            (
                handle_main_menu_input.run_if(in_state(AppState::MainMenu)),
                handle_host_setup_input.run_if(in_state(AppState::HostSetup)),
                handle_join_setup_input.run_if(in_state(AppState::JoinSetup)),
                handle_lobby_input.run_if(in_state(AppState::Lobby)),
                handle_connecting_input.run_if(in_state(AppState::Connecting)),
                refresh_host_setup_ui.run_if(in_state(AppState::HostSetup)),
                refresh_join_setup_ui.run_if(in_state(AppState::JoinSetup)),
                refresh_lobby_ui.run_if(in_state(AppState::Lobby)),
                poll_net_events,
            ),
        )
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    MainMenu,
    HostSetup,
    JoinSetup,
    Connecting,
    Lobby,
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
            nickname: "Player".into(),
            duration_index: 0,
            focus_nickname: true,
        }
    }
}

impl HostForm {
    fn duration(&self) -> GameDurationMinutes {
        GameDurationMinutes::ALL[self.duration_index % GameDurationMinutes::ALL.len()]
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
            nickname: "Player".into(),
            code: String::new(),
            focus_nickname: true,
        }
    }
}

#[derive(Resource, Default)]
struct Session {
    player_id: Option<String>,
    room: Option<RoomInfo>,
    is_owner: bool,
    accept_connection: bool,
}

#[derive(Resource, Default)]
struct StatusMessage {
    text: String,
}

#[derive(Component)]
struct UiRoot;

#[derive(Component)]
struct DynamicText;

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

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

fn spawn_main_menu(mut commands: Commands, settings: Res<ClientSettings>) {
    spawn_screen(
        &mut commands,
        "ASTROWAR",
        format!(
            "Server: {}\n\n[1] Host internet room\n[2] Join with code\n[Esc] Quit",
            settings.server_url
        ),
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
        "Server: {}\n{status_line}\n{nick_mark} Nickname: {}\n{dur_mark} Duration: {} min\n\n[Tab] Switch field\n[Left/Right] Change duration\n[Enter] Create room\n[Esc] Back",
        settings.server_url,
        form.nickname,
        form.duration().as_minutes(),
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
        "Server: {}\n{status_line}\n{nick_mark} Nickname: {}\n{code_mark} Room code: {}\n\n[Tab] Switch field\n[Enter] Join room\n[Esc] Back",
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
        "{status_line}Code: {}\nPhase: {:?}\nDuration: {} min\nPlayers ({}/{}):\n{}\n\n{owner_help}[Esc] Leave",
        room.code,
        room.phase,
        room.duration_minutes.as_minutes(),
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
    settings: Res<ClientSettings>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        status.text.clear();
        host_form.nickname = settings.nickname.clone();
        host_form.focus_nickname = true;
        next_state.set(AppState::HostSetup);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        status.text.clear();
        join_form.nickname = settings.nickname.clone();
        join_form.code.clear();
        join_form.focus_nickname = true;
        next_state.set(AppState::JoinSetup);
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }
}

fn handle_host_setup_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: EventReader<KeyboardInput>,
    mut form: ResMut<HostForm>,
    mut settings: ResMut<ClientSettings>,
    mut status: ResMut<StatusMessage>,
    mut session: ResMut<Session>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
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
        let nickname = form.nickname.trim().to_string();
        if nickname.is_empty() {
            status.text = "Nickname is required".into();
            return;
        }
        settings.nickname = nickname.clone();
        session.accept_connection = true;
        status.text = "Creating room...".into();
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
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
) {
    if keys.just_pressed(KeyCode::Escape) {
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
        if nickname.is_empty() {
            status.text = "Nickname is required".into();
            return;
        }
        if code.len() < 4 {
            status.text = "Room code is required".into();
            return;
        }
        settings.nickname = nickname.clone();
        session.accept_connection = true;
        status.text = format!("Joining {code}...");
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
) {
    if keys.just_pressed(KeyCode::Escape) {
        session.accept_connection = false;
        status.text.clear();
        bridge.send(NetCommand::Leave);
        session.player_id = None;
        session.room = None;
        session.is_owner = false;
        next_state.set(AppState::MainMenu);
        return;
    }
    if keys.just_pressed(KeyCode::Enter) && session.is_owner {
        status.text = "Starting match...".into();
        bridge.send(NetCommand::StartGame);
    }
}

fn poll_net_events(
    bridge: Res<NetBridge>,
    mut session: ResMut<Session>,
    mut status: ResMut<StatusMessage>,
    mut next_state: ResMut<NextState<AppState>>,
    current: Res<State<AppState>>,
) {
    while let Some(event) = bridge.poll() {
        match event {
            NetEvent::Connected { player_id } => {
                if session.accept_connection {
                    session.player_id = Some(player_id);
                }
            }
            NetEvent::RoomUpdated(room) => {
                if !session.accept_connection && *current.get() != AppState::Lobby {
                    continue;
                }
                let player_id = session.player_id.clone().unwrap_or_default();
                session.is_owner = room.owner_id == player_id;
                session.room = Some(room);
                status.text.clear();
                if *current.get() == AppState::Connecting {
                    next_state.set(AppState::Lobby);
                }
            }
            NetEvent::GameStarted(room) => {
                if !session.accept_connection && *current.get() != AppState::Lobby {
                    continue;
                }
                let player_id = session.player_id.clone().unwrap_or_default();
                session.is_owner = room.owner_id == player_id;
                session.room = Some(room);
                status.text = "Match started (gameplay comes next)".into();
            }
            NetEvent::PlayerLeft {
                nickname,
                forfeited,
                ..
            } => {
                let kind = if forfeited { "forfeited" } else { "left" };
                status.text = format!("{nickname} {kind}");
            }
            NetEvent::Error(message) => {
                status.text = message;
                if *current.get() == AppState::Connecting {
                    session.accept_connection = false;
                    next_state.set(AppState::MainMenu);
                }
            }
            NetEvent::Disconnected => {
                if *current.get() == AppState::Lobby {
                    session.player_id = None;
                    session.room = None;
                    session.is_owner = false;
                    session.accept_connection = false;
                    status.text = "Disconnected from relay".into();
                    next_state.set(AppState::MainMenu);
                }
            }
        }
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
