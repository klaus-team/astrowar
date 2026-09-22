use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.02, 0.04, 0.1)))
        .insert_resource(ClientSettings {
            server_url: net::default_server_url(),
            nickname: "Player".into(),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "AstroWar".into(),
                resolution: (960.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
        .add_systems(OnEnter(AppState::InternetHost), spawn_host_stub)
        .add_systems(OnEnter(AppState::InternetJoin), spawn_join_stub)
        .add_systems(
            OnExit(AppState::MainMenu),
            cleanup_ui_root,
        )
        .add_systems(
            OnExit(AppState::InternetHost),
            cleanup_ui_root,
        )
        .add_systems(
            OnExit(AppState::InternetJoin),
            cleanup_ui_root,
        )
        .add_systems(Update, handle_main_menu_input.run_if(in_state(AppState::MainMenu)))
        .add_systems(
            Update,
            handle_stub_back_input
                .run_if(in_state(AppState::InternetHost).or(in_state(AppState::InternetJoin))),
        )
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    MainMenu,
    InternetHost,
    InternetJoin,
}

#[derive(Resource)]
struct ClientSettings {
    server_url: String,
    nickname: String,
}

#[derive(Component)]
struct UiRoot;

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
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
            "Server default: {}\nNickname default: {}\n\n[1] Internet Multiplayer (host)\n[2] Join with code\n[Esc] Quit",
            settings.server_url, settings.nickname
        ),
    );
}

fn spawn_host_stub(mut commands: Commands, settings: Res<ClientSettings>) {
    spawn_screen(
        &mut commands,
        "Host room",
        format!(
            "Flow not wired yet.\nServer: {}\n\nNext: nickname, duration, create room, show code.\n[Esc] Back",
            settings.server_url
        ),
    );
}

fn spawn_join_stub(mut commands: Commands, settings: Res<ClientSettings>) {
    spawn_screen(
        &mut commands,
        "Join room",
        format!(
            "Flow not wired yet.\nServer: {}\n\nNext: nickname, enter room code, connect.\n[Esc] Back",
            settings.server_url
        ),
    );
}

fn cleanup_ui_root(mut commands: Commands, query: Query<Entity, With<UiRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        next_state.set(AppState::InternetHost);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        next_state.set(AppState::InternetJoin);
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }
}

fn handle_stub_back_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::MainMenu);
    }
}
