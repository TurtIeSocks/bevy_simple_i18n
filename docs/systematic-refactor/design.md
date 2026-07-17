# Design — native Bevy i18n for bevy_simple_i18n 0.4.0

Companion to [map.md](./map.md) (old→new). Research ground truth in [research/](./research/).

## One-paragraph summary

Delete build.rs and rust-i18n. Locale files become real Bevy `Asset`s (`TranslationFile`, one per file, same JSON/YAML/TOML v1+v2 formats as today) discovered through a tiny RON manifest asset (`I18nManifest`, default `assets/locales/i18n.ron`) — the manifest exists because wasm cannot list directories, which is the one legitimate job build.rs was doing. A sync system folds loaded files into the `I18n` resource (now the single source of truth: current locale + merged table + fallback chain), and the existing change-detection-driven `update_translations::<T>` system keeps working untouched in shape — locale switches AND hot reloads flow through the same `Res<I18n>` changed tick. Public component API (`I18nText::new("key").with_arg(..)`, `I18nNumber`, `I18nFont`, `register_i18n_component`) is frozen; breakage concentrates in setup (add one manifest file) and the `I18nComponent` trait signature.

## Data flow

```
assets/locales/i18n.ron ──AssetServer──▶ I18nManifest ─┐
assets/locales/en.json ──(dep load)───▶ TranslationFile ├─▶ sync_store ──▶ Res<I18n>
assets/locales/v2_example.yml ────────▶ TranslationFile ┘   (on AssetEvent)      │ (is_changed)
                                                                                 ▼
user: i18n.set_locale("ja") ────────────────────────────────▶ update_translations::<T>
                                                                                 │
                                                              Text / Text2d / TextFont
```

Hot reload: `file_watcher` fires `AssetEvent::Modified` on the edited locale file → sync_store rebuilds → same downstream path. Zero extra machinery. (No hot reload on wasm — bevy excludes notify there; degrades silently.)

## Decisions + rationale

**D1 — Manifest asset over folder scanning.** `load_folder` returns empty + error log on wasm (`HttpWasmAssetReader::read_directory`, bevy_asset 0.19 io/wasm.rs:127-140) and is fragile on Android. bevy_asset_loader hit the same wall (its `Folder` dynamic asset is documented "not supported for web builds", pushing users to explicit `Files` lists). One RON manifest = one code path on every platform, deterministic merge order, and free "language pack" support (drop files + edit manifest at runtime, no recompile — top community ask per bevy discussion #5874). Cost: users list their locale files once. Rejected alternatives: cfg-split load_folder-on-native/manifest-on-wasm (two behaviors, itch.io-web surprise bugs); keeping a manifest-generating build.rs (perpetuates the BEVY_ASSET_PATH wart).

**D2 — Behavior-parity translation semantics, from the extracted spec.** rust-i18n 3.1.5 source was read; the exact contract is in [research/rust-i18n-replication-spec.md](./research/rust-i18n-replication-spec.md) with test vectors. Parity kept: v1/v2 file formats, dot-flattening + leaf coercion, locale truncation chain (`zh-Hant-CN → zh-Hant → zh`, `-x` tail trim), miss → key echoed verbatim, NO implicit default-locale fallback, `%{name}` first-match-wins/unmatched-stays interpolation, `available_locales` sorted byte-lexicographic, default locale "en". Three conscious divergences (all improvements, none test-observable in real projects): `_version` stripped from v1 keys, parse errors log-and-skip instead of build failure, merge order = manifest order instead of unspecified glob order. Opt-in `fallback: ["en"]` manifest field adds the fallback chain users actually want (matches rust-i18n's own `fallback` option semantics) — default stays empty for parity.

**D3 — `I18n` resource is the only locale state.** Today the crate keeps `I18n.current` AND rust-i18n's global `AtomicStr`, and components secretly read the global — the issue-#7 class of desync. New trait signature passes the resource in: `translate(&self, i18n: &I18n)`. ECS-pure, trivially testable, and per-entity `.with_locale()` override logic becomes explicit.

**D4 — Fonts declared in the same manifest.** The build-time font scan (`FONT_FAMILIES` codegen) is replaced by a `fonts:` section. `FontManager`/`FontFolder` resources and the truncation lookup survive unchanged; only the population source changes (manifest instead of generated const). Same wasm reasoning as D1.

**D5 — Numbers pipeline untouched by the rewrite; icu bumped separately.** icu_decimal/fixed_decimal never depended on rust-i18n. The icu 2.x migration (icu_locid deprecated → icu_locale_core 2.2, FixedDecimal→Decimal 0.7, FixedDecimalFormatter→DecimalFormatter 2.2 with preference-bag ctors) is mechanical and lands as its own commit inside the 0.4 release.

**D6 — Simple key-value stays; no Fluent, no MF2.** bevy_fluent (0.15, bevy 0.19) already owns the Fluent niche and its #11277 upstreaming review documents the ergonomic costs. MessageFormat 2.0 has no production Rust implementation (ICU4X hasn't shipped it as of 2026-07). The crate's differentiator is "dead simple + reactive". CLDR plurals via icu_plurals sub-key convention (bevy-intl-proven shape) is the 0.5 growth path if demanded.

## Readiness / async gap

Assets load async; an `I18nText` spawned before the table arrives renders its key. When sync_store fills the table, `Res<I18n>` changes → everything re-translates. Self-healing, no state machine. `I18n::ready()` exposed (manifest `LoadedWithDependencies` observed) for load-screen gating; tests pump `app.update()` until ready.

## Risks

| Risk | Mitigation |
|---|---|
| Users forget manifest / wrong path | Startup warn! with the exact expected path when manifest fails to load (AssetLoadFailedEvent) |
| `.json` extension collision with other loaders | Typed dep loads (`load_context.load::<TranslationFile>`) — bevy dispatches by asset type, extension only tiebreaks within a type |
| Merge-order regressions vs glob order | Manifest order is explicit; parity test vectors pinned in tests |
| Third-party `I18nComponent` impls break | Documented breaking change in 0.4 migration notes; trait is tiny |
| Text flashes raw key on slow loads (web) | Documented; `ready()` + states recipe in README; optional future `embedded_asset!` defaults |

## Assumptions (delegate-mode judgement calls, review these)

1. 0.4.0 with breaking changes is acceptable — README already promises them.
2. Manifest (one extra file) is an acceptable DX cost to kill build.rs on all platforms; no native-only folder-scan sugar in v1 of the rewrite.
3. RON for the manifest (Bevy-ecosystem convention: bevy_asset_loader `.assets.ron`, bevy_fluent `.ftl.ron`) rather than JSON.
4. Parity default of NO implicit fallback-to-default-locale (miss echoes key) — improvement is opt-in via manifest `fallback` field.
5. yaml/toml parsing stays default-on (compat with documented formats), feature-gated for slim builds.
6. System-locale auto-detect deferred to 0.4.x (bevy_device_lang preferred over sys-locale for Android correctness).
7. Estimated size: crate net-shrinks (−243-line build.rs, −rust-i18n; +~150 LOC loaders/parse, +~60 LOC sync/manifest).
