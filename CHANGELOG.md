# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - Unreleased

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
