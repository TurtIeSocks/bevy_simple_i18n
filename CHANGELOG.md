# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - Unreleased

### Changed

- **Dropped `rust-i18n`: locale files are now real Bevy assets loaded at runtime.** The
  build script (and its compile-time embedding of every locale file into the binary) is
  gone. Translations load through the `AssetServer` like any other asset, driven by a
  small RON manifest (`assets/locales/i18n.ron` by default) that lists the locale files
  and dynamic font families. The manifest replaces directory scanning because wasm and
  Android builds cannot enumerate asset folders.
- **Hot reload.** With Bevy's `file_watcher` feature enabled, editing a locale file
  re-translates all live text instantly. `TranslationFile::set_translation` drives the
  same path programmatically (e.g. for in-game translation tooling).
- **Runtime language packs.** Because nothing is baked into the binary anymore, shipping
  additional or corrected translations is now a pure asset change — no recompile.
- The `I18n` resource is the single source of truth for locale state (current locale,
  merged translation table, available locales, fallback chain). The rust-i18n global
  locale is gone, along with an entire class of desync bugs.
- Translation lookups keep rust-i18n's exact observable behavior, pinned by tests:
  BCP-47 truncation fallback (`zh-Hant-CN` → `zh-Hant` → `zh`, `-x` tails trimmed), a
  complete miss renders the key verbatim, no implicit fallback to the default locale
  (opt in via the manifest's `fallback` list), `%{name}` interpolation semantics
  (first match wins, unmatched patterns stay verbatim), sorted `locales()` list.
  Both locale file formats (v1 per-locale files, v2 `_version: 2`) are still supported
  in JSON, YAML and TOML.
- Migrated the icu stack to ICU4X 2.x: `icu_locid` → `icu_locale_core`,
  `fixed_decimal 0.5` → `0.7` (`Decimal`), `icu_decimal 1.5` → `2.x`
  (`DecimalFormatter`). Number formatting output is unchanged.
- New crate features: `yaml` and `toml` (both default-on) gate the respective locale
  file formats; JSON and the RON manifest are always available.

### Breaking changes

- A locale manifest is now required (default path `assets/locales/i18n.ron`):

  ```ron
  (
      default_locale: "en",
      files: ["en.json", "ja.json"], // order = merge order, later files win
      fonts: [(family: "NotoSans", dir: "fonts/NotoSans", files: ["fallback.ttf", "ja.ttf"])],
  )
  ```

- `I18nPlugin` is no longer a unit struct: use `I18nPlugin::default()` or
  `I18nPlugin::with_manifest("path/to/manifest.ron")`.
- `I18nComponent::locale` and `I18nComponent::translate` now take `&I18n` (locale state
  lives in ECS, not in a global).
- The `BEVY_ASSET_PATH` build-time environment variable is gone — obsolete now that
  assets resolve at runtime like every other Bevy asset (this also removes the special
  setup for workspace projects and docs.rs).
- Translation parse errors are logged at runtime (the asset fails to load) instead of
  failing the build.
- Removed dependencies: `rust-i18n`, `cargo-emit`. Removed: `build.rs`.

## [0.3.0] - 2026-06-19

### Changed

- **Updated to Bevy 0.19.** This is the headline change and is a breaking upgrade; see the
  [Bevy 0.16](https://bevy.org/learn/migration-guides/0-15-to-0-16/),
  [0.17](https://bevy.org/learn/migration-guides/0-16-to-0-17/),
  [0.18](https://bevy.org/learn/migration-guides/0-17-to-0-18/) and
  [0.19](https://bevy.org/learn/migration-guides/0-18-to-0-19/) migration guides for engine-level
  details.
- Bevy 0.19 reworked the text API: `TextFont::font` is now a `FontSource` enum (a `Handle<Font>`
  converts via `.into()`) and `TextFont::font_size` is now a `FontSize` enum (`FontSize::Px(..)`).
- The four built-in components (`I18nText`, `I18nText2d`, `I18nNumber`, `I18nFont`) are now plain
  `#[derive(Component)]` types that use the `#[require(..)]` attribute to insert their target text
  component, instead of hand-written `Component` implementations with `on_add` hooks.
- Translation updates are now handled by a single change-detection system per component type
  (using `Ref<T>` + resource change detection) instead of component hooks plus locale-change
  systems. Initial spawn and locale changes flow through the same code path.
- The `I18nComponent` trait now has a `Component` supertrait and an associated
  `type Target: Component<Mutability = Mutable> + DerefMut<Target = String>` describing which text
  component it drives.
- Reflect types (`I18n`, `I18nText`, `I18nText2d`, `I18nNumber`, `I18nFont`) are now registered
  with the type registry by the plugin.

### Fixed

- **`I18nText2d` now re-translates when the locale changes.** The shared update system previously
  queried `&mut Text` for every registered component, so `Text2d`-backed entities were never
  updated after their initial spawn.
- **The crate now compiles even when no `assets` folder is found.** The build script always emits a
  `rust_i18n::i18n!(..)` invocation (falling back to `"locales"`), so the `t!` macro always has a
  backend to expand into. ([#6](https://github.com/TurtIeSocks/bevy_simple_i18n/issues/6))
- Asset/font paths emitted by the build script are now always forward-slashed for consistent
  resolution across platforms (including wasm).
- The build script's "asset folder not found" message is now a clear, actionable warning that
  points at `BEVY_ASSET_PATH`.

### Removed

- The internal `FontsLoading` resource and `monitor_font_loading` system. Text now renders
  immediately and fonts stream in asynchronously like any other Bevy asset.

### Docs

- Documented `BEVY_ASSET_PATH` and workspace-project setup in the README.
  ([#6](https://github.com/TurtIeSocks/bevy_simple_i18n/issues/6))

### Notes for maintainer

- This release should be published to crates.io; the currently published `0.1.2` predates the
  build-script fixes and does not translate in many setups.
  ([#7](https://github.com/TurtIeSocks/bevy_simple_i18n/issues/7))

## [0.1.x]

- Initial releases targeting Bevy 0.15.
