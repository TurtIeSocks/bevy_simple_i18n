# Plan 005: Add `I18nTextSpan` for rich text (translated spans inside a `Text`/`Text2d` block)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: this plan builds ON TOP of plan 004 (macro
> dedupe) — confirm `src/components/mod.rs` contains
> `define_i18n_text_component` before starting; if it does not, STOP (004 has
> not landed).

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/004-dedupe-text-components-macro.md (DONE required)
- **Category**: direction
- **Planned at**: commit `3870c9f`, 2026-07-19

## Why this matters

Bevy renders rich text (mixed fonts, sizes, colors inline) as child entities
carrying `TextSpan` under a `Text` or `Text2d` root. Today only whole blocks
can be translated: `I18nText` requires `Text`, so a span entity cannot be
driven from a translation key — users must fall back to manual re-translation
systems for any styled sentence fragment. Mixed-style text is a core i18n
need (colored keywords in tutorial text, bold player names inside sentences).
The `I18nComponent` trait already supports any string-carrying component;
`TextSpan` qualifies. After plan 004 this is one macro invocation plus
registration.

## Current state

- `src/components/mod.rs` — holds `define_i18n_text_component!` (from plan
  004) and the `I18nComponent` trait with
  `type Target: Component<Mutability = Mutable> + DerefMut<Target = String>`.
- `src/components/i18n_text.rs` / `i18n_text_2d.rs` — exemplar macro
  invocations (from plan 004); model the new file on them.
- `src/plugin.rs` — `.register_type::<I18nText>()` etc. around line 64, and
  `.register_i18n_component::<I18nText>()` / `::<I18nText2d>()` around line
  83. The new component must be added to both lists.
- Bevy 0.19 fact to verify in step 1: `bevy::prelude::TextSpan` is a
  component that derefs to `String` (it has been `TextSpan(pub String)` with
  `Deref`/`DerefMut` since Bevy 0.15). Also note: `TextSpan`'s own required
  components include `TextFont`, so `I18nFont` on the same entity works the
  same way it does for `Text`.
- Dynamic-font interplay (no code change needed): `update_translations` in
  `src/plugin.rs` already queries `Option<&mut TextFont>` + `Option<Ref<I18nFont>>`
  generically per registered component, so spans get dynamic fonts for free.
- `tests/translation.rs` — integration-test patterns: `test_app()`,
  `advance_until_ready`, `ready_app()`, `spawn_text`. Model the new test on
  `text2d_updates_when_the_global_locale_changes` (`tests/translation.rs:87-100`).

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile | `cargo check --all-targets` | exit 0 |
| Fact check | `cargo doc --no-deps` then inspect, or a scratch test | `TextSpan` derefs `String` |
| Tests | `cargo test --all-features` | all pass incl. new test |
| Minimal | `cargo test --no-default-features` | all pass |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | exit 0 |

## Scope

**In scope**:
- `src/components/i18n_text_span.rs` (create)
- `src/components/mod.rs` (module decl + re-export)
- `src/plugin.rs` (register_type + register_i18n_component)
- `tests/translation.rs` (one new test)
- `README.md` (short subsection under "Features", after "Text Translations")

**Out of scope**:
- The macro body itself — if `I18nTextSpan` needs macro changes, STOP
  (something about the span case breaks an assumption worth human review).
- Examples and the web demo.
- `CHANGELOG.md` — release notes are batched by the maintainer.

## Git workflow

- Current worktree branch; conventional commit, e.g.
  `feat: I18nTextSpan — translated rich-text spans`.
- Do NOT push.

## Steps

### Step 1: Verify the TextSpan fact

Confirm `bevy::prelude::TextSpan` satisfies
`Component<Mutability = Mutable> + DerefMut<Target = String>`: write the new
file's macro invocation (step 2) and let the compiler be the check — the
`I18nComponent` impl inside the macro constrains `Target` exactly so. If it
fails these bounds, STOP and report the exact error.

### Step 2: Create the component

`src/components/i18n_text_span.rs`, modeled on `i18n_text_2d.rs`:

```rust
use super::define_i18n_text_component;

define_i18n_text_component!(
    /// Component for translatable rich-text spans managed by `bevy_simple_i18n`.
    ///
    /// A Bevy [`TextSpan`](bevy::prelude::TextSpan) is inserted automatically (via
    /// `#[require(TextSpan)]`) and kept in sync with the translated value. Spawn it
    /// as a CHILD of an entity with [`Text`](bevy::prelude::Text) (or
    /// [`Text2d`](bevy::prelude::Text2d)) to compose one paragraph out of several
    /// independently styled, independently translated pieces:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_simple_i18n::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// commands.spawn(I18nText::new("greeting")).with_child((
    ///     I18nTextSpan::new("player_name_label").with_arg("name", "Alex"),
    ///     TextColor(Color::srgb(1.0, 0.8, 0.2)),
    /// ));
    /// # }
    /// ```
    I18nTextSpan, target: bevy::prelude::TextSpan
);
```

Wire it in `src/components/mod.rs` (`mod i18n_text_span;` +
`pub use i18n_text_span::*;`) and `src/plugin.rs`
(`.register_type::<I18nTextSpan>()` next to the other register_type calls,
`.register_i18n_component::<I18nTextSpan>()` next to the other registrations).

**Verify**: `cargo check --all-targets` → exit 0.

### Step 3: Integration test

In `tests/translation.rs` (near the Text2d test), gated `#[cfg(feature = "yaml")]`
like its neighbors:

```rust
#[cfg(feature = "yaml")]
#[test]
fn text_span_translates_and_updates_on_locale_change() {
    let mut app = ready_app();
    let root = app.world_mut().spawn(Text::default()).id();
    let span = app.world_mut().spawn(I18nTextSpan::new("hello")).id();
    app.world_mut().entity_mut(root).add_child(span);
    app.update();

    app.world_mut().resource_mut::<I18n>().set_locale("en");
    app.update();
    assert_eq!(app.world().get::<TextSpan>(span).unwrap().0, "Hello world");

    app.world_mut().resource_mut::<I18n>().set_locale("ja");
    app.update();
    assert_eq!(app.world().get::<TextSpan>(span).unwrap().0, "こんにちは世界");
}
```

(`TextSpan` needs importing in the test file: `bevy::prelude::*` already
covers it.)

**Verify**: `cargo test --all-features` → all pass including the new test.

### Step 4: README

Add a short "Rich text spans" subsection under Features (after "Text
Translations"), ~6 lines: what it is, the child-of-`Text` spawn pattern, one
code snippet (reuse the doc example). Match the README's existing terse tone.

**Verify**: `cargo test --all-features` (doc-tests still green) and
`cargo fmt --check` → exit 0.

## Test plan

- New: `text_span_translates_and_updates_on_locale_change` (step 3) — covers
  spawn-as-child, initial translation, and locale-switch re-render.
- The macro's builders (`with_arg`, `with_locale`, …) are already covered by
  the existing `I18nText` tests; no need to re-test per component.

## Done criteria

- [ ] `grep -c "I18nTextSpan" src/plugin.rs` → 2 (register_type + register_i18n_component)
- [ ] `cargo test --all-features` exits 0; new test present and passing
- [ ] `cargo test --no-default-features` exits 0
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` exits 0
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `git status` shows only in-scope files modified

## STOP conditions

- `TextSpan` fails the `Target` bounds (step 1) — report the compiler error.
- The macro needs modification to accommodate spans.
- The integration test renders an empty string: likely the span update runs
  before Bevy's text propagation — investigate one `app.update()` more, then
  STOP and report if it still fails (do not restructure the plugin's systems).

## Maintenance notes

- If plan 006 (mutators) lands after this, `I18nTextSpan` gains the setters
  automatically via the macro — no follow-up needed.
- Reviewer: check README snippet compiles as a doc-test (it is copied from
  the struct docs, which do).
