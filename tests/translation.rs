//! End-to-end tests that the i18n components actually translate, and react to locale changes.
//!
//! These run a real (headless) Bevy `App` with the real `assets/` folder, so they cover
//! the full runtime pipeline: manifest asset -> translation-file assets -> `I18n`
//! resource -> component update systems. The parity vectors mirror the exact observable
//! behavior of the old rust-i18n backend (see
//! docs/systematic-refactor/research/rust-i18n-replication-spec.md §7).

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::text::Font;
use bevy_simple_i18n::prelude::*;

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Font>()
        .add_plugins(I18nPlugin::default());
    app
}

/// Translations now load asynchronously through the asset server; pump the app until
/// the manifest and all its translation files are in.
fn advance_until_ready(app: &mut App) {
    for _ in 0..2_000 {
        app.update();
        if app.world().resource::<I18n>().ready() {
            // One more frame so systems that react to the freshly-built table run.
            app.update();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("i18n assets did not become ready within 2s");
}

fn ready_app() -> App {
    let mut app = test_app();
    advance_until_ready(&mut app);
    app
}

fn text(app: &App, entity: Entity) -> String {
    app.world()
        .get::<Text>(entity)
        .expect("the `#[require(Text)]` attribute should have inserted a Text component")
        .0
        .clone()
}

fn spawn_text(app: &mut App, bundle: impl Bundle) -> Entity {
    let id = app.world_mut().spawn(bundle).id();
    app.update();
    id
}

// Several vectors assert values that v2_example.yml (later in the manifest) overrides;
// they only hold when the YAML file actually loads, hence the `yaml` feature gates.

#[cfg(feature = "yaml")]
#[test]
fn translates_with_a_forced_locale() {
    let mut app = ready_app();
    let en = spawn_text(&mut app, I18nText::new("hello").with_locale("en"));
    let ja = spawn_text(&mut app, I18nText::new("hello").with_locale("ja"));

    assert_eq!(text(&app, en), "Hello world");
    assert_eq!(text(&app, ja), "こんにちは世界");
}

#[cfg(feature = "yaml")]
#[test]
fn updates_when_the_global_locale_changes() {
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("hello"));

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(text(&app, id), "Hello world");

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(text(&app, id), "こんにちは世界");
}

#[cfg(feature = "yaml")]
#[test]
fn text2d_updates_when_the_global_locale_changes() {
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText2d::new("hello"));

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(app.world().get::<Text2d>(id).unwrap().0, "Hello world");

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(app.world().get::<Text2d>(id).unwrap().0, "こんにちは世界");
}

#[cfg(feature = "yaml")]
#[test]
fn text_span_translates_and_updates_on_locale_change() {
    let mut app = ready_app();
    let root = app.world_mut().spawn(Text::default()).id();
    let span = app.world_mut().spawn(I18nTextSpan::new("hello")).id();
    app.world_mut().entity_mut(root).add_child(span);
    app.update();

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(app.world().get::<TextSpan>(span).unwrap().0, "Hello world");

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(
        app.world().get::<TextSpan>(span).unwrap().0,
        "こんにちは世界"
    );
}

#[cfg(feature = "yaml")]
#[test]
fn interpolates_arguments() {
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("messages.hello")
            .with_arg("name", "Bevy")
            .with_locale("en"),
    );

    assert_eq!(text(&app, id), "Hello, Bevy");
}

// ------------------------- rust-i18n parity vectors -------------------------

#[cfg(feature = "yaml")]
#[test]
fn locale_truncation_chain_resolves_parent_locales() {
    let mut app = ready_app();
    // zh-TW-whatever -> zh-TW (exact table entry)
    let zh = spawn_text(
        &mut app,
        I18nText::new("hello").with_locale("zh-TW-whatever"),
    );
    // en-US -> en
    let en = spawn_text(&mut app, I18nText::new("hello").with_locale("en-US"));

    assert_eq!(text(&app, zh), "你好世界");
    assert_eq!(text(&app, en), "Hello world");
}

#[cfg(feature = "yaml")]
#[test]
fn underscore_locale_is_normalized_not_fatal() {
    // Every platform's locale API encourages `en_US`-style underscores; `with_locale`
    // must normalize to hyphens instead of handing an unparseable tag to the icu
    // formatter (which used to panic on it).
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("hello").with_locale("ja_JP"));

    assert_eq!(text(&app, id), "こんにちは世界");
}

#[cfg(feature = "yaml")]
#[test]
fn invalid_locale_falls_back_to_global_not_panic() {
    // A locale that isn't a BCP-47 tag at all: logged and ignored, `self.locale` stays
    // `None`, so the entity renders with the global locale instead of crashing.
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("hello").with_locale("not a locale!"),
    );

    assert_eq!(text(&app, id), "Hello world");
}

#[test]
fn missing_key_echoes_the_key_verbatim() {
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("does.not.exist").with_locale("en"));

    assert_eq!(text(&app, id), "does.not.exist");
}

#[test]
fn no_implicit_fallback_to_default_locale() {
    // "text2d" exists only in en.json; requesting it for ja must MISS (key echo),
    // exactly like rust-i18n with no `fallback` configured.
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("text2d").with_locale("ja"));

    assert_eq!(text(&app, id), "text2d");
}

#[cfg(feature = "yaml")]
#[test]
fn later_manifest_files_override_earlier_ones() {
    // en.json says "Hello World" (capital W); v2_example.yml, listed later in the
    // manifest, overrides it with "Hello world" — the merge order the old glob-based
    // backend happened to produce and the tests always relied on.
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("hello").with_locale("en"));

    assert_eq!(text(&app, id), "Hello world");
}

#[cfg(feature = "yaml")]
#[test]
fn unmatched_interpolation_pattern_stays_verbatim() {
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("messages.hello").with_locale("en"));

    assert_eq!(text(&app, id), "Hello, %{name}");
}

#[cfg(feature = "yaml")]
#[test]
fn available_locales_are_sorted_and_complete() {
    let app = ready_app();
    let i18n = app.world().resource::<I18n>();
    let locales = i18n.locales();

    let mut sorted = locales.to_vec();
    sorted.sort();
    assert_eq!(locales, &sorted[..], "locales must be sorted ascending");
    for expected in ["en", "ja", "zh-TW", "cs", "uk"] {
        assert!(
            locales.iter().any(|l| l == expected),
            "expected locale {expected} in {locales:?}"
        );
    }
}

#[test]
fn default_locale_is_en_before_assets_arrive() {
    let app = test_app();
    assert_eq!(app.world().resource::<I18n>().current(), "en");
}

#[cfg(feature = "yaml")]
#[test]
fn text_spawned_before_assets_load_self_heals() {
    // An entity spawned while the table is still empty renders its key, then
    // re-translates automatically once the assets arrive (change detection on I18n).
    let mut app = test_app();
    let id = app
        .world_mut()
        .spawn(I18nText::new("hello").with_locale("ja"))
        .id();
    app.update();
    assert_eq!(text(&app, id), "hello", "pre-load render echoes the key");

    advance_until_ready(&mut app);
    assert_eq!(text(&app, id), "こんにちは世界");
}

#[cfg(feature = "yaml")]
#[test]
fn modified_translation_assets_retranslate_live_text() {
    // Drives the same code path a file_watcher hot reload takes: mutating a
    // `TranslationFile` asset emits `AssetEvent::Modified`, the sync system rebuilds
    // the table, and live text re-translates — no respawn, no manual refresh.
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("hello").with_locale("ja"));
    assert_eq!(text(&app, id), "こんにちは世界");

    let mut files = app.world_mut().resource_mut::<Assets<TranslationFile>>();
    let ids: Vec<_> = files.ids().collect();
    for asset_id in ids {
        // `get_mut` intentionally flags the asset as Modified.
        let mut file = files.get_mut(asset_id).unwrap();
        file.set_translation("ja", "hello", "アップデート済み");
    }

    app.update(); // sync_translations sees Modified, rebuilds the table
    app.update(); // update_translations reacts to the changed I18n resource
    assert_eq!(text(&app, id), "アップデート済み");
}

#[cfg(feature = "plurals")]
#[test]
fn plural_categories_select_the_right_form() {
    let mut app = ready_app();
    let one = spawn_text(
        &mut app,
        I18nText::new("cats").with_count(1).with_locale("en"),
    );
    let many = spawn_text(
        &mut app,
        I18nText::new("cats").with_count(3).with_locale("en"),
    );

    assert_eq!(text(&app, one), "You have 1 cat");
    assert_eq!(text(&app, many), "You have 3 cats");
}

#[cfg(feature = "plurals")]
#[test]
fn non_finite_counts_do_not_panic() {
    // A NaN/infinite count is a data bug, not a reason to crash the game: it logs an
    // error and behaves as if no count was set (bare key -> the usual miss echo,
    // since en.json ships no bare "cats" key).
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("cats").with_count(f64::NAN).with_locale("en"),
    );

    assert_eq!(text(&app, id), "cats");
}

#[cfg(feature = "plurals")]
#[test]
fn exact_count_key_overrides_the_cldr_category() {
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("cats").with_count(0).with_locale("en"),
    );

    assert_eq!(text(&app, id), "You have no cats");
}

#[cfg(feature = "numbers")]
#[test]
fn numbers_localize_per_locale() {
    // Characterization tests for the icu formatting pipeline (guards the icu 2.x bump).
    let mut app = ready_app();
    let en = spawn_text(&mut app, I18nNumber::new(24501.2).with_locale("en"));
    let de = spawn_text(&mut app, I18nNumber::new(24501.2).with_locale("de"));

    assert_eq!(text(&app, en), "24,501.2");
    assert_eq!(text(&app, de), "24.501,2");
}

#[cfg(all(feature = "numbers", feature = "yaml"))]
#[test]
fn number_interpolation_arguments_are_localized() {
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("messages.cats")
            .with_num_arg("count", 2000.3)
            .with_locale("en"),
    );

    assert_eq!(text(&app, id), "You have 2,000.3 cats");
}

#[cfg(all(feature = "numbers", feature = "yaml"))]
#[test]
fn set_num_arg_retranslates_live_text() {
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("messages.cats")
            .with_num_arg("count", 1)
            .with_locale("en"),
    );
    assert_eq!(text(&app, id), "You have 1 cats");

    app.world_mut()
        .get_mut::<I18nText>(id)
        .unwrap()
        .set_num_arg("count", 2000.3);
    app.update();
    assert_eq!(text(&app, id), "You have 2,000.3 cats");
}

#[cfg(feature = "yaml")]
#[test]
fn set_key_retranslates() {
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("hello").with_locale("en"));
    assert_eq!(text(&app, id), "Hello world");

    app.world_mut()
        .get_mut::<I18nText>(id)
        .unwrap()
        .set_key("text2d");
    app.update();
    assert_eq!(text(&app, id), "Hello World (Text2d)");
}

#[cfg(feature = "plurals")]
#[test]
fn set_count_retranslates() {
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("cats").with_count(1).with_locale("en"),
    );
    assert_eq!(text(&app, id), "You have 1 cat");

    app.world_mut()
        .get_mut::<I18nText>(id)
        .unwrap()
        .set_count(3);
    app.update();
    assert_eq!(text(&app, id), "You have 3 cats");
}

#[cfg(feature = "numbers")]
#[test]
fn set_number_reformats() {
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nNumber::new(1.5).with_locale("de"));
    assert_eq!(text(&app, id), "1,5");

    app.world_mut()
        .get_mut::<I18nNumber>(id)
        .unwrap()
        .set_number(24501.2);
    app.update();
    assert_eq!(text(&app, id), "24.501,2");
}

#[cfg(feature = "yaml")]
#[test]
fn invalid_set_locale_keeps_previous() {
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText::new("hello").with_locale("ja"));
    assert_eq!(text(&app, id), "こんにちは世界");

    app.world_mut()
        .get_mut::<I18nText>(id)
        .unwrap()
        .set_locale("not a locale!");
    app.update();
    assert_eq!(text(&app, id), "こんにちは世界");
}

#[cfg(feature = "numbers")]
#[test]
fn nan_number_renders_empty_not_panic() {
    // A NaN/infinite I18nNumber is a data bug, not a crash: it logs an error at
    // construction time and renders as an empty string.
    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nNumber::new(f64::NAN));

    assert_eq!(text(&app, id), "");
}

#[cfg(all(feature = "numbers", feature = "yaml"))]
#[test]
fn nan_num_arg_leaves_pattern_verbatim() {
    // A non-finite `with_num_arg` value is logged and skipped (not pushed as an
    // interpolation arg), so the `%{count}` pattern stays verbatim — the crate's
    // established "visible, not invisible" failure mode.
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        I18nText::new("messages.cats")
            .with_num_arg("count", f64::INFINITY)
            .with_locale("en"),
    );

    assert_eq!(text(&app, id), "You have %{count} cats");
}

#[test]
fn dynamic_font_family_is_applied_from_the_manifest() {
    let mut app = ready_app();
    let id = spawn_text(
        &mut app,
        (
            I18nText::new("hello").with_locale("ja"),
            I18nFont::new("NotoSans"),
        ),
    );

    let font = &app.world().get::<TextFont>(id).unwrap().font;
    let default_font = TextFont::default().font;
    assert_ne!(
        format!("{font:?}"),
        format!("{default_font:?}"),
        "I18nFont should have swapped the font source for the ja locale"
    );
}
