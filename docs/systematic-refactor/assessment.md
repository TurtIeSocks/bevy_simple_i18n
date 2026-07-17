# Assessment — keep / rewrite / delete

### build.rs
- Verdict: **Delete** (entirely)
- Evidence: [HOT][COMPLEX] — exists only to feed compile-time embedding (locale codegen + font-family discovery codegen)
- Tension with goals: IS the anti-pattern being removed; source of BEVY_ASSET_PATH wart, docs.rs hack, wasm double-ship
- Confidence: High. Caveat: it also solves *asset discovery* (which files exist) — replacement design must re-answer discovery at runtime (wasm can't `read_dir`).

### src/lib.rs — `include!(OUT_DIR/...)`
- Verdict: **Delete** the include; keep prelude
- Confidence: High

### src/plugin.rs — `I18nPlugin`, `register_i18n_component`, `update_translations`
- Verdict: **Keep / refactor in place**
- Evidence: change-detection-driven update system already idiomatic; trait-registration extension pattern is good public API
- Changes: `update_translations` must read translations from a resource/assets instead of `T::translate()` calling globals; add asset-event-driven re-translate; `load_dynamic_fonts` re-sourced from config/manifest instead of generated const
- Confidence: High

### src/resources.rs — `I18n`
- Verdict: **Rewrite**
- Evidence: [COMPLEX] double source of truth — mirrors rust-i18n global; `locales` frozen at compile time
- New shape: single source of truth; holds current locale + fallback chain + loaded translation tables (or handle to them); `set_locale` stays same signature
- Confidence: High

### src/resources.rs — `FontFolder::get` (BCP-47 truncation fallback)
- Verdict: **Keep**, generalize
- Evidence: `en-US → en → fallback` walk is exactly the locale-fallback logic translations need too; promote to shared helper
- Confidence: High

### src/components/* — `I18nText`, `I18nText2d`, `I18nNumber`, `I18nFont`, builders
- Verdict: **Keep** (API frozen), internals touched only where they call `rust_i18n::locale()`
- Tension: `I18nComponent::locale()`/`translate()` signatures assume global state — trait needs the store passed in (breaking for third-party impls, acceptable)
- Confidence: High

### src/components/utils.rs — `translate_by_key`
- Verdict: **Rewrite** (small)
- Evidence: 3 of its lines are rust-i18n calls; `%{name}` replace is trivial to own; icu number formatting stays
- Confidence: High

### Number pipeline (icu_decimal, fixed_decimal, icu_locid)
- Verdict: **Keep / defer**
- Evidence: independent of rust-i18n. icu4x 2.x exists (renames/breaking) — separate upgrade, do NOT couple to this refactor
- Confidence: Medium (version-bump question delegated to research)

### tests/translation.rs
- Verdict: **Refactor in place**
- Evidence: assertions stay valid; setup must pump `app.update()` until assets load (or use direct-insert of translation data for determinism)
- Confidence: High

### assets/locales/*, assets/fonts/*
- Verdict: **Keep** — file formats are the compatibility contract
- Confidence: High

### web/ demo
- Verdict: **Keep** — becomes the wasm acceptance test for runtime loading
- Confidence: High

## Tally
Delete: build.rs + include. Rewrite: I18n resource, translate_by_key. Keep/refactor: plugin, components, tests, assets, web. Net: crate gets SMALLER (243-line build.rs dies, replaced by an AssetLoader ~100 LOC).
