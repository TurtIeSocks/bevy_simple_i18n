# Plan 011: Closure writer registration — drive ANY foreign text component without a trait impl

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: confirm `grep -c "macro_rules! define_i18n_text_component" src/components/mod.rs` → 1
> and `grep -c "pub trait I18nComponentRegistration" src/plugin.rs` → 1. On a
> mismatch, STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (macro restructuring + new public API)
- **Depends on**: none (plan 010 runs first in this batch; this plan's files
  overlap with it only in tests/README/CHANGELOG appends)
- **Category**: direction
- **Planned at**: commit `d411a90`, 2026-07-19

## Why this matters

The orphan rule means only THIS crate can `impl I18nTarget` for foreign types
(we ship `rich_text3d`, and `fontmesh` in plan 010) — every other third-party
text crate is locked out unless we add a feature for it. Closure registration
sidesteps coherence entirely: behavior passed as a value needs no impl. A
user integrating any crate's `SomeLabel(String)` writes:

```rust
app.register_i18n_writer::<SomeLabel>(|label, text| label.0 = text);
commands.spawn((I18nKey::new("hello").with_arg("name", "X"), SomeLabel::default()));
```

and gets the full pipeline (interpolation, plurals, per-entity locale,
setters, warn-once) with zero coherence issues and zero new features in this
crate. `I18nKey` is needed because the existing driver components
(`I18nText` etc.) `#[require]` their own targets — `I18nText` would drag a
bevy_ui `Text` (and its `Node` requirement) onto a 3D entity.

## Current state

- `src/components/mod.rs`:
  - `define_i18n_text_component!` macro — one arm:
    `($(#[$struct_meta:meta])* $name:ident, target: $target:ty)` generating:
    the struct (fields `key`, `args`, `locale`, cfg-gated `count`) with
    `#[require($target)]` + `#[derive(Component, Default, Reflect, ...)]`,
    the `impl I18nComponent` block (`type Target = $target;` + `locale()` +
    `translate()`), and an `impl $name` block with the builder methods
    (`new`, `with_locale`, `with_arg`, `with_num_arg`, `with_count`) and the
    in-place setters (`set_key`, `set_locale`, `set_arg`, `set_num_arg`,
    `clear_args`, `set_count`). Read the macro in full before touching it.
  - The `translate()` body delegates to `crate::components::utils::translate_by_key`
    / `translate_plural` — free functions, so a driver without an
    `I18nComponent` impl can reuse them directly.
- `src/plugin.rs`:
  - `pub trait I18nComponentRegistration { fn register_i18n_component<T: I18nComponent>(&mut self) -> &mut Self; }`
    with the `impl for App` adding `update_translations::<T>` to `Update`.
  - `update_translations<T>` — the change-detection pattern to replicate:
    skips unless `i18n.is_changed() || key.is_changed() || font_changed`.
- `src/lib.rs` prelude re-exports `components::*` and `plugin::*`.
- Tests: `tests/translation.rs` with `ready_app()`/`spawn_text()` helpers and
  (from plan 008) the `MockSegment`/`MockLabel` pattern for foreign-shape
  mocks.

## Commands you will need

Standard gate suite (see plan 010's list; identical).

## Scope

**In scope**: `src/components/mod.rs`, `src/plugin.rs`,
`tests/translation.rs`, `README.md`, `CHANGELOG.md`.

**Out of scope**: `src/components/utils.rs` (the free functions already have
the right shape), `i18n_text*.rs` files, `Cargo.toml`, examples, CI.

## Steps

### Step 1: `I18nKey` — a target-less driver component

Restructure `define_i18n_text_component!` so the builder/setter surface is
shared, then add a target-less arm. Recommended shape — internal `@` rules:

```rust
macro_rules! define_i18n_text_component {
    // existing public arm: struct + #[require] + I18nComponent impl + shared body
    ($(#[$struct_meta:meta])* $name:ident, target: $target:ty) => { ... calls @struct/@builders ... };
    // NEW public arm: no target, no #[require], no I18nComponent impl;
    // instead generates INHERENT `locale()` + `translate()` with the same
    // bodies the trait impl uses (delegating to utils::translate_by_key /
    // translate_plural), so register_i18n_writer's system can call them.
    ($(#[$struct_meta:meta])* $name:ident, no_target) => { ... };
    (@struct ...) => { ... };
    (@builders ...) => { ... };
}
```

Any internal factoring that leaves BOTH arms' expansions byte-equivalent to
"what you'd write by hand" is fine — the requirements are: no duplicated
builder-method bodies across arms, existing three invocations unchanged, and
the new arm produces no `#[require]` and no `I18nComponent` impl. Then:

```rust
define_i18n_text_component!(
    /// Translation-key driver with no built-in render target (see
    /// [`register_i18n_writer`](crate::prelude::I18nComponentRegistration::register_i18n_writer)).
    ///
    /// Pair it with any third-party text component and a closure that writes
    /// the translated string — the escape hatch when an `I18nTarget` impl is
    /// impossible (orphan rule: foreign trait + foreign type).
    I18nKey, no_target
);
```

in a new `src/components/i18n_key.rs` (mod + pub use in mod.rs, NOT
cfg-gated), and `register_type::<I18nKey>()` in `plugin.rs`.

**Verify**: `cargo check --all-targets --all-features` → exit 0;
`cargo test --all-features` → baseline all green (macro refactor is
behavior-preserving for the three existing components).

### Step 2: `register_i18n_writer`

Add to the `I18nComponentRegistration` trait and its `App` impl:

```rust
/// Registers a closure that writes translated text (driven by an [`I18nKey`]
/// on the same entity) into `Target` — for third-party components this crate
/// has no [`I18nTarget`](crate::prelude::I18nTarget) impl for. The closure
/// runs whenever the locale, translation table, or the entity's `I18nKey`
/// changes.
fn register_i18n_writer<Target, F>(&mut self, writer: F) -> &mut Self
where
    Target: Component<Mutability = Mutable>,
    F: Fn(&mut Target, String) + Send + Sync + 'static;
```

`App` impl: add a move-closure system to `Update` mirroring
`update_translations`' change detection (no font handling — `I18nFont`
targets `TextFont`, meaningless for unknown targets):

```rust
self.add_systems(
    Update,
    move |i18n: Res<I18n>, mut query: Query<(&mut Target, Ref<I18nKey>)>| {
        let i18n_changed = i18n.is_changed();
        for (mut target, key) in query.iter_mut() {
            if !i18n_changed && !key.is_changed() {
                continue;
            }
            writer(&mut target, key.translate(&i18n));
        }
    },
)
```

(`Mutable` import: match how `plugin.rs`/`mod.rs` already name it —
`bevy::ecs::component::Mutable`.)

**Verify**: `cargo check --all-targets --all-features` → exit 0.

### Step 3: Tests

In `tests/translation.rs`:

1. `closure_writer_drives_foreign_component` (gate `#[cfg(feature = "yaml")]`):
   `#[derive(Component, Default)] struct ForeignLabel(String);` — NO
   `I18nTarget` impl, NO `Deref`. Register
   `app.register_i18n_writer::<ForeignLabel>(|l, text| l.0 = text)` on
   `test_app()` before `advance_until_ready`; spawn
   `(I18nKey::new("hello"), ForeignLabel::default())`; assert `"Hello world"`
   after `set_locale("en")` and `"こんにちは世界"` after `set_locale("ja")`.
2. `closure_writer_interpolates_args` (gate
   `#[cfg(all(feature = "numbers", feature = "yaml"))]`): same setup with
   `I18nKey::new("messages.cats").with_num_arg("count", 2000.3).with_locale("en")`
   → assert `"You have 2,000.3 cats"` — proves the full pipeline (not just
   bare lookup) flows through the closure path.

**Verify**: `cargo test --all-features` → baseline + 2 new, all pass;
`cargo test --no-default-features` → all pass (test 1 is yaml-gated; if the
no-default build trips dead-code warnings on the mocks, gate their
definitions like the tests are).

### Step 4: README + CHANGELOG

README: extend the "Third-party text components" section with an
"Any other crate: closure writers" sub-paragraph — the two-line example from
"Why this matters", plus one sentence tying it to the orphan-rule paragraph
already there (this is the no-impl-needed escape hatch). CHANGELOG
`[Unreleased]` → `### Added`: `I18nKey` + `register_i18n_writer` bullet.

**Verify**: full gate suite → all green.

## Done criteria

- [ ] `grep -c "no_target" src/components/mod.rs` ≥ 2 (macro arm + docs) and `src/components/i18n_key.rs` exists
- [ ] `grep -c "fn register_i18n_writer" src/plugin.rs` → 2 (trait decl + impl)
- [ ] `cargo test --all-features` exits 0 with both new tests passing
- [ ] `cargo test --no-default-features` exits 0
- [ ] clippy/-D warnings, doc/-D warnings (--all-features), fmt, wasm check all exit 0
- [ ] Existing three `define_i18n_text_component!` invocations textually unchanged
- [ ] `git status` shows only in-scope files

## STOP conditions

- The macro refactor cannot keep both arms free of duplicated builder bodies
  without breaking one of the three existing invocations — report the
  conflict instead of forking the builder implementations.
- The move-closure system fails to satisfy `IntoSystem` (compiler error about
  the closure's parameters) — report the exact error; do not restructure into
  a resource-stored closure without review.
- Change detection through the closure path fails (test 1's second assert) —
  report; the `Ref<I18nKey>`/`is_changed` pattern should behave identically
  to `update_translations`.

## Maintenance notes

- `I18nKey` is deliberately render-agnostic: no `#[require]`, no font
  support. If someone asks for `I18nFont` with closure writers, that's a new
  design conversation, not a patch.
- Future: `register_i18n_writer` with a custom driver type (generic `D`
  instead of hardcoded `I18nKey`) is a backward-compatible extension if
  demanded — don't build it now.
- Reviewer: check the macro's existing three expansions didn't drift (the
  plan requires invocations unchanged; `cargo expand` or test-suite green is
  the referee).
