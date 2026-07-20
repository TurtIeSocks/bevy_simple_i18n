# Plan 009: Feature-gated `bevy_rich_text3d` integration + working example (fixes the orphan-rule hole in the 008 recipe)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: confirm `src/components/mod.rs` contains
> `pub trait I18nTarget` (plan 008 landed). If missing, STOP.

## Status

- **Priority**: P1 (the README recipe merged by plan 008 is unimplementable — see below; this must land in the same release)
- **Effort**: M
- **Risk**: MED (new optional dependency; new example needs a real third-party API)
- **Depends on**: plan 008 (already on this branch)
- **Category**: bug + direction
- **Planned at**: commit `eb6d2fc`, 2026-07-19

## Why this matters

Plan 008's README recipe tells users to `impl I18nTarget for
FetchedTextSegment` in their own crate. That is an **orphan-rule violation**
(E0117): `I18nTarget` is foreign (ours) and `FetchedTextSegment` is foreign
(bevy_rich_text3d's) from any user crate's perspective — the recipe cannot
compile anywhere, and its `rust,ignore` fence hid that from every checker.
The only first-class fix is the standard ecosystem pattern: this crate (which
owns the trait) provides the impl behind an optional feature. That also
unlocks a real `I18nText3dSegment` component (one macro invocation — plurals,
interpolation, setters for 3D text) and a runnable example, which is what the
maintainer asked for.

## Current state

- `src/components/mod.rs` — `I18nTarget` trait + 3 impls (Text/Text2d/
  TextSpan); `define_i18n_text_component!` macro (takes
  `$(#[$struct_doc:meta])* $name:ident, target: $target:ty`); existing
  invocation exemplar: `src/components/i18n_text_span.rs`.
- `src/plugin.rs` — registration block: `.register_type::<I18nTextSpan>()`
  (~line 66) and `.register_i18n_component::<I18nTextSpan>()` (~line 86);
  cfg-gated exemplar right below:
  ```rust
  #[cfg(feature = "numbers")]
  app.register_type::<crate::components::I18nNumber>()
      .register_i18n_component::<crate::components::I18nNumber>();
  ```
- `Cargo.toml` — `[features]` block (~line 18) with `default = ["numbers",
  "yaml", "toml", "detect", "plurals"]` and optional-dep patterns like
  `detect = ["dep:bevy_device_lang"]`; `[[example]]` entries at the bottom
  with `required-features`.
- `README.md` — section `### Third-party text components (bevy_rich_text3d)`
  (added by 008): its recipe block starts with
  `impl I18nTarget for FetchedTextSegment {` — this is the unimplementable
  text to replace.
- `CHANGELOG.md` — `## [Unreleased]` with the `I18nTarget` bullet under
  `### Added`.
- `.github/workflows/ci.yml` — `build_examples` job runs
  `cargo build --examples`.
- Repo assets usable by the example: `assets/fonts/NotoSans/fallback.ttf`,
  `ja.ttf`, `zh-TW.ttf`, `ko.ttf`, `th.ttf`; locales `en`, `ja`, `zh-TW`,
  `cs`, `uk` via `assets/locales/`.
- Baseline at `eb6d2fc`: `cargo test --all-features` = 39 unit + 30
  integration + 7 doc (1 ignored); `--no-default-features` = 27 + 4 + 6
  (1 ignored); clippy `-D warnings`, rustdoc `-D warnings`, fmt, wasm
  (default features) all clean.

### Verified `bevy_rich_text3d` 0.7 facts (from its docs; re-verify while implementing)

- bevy 0.19; plugin: `Text3dPlugin { load_system_fonts: true, ..Default::default() }`.
- Extra fonts: `LoadFonts { font_paths: vec![...], font_directories: vec![...], ..Default::default() }`
  resource with **filesystem** paths (e.g. `"assets/fonts/NotoSans"`).
- Text entity needs: `Text3d` + `Text3dStyling` + `Text3dBounds` + `Mesh3d::default()` +
  `MeshMaterial3d(materials.add(StandardMaterial { base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()), alpha_mode: AlphaMode::Blend, ..Default::default() }))`.
- `FetchedTextSegment(pub String)` is a mutable Component with `Default`; a
  `Text3d` references it via `Text3dSegment::Extract(entity)` (or
  `Text3d::from_extract(entity)` for a single-segment text).
- `Text3d` also removes linked `FetchedTextSegment` entities on removal
  unless `SharedTextSegment` is added — irrelevant here but don't fight it.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile everything incl. feature | `cargo check --all-targets --all-features` | exit 0 |
| Feature-off regression | `cargo check --all-targets` | exit 0 |
| Tests | `cargo test --all-features` | all pass incl. new test |
| Minimal | `cargo test --no-default-features` | all pass |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` | exit 0 |
| Format | `cargo fmt` then `cargo fmt --check` | exit 0 |
| Wasm (default features — unchanged) | `cargo check --target wasm32-unknown-unknown` | exit 0 |
| Example builds | `cargo build --example rich_text_3d --features rich_text3d` | exit 0 |

## Scope

**In scope**:
- `Cargo.toml` (optional dep + feature + example entry) and `Cargo.lock`
  (mechanical update from adding the dep)
- `src/components/i18n_text_3d_segment.rs` (create)
- `src/components/mod.rs` (cfg-gated module decl + re-export + the
  `I18nTarget for FetchedTextSegment` impl may live here or in the new file —
  put it in the new file)
- `src/plugin.rs` (cfg-gated registration)
- `examples/rich_text_3d.rs` (create)
- `tests/translation.rs` (one cfg-gated test)
- `README.md` (fix the 008 recipe section; features table row)
- `CHANGELOG.md` (entries)
- `.github/workflows/ci.yml` (build_examples job only)

**Out of scope**:
- The wasm CI job stays on default features (bevy_rich_text3d's wasm story is
  unverified — do not add `--all-features` there).
- No changes to the `I18nTarget` trait, macro, or existing components.
- `web/` and the publish workflow.

## Git workflow

- Current worktree branch. Two conventional commits:
  `feat: rich_text3d feature — I18nTarget impl + I18nText3dSegment + example`
  then `docs: fix the orphan-rule-violating rich_text3d recipe` (or one
  commit if the README fix is inseparable; note the choice).
- Do NOT push.

## Steps

### Step 1: Cargo wiring

In `Cargo.toml`:

```toml
# [dependencies]
bevy_rich_text3d = { version = "0.7", optional = true }

# [features]
# I18nTarget impl for bevy_rich_text3d's FetchedTextSegment + the
# I18nText3dSegment component. Off by default: it pins a third-party
# release cadence.
rich_text3d = ["dep:bevy_rich_text3d"]

# [[example]] (bottom, matching the existing entries' style)
[[example]]
name = "rich_text_3d"
path = "examples/rich_text_3d.rs"
required-features = ["rich_text3d"]
```

Run `cargo check --all-targets --all-features` once to materialize the
`Cargo.lock` update; commit the lockfile diff with this work.

**Verify**: `cargo check --all-targets` (feature off) → exit 0, and
`cargo metadata --format-version 1 | grep -c bevy_rich_text3d` → ≥1.

### Step 2: The impl + component

Create `src/components/i18n_text_3d_segment.rs`:

```rust
use bevy_rich_text3d::FetchedTextSegment;

use super::{define_i18n_text_component, I18nTarget};

// This impl lives HERE (not in user code) because of the orphan rule:
// `I18nTarget` is this crate's trait, so this crate may implement it for the
// foreign `FetchedTextSegment` — no user crate can.
impl I18nTarget for FetchedTextSegment {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

define_i18n_text_component!(
    /// Component for translatable `bevy_rich_text3d` text segments (feature
    /// `rich_text3d`).
    ///
    /// A [`FetchedTextSegment`] is inserted automatically (via `#[require(..)]`).
    /// Reference this entity from a [`Text3d`](bevy_rich_text3d::Text3d) via
    /// `Text3dSegment::Extract(entity)` and the segment re-translates on locale
    /// change like any other i18n component — interpolation, plurals and the
    /// in-place setters all work.
    I18nText3dSegment, target: bevy_rich_text3d::FetchedTextSegment
);
```

Wire in `src/components/mod.rs` (mirror the `numbers` cfg pattern):

```rust
#[cfg(feature = "rich_text3d")]
mod i18n_text_3d_segment;
#[cfg(feature = "rich_text3d")]
pub use i18n_text_3d_segment::*;
```

And in `src/plugin.rs` build(), after the `numbers` cfg block:

```rust
#[cfg(feature = "rich_text3d")]
app.register_type::<crate::components::I18nText3dSegment>()
    .register_i18n_component::<crate::components::I18nText3dSegment>();
```

**Verify**: `cargo check --all-targets --all-features` → exit 0. If the
`Reflect` derive on the macro-generated struct fails because
`FetchedTextSegment` lacks reflection: the macro's derive only reflects OUR
struct's fields (key/args/locale/count), so this should not occur — if it
does, STOP and report the error.

### Step 3: Integration test (headless, no Text3dPlugin needed)

Our update system writes `FetchedTextSegment` directly; rich_text3d's own
systems are irrelevant to the translation contract. In `tests/translation.rs`:

```rust
#[cfg(all(feature = "rich_text3d", feature = "yaml"))]
#[test]
fn rich_text3d_segment_translates_on_locale_change() {
    use bevy_rich_text3d::FetchedTextSegment;

    let mut app = ready_app();
    let id = spawn_text(&mut app, I18nText3dSegment::new("hello"));

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(
        app.world().get::<FetchedTextSegment>(id).unwrap().as_str(),
        "Hello world"
    );

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(
        app.world().get::<FetchedTextSegment>(id).unwrap().as_str(),
        "こんにちは世界"
    );
}
```

(`bevy_rich_text3d` becomes visible to the test crate through the optional
dependency when the feature is on — no dev-dependency needed.)

**Verify**: `cargo test --all-features` → all pass including this test.

### Step 4: The example

Create `examples/rich_text_3d.rs` — a minimal 3D scene: one `Text3d` whose
single segment extracts from an `I18nText3dSegment("hello")` entity, plus a
locale-cycling key. Skeleton (adapt as the real API demands, keeping the
structure):

```rust
//! Translated 3D text via `bevy_rich_text3d` (feature `rich_text3d`).
//!
//! Run: cargo run --example rich_text_3d --features rich_text3d
//! Press SPACE to cycle through the available locales.

use bevy::prelude::*;
use bevy_rich_text3d::{
    Text3d, Text3dPlugin, Text3dStyling, TextAtlas,
};
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
```

Adaptation rules: if `Text3d::from_extract`, `Text3dStyling` field names, or
the material wiring differ from the real 0.7 API, follow the crate's own
`examples/` (fetch nothing — the compiler errors plus
`cargo doc -p bevy_rich_text3d` in the worktree are your reference). CJK
locales may render tofu if the system lacks fonts — acceptable for the
example; if `LoadFonts` accepts a directory cleanly, add
`assets/fonts/NotoSans` via
`.insert_resource(LoadFonts { font_directories: vec!["assets/fonts/NotoSans".into()], ..Default::default() })`
and note in the header comment that Noto ships with the repo. Do not add a
`--no-default-features` variant.

**Verify**: `cargo build --example rich_text_3d --features rich_text3d` →
exit 0. (Running it needs a window; skip and note if headless.)

### Step 5: README + CHANGELOG

1. README: rewrite `### Third-party text components (bevy_rich_text3d)` —
   lead with the feature:
   - `bevy_simple_i18n = { version = "...", features = ["rich_text3d"] }`
     gives `I18nText3dSegment` + the `I18nTarget` impl for
     `FetchedTextSegment`; show the segment-spawn snippet from the example
     and point at `examples/rich_text_3d.rs`.
   - Add one explicit paragraph: why the feature exists — the orphan rule
     prevents user crates from implementing our trait for their type; for
     OTHER third-party crates, implement `I18nTarget` for **your own**
     component types, or request an upstream/feature impl.
   - Keep the `I18nFont`-doesn't-apply note.
   - Add the `rich_text3d` row to the `## Cargo Features` table (Default: no).
2. CHANGELOG `## [Unreleased]` → `### Added`: the feature bullet (impl +
   component + example, off by default). Also REWORD the existing
   `I18nTarget` bullet's `FetchedTextSegment` mention to point at the
   feature (the trait alone cannot reach foreign types from user code).

**Verify**: `cargo fmt --check` → exit 0.

### Step 6: CI

In `.github/workflows/ci.yml`, `build_examples` job: change
`run: cargo build --examples` to `run: cargo build --examples --all-features`
(compiles the new example; the wasm job stays untouched).

**Verify**: YAML parse
(`python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`)
→ no error. Then the full gate suite (every command in the table) → all green.

## Test plan

- `rich_text3d_segment_translates_on_locale_change` (step 3) — proves the
  feature-gated impl + component through the standard pipeline.
- The example compiling under `--all-features` in CI is the API-drift canary
  against bevy_rich_text3d releases.

## Done criteria

- [ ] `cargo check --all-targets` (feature OFF) exits 0 — no accidental hard dependency
- [ ] `grep -c "impl I18nTarget for FetchedTextSegment" src/components/i18n_text_3d_segment.rs` → 1
- [ ] `cargo build --example rich_text_3d --features rich_text3d` exits 0
- [ ] `cargo test --all-features` exits 0; new test passing
- [ ] `cargo test --no-default-features` exits 0
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` exits 0
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `cargo check --target wasm32-unknown-unknown` (default features) exits 0
- [ ] `grep -c "impl I18nTarget for FetchedTextSegment" README.md` → 0 (broken recipe gone)
- [ ] `git status` shows only in-scope files modified

## STOP conditions

- `bevy_rich_text3d = "0.7"` fails to resolve or its bevy requirement
  conflicts with ours — report the exact cargo error.
- `FetchedTextSegment` is not a tuple struct with a public `.0: String`
  (the impl fails to compile) — report its actual shape and available
  setters (`set_if_changed`?) instead of improvising the impl.
- `#[require(FetchedTextSegment)]` fails (no `Default`?) — report.
- The example cannot compile after 3 fix iterations against the real API —
  commit the library parts (steps 1–3, 5, 6 minus the example entry) and
  report the example's compiler errors.
- Any existing test fails.

## Maintenance notes

- Version policy: bump the `bevy_rich_text3d` requirement in lockstep with
  bevy majors (their 0.7 = bevy 0.19). The `--all-features` examples build in
  CI is the drift alarm.
- Reviewer: check feature-off builds are byte-identical in behavior (no
  stray `use bevy_rich_text3d` outside `cfg`), and that the README no longer
  teaches the orphan-rule-violating impl anywhere.
