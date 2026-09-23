//! Fixed logical play board. Gameplay and network coords live in this space.
//! The camera letterboxes into the window so the board never stretches or gains
//! extra playable area on resize (same ratio as design 120×90 → 4:3).

use bevy::prelude::*;
use bevy::render::camera::{ClearColorConfig, ScalingMode, Viewport};
use bevy::window::WindowResized;

/// Logical board width in world units (4:3 with [`BOARD_HEIGHT`]).
pub const BOARD_WIDTH: f32 = 960.0;
/// Logical board height in world units (4:3 with [`BOARD_WIDTH`]).
pub const BOARD_HEIGHT: f32 = 720.0;

/// Half-width of the ship / asteroid strip inside the board.
pub const PLAY_AREA_X: f32 = 420.0;

pub fn letterbox_viewport(window_width: u32, window_height: u32) -> Viewport {
    let win_w = window_width.max(1) as f32;
    let win_h = window_height.max(1) as f32;
    let scale = (win_w / BOARD_WIDTH).min(win_h / BOARD_HEIGHT);
    let mut view_w = (BOARD_WIDTH * scale).round() as u32;
    let mut view_h = (BOARD_HEIGHT * scale).round() as u32;
    view_w = view_w.clamp(1, window_width.max(1));
    view_h = view_h.clamp(1, window_height.max(1));
    let x = (window_width.saturating_sub(view_w)) / 2;
    let y = (window_height.saturating_sub(view_h)) / 2;
    Viewport {
        physical_position: UVec2::new(x, y),
        physical_size: UVec2::new(view_w, view_h),
        depth: 0.0..1.0,
    }
}

pub fn setup_board_camera(mut commands: Commands, windows: Query<&Window>) {
    let viewport = match windows.get_single() {
        Ok(window) => letterbox_viewport(window.physical_width(), window.physical_height()),
        Err(_) => letterbox_viewport(960, 720),
    };

    commands.spawn((
        Camera2d,
        Camera {
            viewport: Some(viewport),
            clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.04, 0.1)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: BOARD_WIDTH,
                height: BOARD_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

pub fn update_board_viewport(
    mut resize: EventReader<WindowResized>,
    windows: Query<&Window>,
    mut cameras: Query<&mut Camera, With<Camera2d>>,
) {
    let mut should_update = false;
    for _ in resize.read() {
        should_update = true;
    }
    if !should_update {
        return;
    }
    let Ok(window) = windows.get_single() else {
        return;
    };
    let viewport = letterbox_viewport(window.physical_width(), window.physical_height());
    for mut camera in &mut cameras {
        camera.viewport = Some(viewport.clone());
    }
}
