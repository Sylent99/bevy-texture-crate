//! Minimal example: a cube wearing the "exterior_wall_main" material from your palette.
//! Needs `assets/palette.json` (see ../palette.example.json) and a light + camera.
mod ambientcg;

use ambientcg::{AmbientCgLibrary, AmbientCgPlugin};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AmbientCgPlugin::new("palette.json"))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<AmbientCgLibrary>,
    asset_server: Res<AssetServer>,
) {
    // Seed picks a variant when a role lists several (e.g. use a building id).
    let wall = lib.material("exterior_wall_main", 0, &asset_server, &mut materials);

    // Procedural meshes need UVs and TANGENTS or normal maps will not show.
    let cube = Mesh::from(Cuboid::new(2.0, 2.0, 2.0))
        .with_generated_tangents()
        .unwrap();

    commands.spawn((Mesh3d(meshes.add(cube)), MeshMaterial3d(wall)));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-3.5, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
