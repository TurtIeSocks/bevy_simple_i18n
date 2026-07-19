# Plan 008: Generalize `I18nComponent::Target` to an `I18nTarget` trait (unlock third-party text components)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 6558372..HEAD -- src/components/mod.rs src/plugin.rs tests/translation.rs README.md CHANGELOG.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (public trait-bound change; every registered component flows through it)
- **Depends on**: none (004–007 already merged to main)
- **Category**: direction
- **Planned at**: commit `6558372`, 2026-07-19

## Why this matters

`I18nComponent::Target` requires `Component<Mutability = Mutable> +
DerefMut<Target = String>`. That fits Bevy's `Text`/`Text2d`/`TextSpan`, but
locks out every text component that holds a `String` without derefing to it —
concretely `bevy_rich_text3d`'s `FetchedTextSegment(String)` (its designed
dynamic-text hook, which exposes `as_str`/`set_if_changed` but no `DerefMut`).
Users gluing the crates today must bypass our pipeline entirely and lose
interpolation, plurals and warn-once (all `pub(crate)`). Replacing the
`DerefMut` bound with a one-method `I18nTarget` trait lets any crate's text
component join the full pipeline in ~5 user-side lines, without this crate
taking any new dependency. This is a breaking change for hand-written
`I18nComponent` impls (0.5.0 material) with a 3-line migration.

## Current state

- `src/components/mod.rs`:
  - top-of-file imports include `use std::ops::DerefMut;`
  - the trait (lines ~21-44):
    ```rust
    pub trait I18nComponent: Component {
        /// The Bevy text component this writes its translated value into.
        ///
        /// It must dereference to a [`String`] — as Bevy's [`Text`](bevy::prelude::Text) and
        /// [`Text2d`](bevy::prelude::Text2d) both do — and is inserted automatically via the
        /// `#[require(..)]` attribute on the implementing component.
        type Target: Component<Mutability = Mutable> + DerefMut<Target = String>;
        ...
    }
    ```
  - also holds `InterpolationType` and the `define_i18n_text_component!`
    macro (the macro references `$target` only as a type — it needs no change).
- `src/plugin.rs`, in `update_translations` (~line 130):
  ```rust
  **target = key.translate(&i18n);
  ```
  where `target: Mut<T::Target>` from the query. This deref-assign is the ONLY
  place the `DerefMut` bound is actually used.
- Bevy 0.19 facts: `Text`, `Text2d`, and `TextSpan` are all tuple structs
  around `String` (`pub struct Text(pub String);` etc.), so explicit
  `set_text` impls can write `self.0 = text;`.
- `tests/translation.rs` — integration-test helpers `test_app()`,
  `ready_app()`, `spawn_text()`, `text()`; README-style custom-component
  exemplar lives in README's "Traits" section.
- `README.md` — has a `## Traits` section documenting `I18nComponent` (its
  example signature must be updated) and a `## Features` section.
- `CHANGELOG.md` — has an `## [Unreleased]` section with `### Added` and
  `### Changed` subsections; append there.
- Baseline at `6558372` (verified green in PR #12 CI + locally):
  `cargo test --all-features` = 39 unit + 29 integration + 7 doc;
  `cargo test --no-default-features` = 27 + 4 + 6; clippy `-D warnings`,
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`, `cargo fmt --check`,
  wasm32 check — all clean.

Design decision already made (do not revisit): NO blanket
`impl<T: DerefMut<Target=String> + ...> I18nTarget for T` — a blanket plus
future foreign impls is an E0119 coherence trap for downstream crates. Three
explicit impls instead.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile | `cargo check --all-targets` | exit 0 |
| Tests | `cargo test --all-features` | all pass (baseline counts + 1 new integration test) |
| Minimal | `cargo test --no-default-features` | all pass |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | exit 0 |
| Format | `cargo fmt` then `cargo fmt --check` | exit 0 |
| Wasm | `cargo check --target wasm32-unknown-unknown` | exit 0 |

## Scope

**In scope**:
- `src/components/mod.rs` (new trait + 3 impls + bound change + doc updates)
- `src/plugin.rs` (`set_text` call site)
- `tests/translation.rs` (mock third-party-target test)
- `README.md` (Traits section update + compat recipe)
- `CHANGELOG.md` (Unreleased entries)

**Out of scope**:
- Adding `bevy_rich_text3d` as a dependency (optional or otherwise) — the
  recipe is documentation, deliberately not compiled against the real crate.
- `src/components/utils.rs`, `src/plural.rs`, `src/resources.rs`, examples,
  `web/`, workflows.
- The crate version in `Cargo.toml` — release numbering is the maintainer's.

## Git workflow

- Current worktree branch; conventional commits, e.g.
  `feat!: generalize I18nComponent::Target to the I18nTarget trait` then
  `docs: bevy_rich_text3d compatibility recipe`.
- Do NOT push.

## Steps

### Step 1: The trait and its impls

In `src/components/mod.rs`, above the `I18nComponent` trait:

```rust
/// A text component `bevy_simple_i18n` can write translated strings into.
///
/// Implemented for Bevy's [`Text`](bevy::prelude::Text),
/// [`Text2d`](bevy::prelude::Text2d) and [`TextSpan`](bevy::prelude::TextSpan).
/// Implement it for any third-party component that carries a `String` (e.g.
/// `bevy_rich_text3d`'s `FetchedTextSegment`) to drive it from a translation
/// key with the full pipeline — interpolation, plurals, per-entity locales:
///
/// ```rust,ignore
/// impl I18nTarget for FetchedTextSegment {
///     fn set_text(&mut self, text: String) {
///         self.0 = text;
///     }
/// }
/// ```
///
/// Deliberately NOT blanket-implemented over `DerefMut<Target = String>`:
/// a blanket impl would make downstream `impl I18nTarget for TheirType`
/// a coherence error (E0119) the moment `TheirType` could ever deref to
/// `String`. Explicit impls keep the trait open for the ecosystem.
pub trait I18nTarget: Component<Mutability = Mutable> {
    /// Replaces the component's text with the translated value.
    fn set_text(&mut self, text: String);
}

impl I18nTarget for bevy::prelude::Text {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

impl I18nTarget for bevy::prelude::Text2d {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

impl I18nTarget for bevy::prelude::TextSpan {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}
```

Then change the associated-type bound and its doc:

```rust
    /// The text component this writes its translated value into — anything
    /// implementing [`I18nTarget`]. It is inserted automatically via the
    /// `#[require(..)]` attribute on the implementing component.
    type Target: I18nTarget;
```

Remove `use std::ops::DerefMut;` (and the `Mutable` import if it is now only
used inside the `I18nTarget` bound — keep whatever the compiler needs).

**Verify**: `cargo check --all-targets` → the ONLY errors remaining should be
in `src/plugin.rs` (the deref-assign); if errors appear elsewhere, STOP.

### Step 2: The write site

In `src/plugin.rs` `update_translations`, replace:

```rust
**target = key.translate(&i18n);
```

with:

```rust
target.set_text(key.translate(&i18n));
```

(`Mut<T::Target>` auto-derefs for the method call and flags change detection
exactly as the deref-assign did.) If `I18nTarget` needs importing there, add
it to the existing `crate::` import list.

**Verify**: `cargo check --all-targets` → exit 0;
`cargo test --all-features` → baseline counts, all pass (behavior unchanged
for the built-in targets).

### Step 3: Prove a non-DerefMut target works

In `tests/translation.rs`, add a mock third-party component (deliberately NO
`Deref`/`DerefMut`) and a driving component, modeled on the file's existing
patterns:

```rust
/// Mock of a third-party text component (e.g. bevy_rich_text3d's
/// `FetchedTextSegment`): holds a String but does NOT deref to it.
#[derive(Component, Default)]
struct MockSegment(String);

impl I18nTarget for MockSegment {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

#[derive(Component)]
#[require(MockSegment)]
struct MockLabel(String);

impl I18nComponent for MockLabel {
    type Target = MockSegment;
    fn locale<'a>(&'a self, i18n: &'a I18n) -> &'a str {
        i18n.current()
    }
    fn translate(&self, i18n: &I18n) -> String {
        i18n.translate(self.locale(i18n), &self.0)
            .unwrap_or(&self.0)
            .to_string()
    }
}

#[cfg(feature = "yaml")]
#[test]
fn third_party_target_without_derefmut_translates() {
    let mut app = test_app();
    app.register_i18n_component::<MockLabel>();
    advance_until_ready(&mut app);
    let id = app.world_mut().spawn(MockLabel("hello".into())).id();
    app.update();

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(app.world().get::<MockSegment>(id).unwrap().0, "Hello world");

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(
        app.world().get::<MockSegment>(id).unwrap().0,
        "こんにちは世界"
    );
}
```

Note: `register_i18n_component` must be called before `advance_until_ready`
pumping, and `I18nTarget` / `I18nComponentRegistration` come via
`bevy_simple_i18n::prelude::*` which the file already imports. If the
`#[require(MockSegment)]` attribute needs `MockSegment: Default`, the derive
above provides it.

**Verify**: `cargo test --all-features` → all pass including the new test.

### Step 4: README

1. In `## Traits` → the `I18nComponent` example: update the wording that says
   the target "must dereference to a String" to "must implement
   [`I18nTarget`]" and mention the three built-in impls.
2. Add a subsection after it, `### Third-party text components
   (bevy_rich_text3d)`, ~15 lines: state that any `String`-carrying component
   can join via `I18nTarget`, then a `rust,ignore` fenced block showing
   `impl I18nTarget for FetchedTextSegment` plus a minimal driving component,
   and one sentence that `I18nFont` does not apply to `Text3d` (it styles via
   `Text3dStyling`, not `TextFont`).

**Verify**: `cargo test --all-features` → doc-tests still green;
`cargo fmt --check` → exit 0.

### Step 5: CHANGELOG

Append to `## [Unreleased]`:

- Under `### Added`: `I18nTarget` trait — implement it to drive any
  third-party `String`-carrying text component (e.g. `bevy_rich_text3d`'s
  `FetchedTextSegment`) with the full i18n pipeline.
- New `### Breaking changes` subsection (mirroring the 0.4.0 section's
  style): `I18nComponent::Target`'s bound changed from
  `Component<Mutability = Mutable> + DerefMut<Target = String>` to
  `I18nTarget`. Migration: implement `I18nTarget` for your target type
  (3 lines: `fn set_text(&mut self, text: String) { *self = text; }` — or
  `self.0 = text` for tuple structs); Bevy's `Text`/`Text2d`/`TextSpan` are
  provided.

**Verify**: full gate suite — all commands from the table, all green.

## Test plan

- New: `third_party_target_without_derefmut_translates` (step 3) — the
  load-bearing proof that a non-`DerefMut` target compiles against the new
  bound and re-renders on locale change through the standard system.
- Existing suite is the no-regression referee for the built-in targets.

## Done criteria

- [ ] `grep -c "DerefMut" src/components/mod.rs src/plugin.rs` → 0 in both
- [ ] `grep -c "pub trait I18nTarget" src/components/mod.rs` → 1
- [ ] `grep -c "impl I18nTarget" src/components/mod.rs` → 3
- [ ] `cargo test --all-features` exits 0; new test present and passing
- [ ] `cargo test --no-default-features` exits 0
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` exits 0
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `cargo check --target wasm32-unknown-unknown` exits 0
- [ ] `git status` shows only in-scope files modified

## STOP conditions

- `Text`/`Text2d`/`TextSpan` turn out not to be plain tuple structs over
  `String` in the pinned bevy version (`self.0 = text` fails to compile) —
  report the actual shape.
- The `Mut<T::Target>` method-call in step 2 does not trigger change
  detection (existing `updates_when_the_global_locale_changes` test fails) —
  report; do not restructure the system.
- The mock test cannot register without touching `src/plugin.rs` beyond the
  one-line write-site change.
- Any existing test fails and the fix would require editing that test.

## Maintenance notes

- Release note: this is the 0.5.0 breaking change; ship it together with the
  already-unreleased additive features.
- Future: if demand materializes, a feature-gated
  `impl I18nTarget for FetchedTextSegment` + `I18nText3dSegment` component
  can layer on top without further design work — the trait is the contract.
- Reviewer: confirm the three built-in impls write `self.0` (not `**self`) so
  no `DerefMut` sneaks back in as a hidden requirement.
