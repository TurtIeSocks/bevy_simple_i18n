# Trace — bevy_simple_i18n @ 0.3.0 (function-level, repo ≈ 630 LOC)

## Dependency under the knife: `rust-i18n` v3

Exact surface consumed (everything else in rust-i18n is unused):

| rust-i18n API | Call site | Role |
|---|---|---|
| `i18n!(<assets dir>)` | codegen by `build.rs:30`, `include!`d at `src/lib.rs:5` | compile-time embed of every locale file under game's `assets/` |
| `t!(key, locale = ..)` | `src/components/utils.rs:49` | key → translated string |
| `replace_patterns(s, keys, vals)` | `src/components/utils.rs:51` | `%{name}` interpolation |
| `set_locale(&str)` | `src/resources.rs:29` | writes GLOBAL atomic locale (outside ECS) |
| `locale()` | `src/resources.rs:46`, `i18n_text.rs:59`, `i18n_text_2d.rs:56`, `i18n_number.rs:44` | reads global locale |
| `available_locales!()` | `src/resources.rs:47` | compile-time locale list |

## Modules

### build.rs (243 LOC) `[COMPLEX]` `[HOT]`
- `main()` — emits `$OUT_DIR/bevy_simple_i18n.rs` containing (a) `rust_i18n::i18n!(path)` invocation, (b) generated `FONT_FAMILIES: &[FontFamily]` const from scanning `assets/fonts/**`.
- `resolve_asset_dir()` — heuristic: `$BEVY_ASSET_PATH` env override → walk up from `OUT_DIR` to `target/`, sibling `assets/` (or `imported_assets/Default`). Fails silently on docs.rs; warns otherwise. Known wart for workspaces (README section exists because of it; issue #6 fallout).
- `visit_dirs`, `FontFamily::write/push_const/snake_case` — codegen helpers for font constants.
- Both jobs (locale embed + font discovery) exist ONLY because content is baked at compile time.

### src/lib.rs (11 LOC)
- `include!(OUT_DIR/bevy_simple_i18n.rs)` + prelude re-exports.

### src/plugin.rs (106 LOC)
- `I18nPlugin::build` — init/register `I18n`, `FontManager`, components; `PreStartup` `load_dynamic_fonts`; registers built-in components via `register_i18n_component`.
- `I18nComponentRegistration::register_i18n_component<T>` (public trait on `App`) — adds `update_translations::<T>` to `Update`.
- `update_translations<T: I18nComponent>` — change-detection driven: re-translates when `Res<I18n>` changed or `Ref<T>` changed; writes `**target = key.translate()`; syncs `TextFont.font` from `FontManager` when `I18nFont` present. **Already idiomatic — keep shape.**
- `load_dynamic_fonts` — iterates build-generated `FONT_FAMILIES`, `asset_server.load(path)` per font file into `FontManager`.

### src/resources.rs (118 LOC)
- `I18n` resource — `locales: Vec<String>` (from `available_locales!` at startup), `current: String`. `set_locale` validates via `icu_locid::Locale` parse then **forwards to rust-i18n global** — double source of truth `[COMPLEX]`.
- `FontFolder::get(locale)` — BCP-47 truncation fallback `en-US → en → fallback`. Good logic, reusable for translations too.
- `FontManager` — `family → FontFolder` map resource.

### src/components/ (321 LOC)
- `I18nComponent` trait (`mod.rs`) — `type Target: Component + DerefMut<Target=String>`; `locale()`; `translate()`. Public extension point. `locale()` reads rust-i18n GLOBAL, not `Res<I18n>` `[COMPLEX]` — trait method has no World access, forced by global-state design.
- `I18nText` (`#[require(Text)]`), `I18nText2d` (`#[require(Text2d)]`), `I18nNumber` (`#[require(Text)]`, feature `numbers`) — key + args + optional per-entity locale override; builders `new/with_locale/with_arg/with_num_arg`.
- `InterpolationType` — `String | Number(FixedDecimal)`.
- `utils.rs` — `translate_by_key` (t! + replace_patterns + icu number formatting of num args), `f64_to_fd`, `get_formatter` (icu_decimal, panics on bad locale `[COMPLEX]`).
- `I18nFont(String)` (`#[require(TextFont)]`) — family name only.

### tests/translation.rs (96 LOC) `[UNTESTED-adjacent]`
- 4 e2e tests: forced locale, global locale change, Text2d regression (issue #7), interpolation. All depend on compile-time-embedded `assets/locales/*`. Will need rework for async asset loading (app.update() loops until loaded).

### Assets
- `assets/locales/{en,ja,zh-TW}.json` (v1 format: filename = locale) + `v2_example.yml` (v2: `_version: 2`, per-key locale maps).
- `assets/fonts/NotoSans/{fallback,ja,zh}.ttf` (implied by web/dist).
- `web/` — Trunk wasm demo workspace member. **wasm ships locales twice**: baked into the .wasm binary AND present under `web/dist/assets/locales/`.

## Signals
- `[HOT]` build.rs — 3 of last 5 commits touch it (issue #6 fix, 0.19 port).
- Locale data flows: file → build.rs codegen → static in binary → `t!` lookup. No runtime path at all: no hot reload, no downloadable language packs, no mod support.
- Two locale sources of truth (`I18n.current` + rust-i18n global) already caused the issue-#7 class of desync bugs.
