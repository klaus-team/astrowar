mod board;
mod game_sync;
mod highscores;
mod net_bridge;
mod nickname;
mod playing;
mod shapes;
mod sounds;
mod storage;
mod ui;

use bevy::prelude::*;
use board::{setup_board_camera, update_board_viewport};
use game_sync::GameMessage;
use highscores::HighScores;
use net_bridge::{NetBridge, NetCommand, NetEvent};
use nickname::{load_last_nickname, save_last_nickname};
use shapes::setup_shape_meshes;
use sounds::{
    play_sfx_triggers, setup_sounds, sync_spinner_hum, toggle_mute_audio, AudioMuted, SfxTrigger,
};
use playing::{
    advance_interpolation, begin_host_sim, cleanup_playing, handle_relayed_game_message,
    mark_player_forfeit, maybe_record_high_score, playing_client_local_hits,
    playing_host_simulate, playing_predict_local, playing_send_input, record_local_high_score,
    seed_ships_from_room, send_game, snapshot, spawn_playing_hud, sync_world_sprites,
    watch_hurt_flash, HostSim, HurtFlash, InputThrottle, LatestState, LocalPrediction,
    MatchOverReturn,
};
use ui::{
    cleanup_ui_root, handle_connecting_input, handle_host_setup_input, handle_join_setup_input,
    handle_lobby_input, handle_main_menu_input, handle_solo_setup_input, refresh_host_setup_ui,
    refresh_join_setup_ui, refresh_lobby_ui, refresh_main_menu_ui, refresh_solo_setup_ui,
    spawn_connecting, spawn_host_setup, spawn_join_setup, spawn_lobby, spawn_main_menu,
    spawn_solo_setup,
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
        .insert_resource(MatchOverReturn::default())
        .insert_resource(HurtFlash::default())
        .insert_resource(AudioMuted::default())
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
                toggle_mute_audio,
                handle_main_menu_input.run_if(in_state(AppState::MainMenu)),
                handle_solo_setup_input.run_if(in_state(AppState::SoloSetup)),
                handle_host_setup_input.run_if(in_state(AppState::HostSetup)),
                handle_join_setup_input.run_if(in_state(AppState::JoinSetup)),
                handle_lobby_input.run_if(in_state(AppState::Lobby)),
                handle_connecting_input.run_if(in_state(AppState::Connecting)),
                handle_playing_input.run_if(in_state(AppState::Playing)),
                auto_return_after_match_over.run_if(in_state(AppState::Playing)),
                refresh_solo_setup_ui.run_if(in_state(AppState::SoloSetup)),
                refresh_host_setup_ui.run_if(in_state(AppState::HostSetup)),
                refresh_join_setup_ui.run_if(in_state(AppState::JoinSetup)),
                refresh_lobby_ui.run_if(in_state(AppState::Lobby)),
                refresh_main_menu_ui.run_if(in_state(AppState::MainMenu)),
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
                watch_hurt_flash.run_if(in_state(AppState::Playing)),
                sync_world_sprites.run_if(in_state(AppState::Playing)),
                play_sfx_triggers,
            ),
        )
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) enum AppState {
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
pub(crate) struct ClientSettings {
    pub(crate) server_url: String,
    pub(crate) nickname: String,
}

#[derive(Resource)]
pub(crate) struct HostForm {
    pub(crate) nickname: String,
    pub(crate) duration_index: usize,
    pub(crate) focus_nickname: bool,
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
    pub(crate) fn duration(&self) -> GameDurationMinutes {
        GameDurationMinutes::ALL[self.duration_index % GameDurationMinutes::ALL.len()]
    }

    pub(crate) fn solo_duration(&self) -> GameDurationMinutes {
        GameDurationMinutes::SOLO[self.duration_index % GameDurationMinutes::SOLO.len()]
    }
}

#[derive(Resource)]
pub(crate) struct JoinForm {
    pub(crate) nickname: String,
    pub(crate) code: String,
    pub(crate) focus_nickname: bool,
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
pub(crate) enum ConnectIntent {
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
pub(crate) struct StatusMessage {
    pub(crate) text: String,
}

fn setup_net_bridge(mut commands: Commands) {
    commands.insert_resource(NetBridge::spawn());
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
    mut match_over_return: ResMut<MatchOverReturn>,
) {
    let match_over = latest.match_over || (session.is_owner && host.match_over);
    // Any key while the match is over counts as interaction and restarts the idle timer.
    if match_over && keys.get_just_pressed().next().is_some() && !keys.just_pressed(KeyCode::Escape)
    {
        match_over_return.timer = Some(Timer::from_seconds(
            MatchOverReturn::IDLE_SECS,
            TimerMode::Once,
        ));
    }
    if keys.just_pressed(KeyCode::Escape) {
        leave_playing_to_menu(
            &mut session,
            &mut status,
            &mut next_state,
            &bridge,
            &latest,
            &host,
            &mut high_scores,
            &mut match_over_return,
        );
    }
}

fn auto_return_after_match_over(
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut status: ResMut<StatusMessage>,
    mut next_state: ResMut<NextState<AppState>>,
    bridge: Res<NetBridge>,
    latest: Res<LatestState>,
    host: Res<HostSim>,
    mut high_scores: ResMut<HighScores>,
    mut match_over_return: ResMut<MatchOverReturn>,
) {
    let match_over = latest.match_over || (session.is_owner && host.match_over);
    if !match_over {
        match_over_return.reset();
        return;
    }
    if match_over_return.timer.is_none() {
        match_over_return.timer = Some(Timer::from_seconds(
            MatchOverReturn::IDLE_SECS,
            TimerMode::Once,
        ));
    }
    let finished = match_over_return
        .timer
        .as_mut()
        .map(|timer| {
            timer.tick(time.delta());
            timer.just_finished()
        })
        .unwrap_or(false);
    if finished {
        leave_playing_to_menu(
            &mut session,
            &mut status,
            &mut next_state,
            &bridge,
            &latest,
            &host,
            &mut high_scores,
            &mut match_over_return,
        );
    }
}

fn leave_playing_to_menu(
    session: &mut Session,
    status: &mut StatusMessage,
    next_state: &mut NextState<AppState>,
    bridge: &NetBridge,
    latest: &LatestState,
    host: &HostSim,
    high_scores: &mut HighScores,
    match_over_return: &mut MatchOverReturn,
) {
    record_local_high_score(session, latest, host, high_scores);
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
    match_over_return.reset();
    next_state.set(AppState::MainMenu);
}

pub(crate) fn start_solo_match(
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

