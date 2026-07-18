# Goals — drop rust-i18n, go native Bevy

Source: user request 2026-07-17 ("drop this dependency and create our own i18n system that feels like NATIVE bevy i18n") + README §Project Status (maintainer: "long term goal is to create a more Bevy-like internationalization library... expect breaking changes").

## Driving goals (checklist picks)
1. **Dependency reduction** — remove `rust-i18n` (and with it the entire build.rs codegen pipeline).
2. **Architecture / idiomatic Bevy** — locale data as first-class `Asset`s loaded at runtime through `AssetServer`; locale state lives ONLY in ECS (`Res<I18n>`); systems react via change detection / asset events. No global statics, no `include!` codegen.

## Derived requirements
- **Hot reload**: editing a locale file under `assets/` updates on-screen text live (bevy `file_watcher`). Impossible today; flagship win of the migration.
- **No build.rs**: kills the `BEVY_ASSET_PATH` workspace wart, docs.rs special-casing, and double-shipping locales on wasm.
- **wasm parity**: web demo (Trunk) must keep working. Constrains asset discovery (no `read_dir` on wasm).
- **Keep the public component API stable-ish**: `I18nText::new("key").with_arg(..).with_locale(..)`, `I18nNumber`, `I18nText2d`, `I18nFont`, `register_i18n_component::<T>` — these are good and "already Bevy". Breakage should concentrate in setup/plugin config, not call sites.
- **Keep locale-file formats compatible** (v1 per-locale json/yaml/toml + v2 `_version: 2`) so existing users migrate without rewriting translations. `%{name}` interpolation syntax stays.
- **Number localization stays** (icu_decimal/fixed_decimal) — orthogonal to rust-i18n, unaffected.

## Constraints
- Breaking changes allowed: README warns, pre-1.0 crate, next release can be 0.4.0. `feat!` precedent exists (Bevy 0.19 port).
- Solo maintainer; crate motto is "dead simple" — reject heavyweight solutions (full Fluent runtime) unless research shows simple ones fail.
- Bevy 0.19, edition 2021.
- Migration strategy: big-bang within the crate (surface is tiny), one release. No parallel-run/strangler needed at 630 LOC.

## Non-goals (this pass)
- Fluent/ICU MessageFormat grammar (plurals/genders) — note as future work unless research overturns.
- Locale auto-detection from OS — candidate nice-to-have, decide from research (sys-locale wasm story).
