//! End-to-end tests that the i18n components actually translate, and react to locale changes.
//!
//! These run a real (headless) Bevy `App`, so they double as a regression guard for the
//! "doesn't translate anything" report in
//! <https://github.com/TurtIeSocks/bevy_simple_i18n/issues/7>.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::text::Font;
use bevy_simple_i18n::prelude::*;

/// Builds the smallest `App` that can drive the plugin: an asset server (so the font loader has
/// somewhere to read from) plus the registered `Font` asset that `load_dynamic_fonts` produces.
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Font>()
        .add_plugins(I18nPlugin);
    app
}

fn text(app: &App, entity: Entity) -> String {
    app.world()
        .get::<Text>(entity)
        .expect("the `#[require(Text)]` attribute should have inserted a Text component")
        .0
        .clone()
}

#[test]
fn translates_with_a_forced_locale() {
    let mut app = test_app();
    let en = app
        .world_mut()
        .spawn(I18nText::new("hello").with_locale("en"))
        .id();
    let ja = app
        .world_mut()
        .spawn(I18nText::new("hello").with_locale("ja"))
        .id();

    app.update();

    assert_eq!(text(&app, en), "Hello world");
    assert_eq!(text(&app, ja), "こんにちは世界");
}

#[test]
fn updates_when_the_global_locale_changes() {
    let mut app = test_app();
    let id = app.world_mut().spawn(I18nText::new("hello")).id();

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(text(&app, id), "Hello world");

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(text(&app, id), "こんにちは世界");
}

/// Regression test: before the 0.18 rewrite the update system hard-coded `&mut Text`, so
/// `I18nText2d` entities (which carry `Text2d`, not `Text`) never re-translated when the locale
/// changed. The associated `I18nComponent::Target` now drives the correct component.
#[test]
fn text2d_updates_when_the_global_locale_changes() {
    let mut app = test_app();
    let id = app.world_mut().spawn(I18nText2d::new("hello")).id();

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(app.world().get::<Text2d>(id).unwrap().0, "Hello world");

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(app.world().get::<Text2d>(id).unwrap().0, "こんにちは世界");
}

#[test]
fn interpolates_arguments() {
    let mut app = test_app();
    let id = app
        .world_mut()
        .spawn(
            I18nText::new("messages.hello")
                .with_arg("name", "Bevy")
                .with_locale("en"),
        )
        .id();

    app.update();

    assert_eq!(text(&app, id), "Hello, Bevy");
}
