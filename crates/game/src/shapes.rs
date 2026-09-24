//! Shared 2D meshes for ships and asteroid silhouettes.

use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use crate::game_sync::AsteroidKind;

#[derive(Resource, Clone)]
pub struct ShapeMeshes {
    pub ship: Handle<Mesh>,
    pub large: Handle<Mesh>,
    pub small: Handle<Mesh>,
    pub spinner: Handle<Mesh>,
    pub zigzag: Handle<Mesh>,
}

impl ShapeMeshes {
    pub fn for_asteroid(&self, kind: AsteroidKind) -> Handle<Mesh> {
        match kind {
            AsteroidKind::Large => self.large.clone(),
            AsteroidKind::Small => self.small.clone(),
            AsteroidKind::Spinner => self.spinner.clone(),
            AsteroidKind::Zigzag => self.zigzag.clone(),
        }
    }
}

pub fn setup_shape_meshes(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(ShapeMeshes {
        ship: meshes.add(ship_mesh()),
        // Unit-radius silhouettes; scaled by asteroid.radius at spawn time.
        large: meshes.add(rock_mesh(1.0, 8, 0.18)),
        small: meshes.add(Mesh::from(RegularPolygon::new(1.0, 5))),
        spinner: meshes.add(Mesh::from(RegularPolygon::new(1.0, 3))),
        zigzag: meshes.add(Mesh::from(Rhombus::new(1.35, 0.85))),
    });
}

/// Flat-top triangle with a gun notch on the top edge (bullet exits through the gap).
fn ship_mesh() -> Mesh {
    // Width ~26, height ~30, origin at center.
    let positions: Vec<[f32; 3]> = vec![
        [-13.0, -15.0, 0.0], // 0 bottom-left
        [13.0, -15.0, 0.0],  // 1 bottom-right
        [9.0, 15.0, 0.0],    // 2 top-right shoulder
        [3.5, 15.0, 0.0],    // 3 notch outer-right
        [3.5, 8.0, 0.0],     // 4 notch inner-right
        [-3.5, 8.0, 0.0],    // 5 notch inner-left
        [-3.5, 15.0, 0.0],   // 6 notch outer-left
        [-9.0, 15.0, 0.0],   // 7 top-left shoulder
    ];
    let indices = Indices::U32(vec![
        0, 1, 2, //
        0, 2, 3, //
        0, 3, 4, //
        0, 4, 5, //
        0, 5, 6, //
        0, 6, 7, //
    ]);
    mesh_from_positions(positions, indices)
}

/// Lumpy rock (irregular N-gon) for large asteroids.
fn rock_mesh(radius: f32, sides: usize, jitter: f32) -> Mesh {
    let mut positions = Vec::with_capacity(sides + 1);
    positions.push([0.0, 0.0, 0.0]);
    let mut indices = Vec::with_capacity(sides * 3);
    for i in 0..sides {
        let t = i as f32 / sides as f32 * std::f32::consts::TAU;
        // Deterministic radius wobble so the silhouette reads as a rock.
        let wobble = 1.0 + jitter * ((i as f32 * 2.7).sin() * 0.6 + (i as f32 * 1.3).cos() * 0.4);
        let r = radius * wobble;
        positions.push([r * t.cos(), r * t.sin(), 0.0]);
        let a = (i + 1) as u32;
        let b = if i + 1 == sides { 1 } else { (i + 2) as u32 };
        indices.extend_from_slice(&[0, a, b]);
    }
    mesh_from_positions(positions, Indices::U32(indices))
}

fn mesh_from_positions(positions: Vec<[f32; 3]>, indices: Indices) -> Mesh {
    let n = positions.len();
    let normals = vec![[0.0, 0.0, 1.0]; n];
    let uvs = positions
        .iter()
        .map(|p| [(p[0] + 1.0) * 0.5, (p[1] + 1.0) * 0.5])
        .collect::<Vec<_>>();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(indices);
    mesh
}
