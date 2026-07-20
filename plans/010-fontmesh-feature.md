# Plan 010: `fontmesh` feature — in-crate `I18nTarget` impl for `bevy_fontmesh::TextMesh` + example

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: confirm `grep -c "rich_text3d" Cargo.toml` ≥ 2
> and `src/components/i18n_text_3d_segment.rs` exists — this plan mirrors that
> landed feature. On a mismatch, STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (additive, feature-gated, off by default)
- **Depends on**: plan 009 (merged — this mirrors it)
- **Category**: direction
- **Planned at**: commit `d411a90`, 2026-07-19

## Why this matters

Ecosystem survey (2026-07-19) found `bevy_fontmesh` is the one other alive,
bevy-0.19 text crate whose component shape fits `I18nTarget` cleanly:
`TextMesh` is a mutable `Component` with `pub text: String`, derives
`Default`, and its mesh regenerates on `Changed<TextMesh>` (its
`src/system.rs:361` filters `Or<(Changed<TextMesh>, Without<TextMeshComputed>)>`).
Like `bevy_rich_text3d`'s `FetchedTextSegment`, only THIS crate (trait owner)
can ship the impl for that foreign type — orphan rule. Feature-gated, off by
default, exactly like `rich_text3d`.

## Current state (the pattern to mirror)

- `Cargo.toml` — `rich_text3d = ["dep:bevy_rich_text3d"]` feature +
  optional dep + `[[example]] name = "rich_text_3d" ... required-features`.
- `src/components/i18n_text_3d_segment.rs` — the 24-line exemplar: orphan-rule
  comment, `impl I18nTarget for FetchedTextSegment`, then
  `define_i18n_text_component!(...)`. **Read it first; copy its structure.**
- `src/components/mod.rs` — cfg-gated `mod` + `pub use` pairs.
- `src/plugin.rs` — cfg-gated `register_type` + `register_i18n_component`
  block (mirrors the `numbers` pattern).
- `tests/translation.rs` — `rich_text3d_segment_translates_on_locale_change`
  as the test exemplar; helpers `ready_app()`, `spawn_text()`.
- `examples/rich_text_3d.rs` — example exemplar (SPACE cycles locales).
- `README.md` — "Third-party text components" section + Cargo Features table.
- `CHANGELOG.md` — `## [Unreleased]` → `### Added` bullets.
- `.github/workflows/ci.yml` — `build_examples` already runs
  `cargo build --examples --all-features`; no CI change needed this time.
- `bevy_fontmesh` 0.6.0 facts (verified from its published source):
  `#[derive(Component, Reflect, Clone, Debug, Default)] #[reflect(Component)]
  #[require(Mesh3d)] pub struct TextMesh { pub text: String, pub font:
  Handle<Font>, pub style: TextMeshStyle }`. Font assets exist in this repo at
  `assets/fonts/NotoSans/` (`ja.ttf` covers Latin + kana + CJK — use it in the
  example so every shipped locale renders).

## Commands you will need

Same gate suite as always: `cargo check --all-targets --all-features`,
`cargo check --all-targets` (feature off), `cargo test --all-features`,
`cargo test --no-default-features`, `cargo clippy --all-targets
--all-features -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc
--no-deps --all-features`, `cargo fmt --check`,
`cargo check --target wasm32-unknown-unknown`,
`cargo build --example font_mesh --features fontmesh`.

## Scope

**In scope**: `Cargo.toml`, `Cargo.lock` (mechanical dep addition),
`src/components/i18n_text_mesh.rs` (new), `src/components/mod.rs`,
`src/plugin.rs`, `tests/translation.rs`, `examples/font_mesh.rs` (new),
`README.md`, `CHANGELOG.md`.

**Out of scope**: `.github/workflows/` (already covers `--all-features`
examples), `web/`, everything else. Do not touch the `rich_text3d` files.

## Steps

### Step 1: Cargo wiring

`bevy_fontmesh = { version = "0.6", optional = true }` dependency;
`fontmesh = ["dep:bevy_fontmesh"]` feature (comment mirroring the
rich_text3d one: off by default, pins a third-party release cadence);
`[[example]] name = "font_mesh", path = "examples/font_mesh.rs",
required-features = ["fontmesh"]`.

**Verify**: `cargo check --all-targets --all-features` → exit 0 (resolves the
new dep); `cargo check --all-targets` → exit 0.

### Step 2: `src/components/i18n_text_mesh.rs`

Mirror `i18n_text_3d_segment.rs` exactly (orphan-rule comment included):

```rust
impl I18nTarget for bevy_fontmesh::TextMesh {
    fn set_text(&mut self, text: String) {
        self.text = text;
    }
}

define_i18n_text_component!(
    /// Component for translatable `bevy_fontmesh` 3D mesh text (feature `fontmesh`).
    ///
    /// A [`TextMesh`](bevy_fontmesh::TextMesh) is inserted automatically (via
    /// `#[require(..)]`) and its `text` field is kept in sync — the mesh
    /// regenerates on change. Set the font by inserting your own `TextMesh`
    /// alongside (the `#[require]` only fills in a default when missing).
    I18nTextMesh, target: bevy_fontmesh::TextMesh
);
```

Wire cfg-gated `mod`/`pub use` in `src/components/mod.rs` and the cfg-gated
`register_type::<I18nTextMesh>` + `register_i18n_component::<I18nTextMesh>`
block in `src/plugin.rs`, mirroring `rich_text3d` exactly.

**Verify**: `cargo check --all-targets --all-features` → exit 0.

### Step 3: Test

In `tests/translation.rs`, mirror the rich_text3d test:
`#[cfg(all(feature = "fontmesh", feature = "yaml"))]` —
spawn `I18nTextMesh::new("hello")`, assert
`app.world().get::<bevy_fontmesh::TextMesh>(id).unwrap().text` is
`"Hello world"` after `set_locale("en")` and `"こんにちは世界"` after
`set_locale("ja")` (using `ready_app()` + updates).

**Verify**: `cargo test --all-features` → baseline + 1 new test, all pass.

### Step 4: Example `examples/font_mesh.rs`

Mirror `examples/rich_text_3d.rs` (doc header with run command, SPACE cycles
locales via the same `cycle_locale` system). Spawn:

```rust
commands.spawn((
    I18nTextMesh::new("hello"),
    bevy_fontmesh::TextMesh {
        font: asset_server.load("fonts/NotoSans/ja.ttf"), // Latin+kana+CJK: every shipped locale renders
        ..Default::default()
    },
    // + material/transform per bevy_fontmesh's own README example
));
```

Consult bevy_fontmesh 0.6 docs/README (docs.rs or github) for the exact
plugin (`FontMeshPlugin`?) and material requirements, and adapt the spawn to
its canonical example — the I18n parts above are the fixed contract, the
rendering scaffold follows the upstream example.

**Verify**: `cargo build --example font_mesh --features fontmesh` → exit 0.

### Step 5: README + CHANGELOG

README: in "Third-party text components", add a short `bevy_fontmesh`
paragraph + snippet next to the rich_text3d one; add `fontmesh` row to the
Cargo Features table. CHANGELOG `[Unreleased]` → `### Added`: `fontmesh`
feature bullet mirroring the `rich_text3d` bullet.

**Verify**: full gate suite (all commands above) → all green.

## Done criteria

- [ ] `grep -c "impl I18nTarget for bevy_fontmesh::TextMesh" src/components/i18n_text_mesh.rs` → 1
- [ ] `cargo test --all-features` exits 0 with the new test passing
- [ ] `cargo test --no-default-features` exits 0 (unchanged)
- [ ] `cargo build --example font_mesh --features fontmesh` exits 0
- [ ] clippy/-D warnings, doc/-D warnings (--all-features), fmt, wasm check (default features) all exit 0
- [ ] `git status` shows only in-scope files

## STOP conditions

- `bevy_fontmesh` 0.6 fails to resolve against this crate's bevy 0.19 tree
  (version conflict in `cargo check`) — report the resolver error.
- `TextMesh` turns out not to be `pub text: String` + `Default` (API drift
  from the surveyed 0.6.0) — report the actual shape.
- The example needs more than ~20 lines of rendering scaffold beyond the
  upstream README example — report rather than inventing a rendering setup.

## Maintenance notes

- Same third-party-cadence liability as `rich_text3d`: when bevy_fontmesh
  releases for bevy 0.20, the optional dep needs a bump. Young crate, solo
  maintainer — if it dies, the feature can be dropped in a minor release
  (off by default, so blast radius is opt-in users only).
- `TextMeshGlyphs` (per-glyph child meshes) has the identical shape — add on
  request, one macro invocation.
