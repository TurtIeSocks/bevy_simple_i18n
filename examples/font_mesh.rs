//! Translated 3D text mesh via `bevy_fontmesh` (feature `fontmesh`).
//!
//! Run: cargo run --example font_mesh --features fontmesh
//! Press SPACE to cycle through the available locales.

use bevy::prelude::*;
use bevy_fontmesh::{FontMeshPlugin, TextMeshStyle};
use bevy_simple_i18n::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FontMeshPlugin::<StandardMaterial>::default())
        .add_plugins(I18nPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, cycle_locale)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            intensity: 5000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 8.0),
    ));

    // The translated mesh: bevy_simple_i18n keeps this entity's TextMesh in
    // sync with the "hello" key. `ja.ttf` covers Latin + kana + CJK so every
    // shipped locale renders.
    commands.spawn((
        I18nTextMesh::new("hello"),
        bevy_fontmesh::TextMesh {
            font: asset_server.load("fonts/NotoSans/ja.ttf"),
            style: TextMeshStyle {
                depth: 0.5,
                ..default()
            },
            ..default()
        },
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.3, 0.8),
            ..default()
        })),
        Transform::from_xyz(-2.5, 0.0, 0.0),
    ));
}

/// SPACE cycles through every locale shipped in the manifest.
fn cycle_locale(keys: Res<ButtonInput<KeyCode>>, mut i18n: ResMut<I18n>) {
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    let locales = i18n.locales().to_vec();
    let Some(current) = locales.iter().position(|l| l == i18n.current()) else {
        if let Some(first) = locales.first() {
            i18n.set_locale(first.clone());
        }
        return;
    };
    let next = locales[(current + 1) % locales.len()].clone();
    i18n.set_locale(next);
}
