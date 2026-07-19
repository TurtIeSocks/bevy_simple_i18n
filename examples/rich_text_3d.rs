//! Translated 3D text via `bevy_rich_text3d` (feature `rich_text3d`).
//!
//! Run: cargo run --example rich_text_3d --features rich_text3d
//! Press SPACE to cycle through the available locales.

use bevy::prelude::*;
use bevy_rich_text3d::{Text3d, Text3dPlugin, Text3dStyling, TextAtlas};
use bevy_simple_i18n::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(Text3dPlugin {
            load_system_fonts: true,
            ..Default::default()
        })
        .add_plugins(I18nPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, cycle_locale)
        .run();
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // The translated segment: bevy_simple_i18n keeps this entity's
    // FetchedTextSegment in sync with the "hello" key.
    let segment = commands.spawn(I18nText3dSegment::new("hello")).id();

    commands.spawn((
        Text3d::from_extract(segment),
        Text3dStyling {
            size: 64.0,
            ..Default::default()
        },
        Mesh3d::default(),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..Default::default()
        })),
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
