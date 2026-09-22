use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.02, 0.04, 0.1)))
        .insert_resource(ClientSettings {
            server_url: net::default_server_url(),
            nickname: String::new(),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "AstroWar".into(),
                resolution: (960, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
        .add_systems(OnExit(AppState::MainMenu), cleanup_menu)
        .add_systems(Update, handle_menu_input.run_if(in_state(AppState::MainMenu)))
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    MainMenu,
    InternetHost,
    InternetJoin,
    Lobby,
    Playing,
}

#[derive(Resource)]
struct ClientSettings {
    server_url: String,
    nickname: String,
}

#[derive(Component)]
struct MenuRoot;

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_main_menu(mut commands: Commands, settings: Res<ClientSettings>) {
    commands
        .spawn((
            MenuRoot,
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
                Text::new("ASTROWAR"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.9, 1.0)),
            ));
            parent.spawn((
                Text::new(format!(
                    "Server default: {}\n[1] Internet Multiplayer (host)\n[2] Join with code\n[Esc] Quit",
                    settings.server_url
                )),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.75, 0.85)),
            ));
        });
}

fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn handle_menu_input(
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
