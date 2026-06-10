use bevy::prelude::*;

use bevy_simple_i18n::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Add the base plugin
        .add_plugins(I18nPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn(Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(20.),
            ..Default::default()
        })
        .with_children(|parent| {
            // Basic usage of the i18n text component
            parent.spawn((
                // i18n text component with key "hello"
                I18nText::new("hello"),
                // Dynamic font component with font family "NotoSans" that auto loads font files based on the set locale
                I18nFont::new("NotoSans"),
                // You can still insert a TextFont component though
                // Keep in mind that the "font" field will be overridden by the I18nFont component
                TextFont {
                    font_size: FontSize::Px(40.0),
                    ..default()
                },
            ));

            // Basic usage of the i18n number component
            parent.spawn((I18nNumber::new(24501.20), I18nFont::new("NotoSans")));

            // Interpolation example
            parent.spawn((
                // You can add as many arguments as you want to the translation
                I18nText::new("messages.hello")
                    .with_arg("name", "Bevy User")
                    // You can also specify the locale for this specific translation
                    // This overrides the globally set locale
                    .with_locale("ja"),
                // Dynamic font component with font family "NotoSans" that auto loads font files based on the set locale
                I18nFont::new("NotoSans"),
            ));
        });

    // Basic usage of the Text2d implementation
    commands.spawn((
        // I18nText2d component with key "text2d"
        I18nText2d::new("text2d"),
        // Dynamic font component with font family "NotoSans" that auto loads font files based on the set locale
        I18nFont::new("NotoSans"),
        // You can still insert a TextFont component though
        // Keep in mind that the "font" field will be overridden by the I18nFont component
        TextFont {
            font_size: FontSize::Px(40.0),
            ..default()
        },
        // Since we're using Text2d, we add a Transform to set its position
        Transform::from_xyz(300., 300., 0.),
    ));
}
