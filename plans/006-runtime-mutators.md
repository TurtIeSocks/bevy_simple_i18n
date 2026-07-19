# Plan 006: Runtime mutators — update keys, args, counts and numbers in place

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: this plan modifies the macro from plan 004 —
> confirm `src/components/mod.rs` contains `define_i18n_text_component`
> before starting; if it does not, STOP (004 has not landed).

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW-MED (new public API; existing behavior untouched)
- **Depends on**: plans/004-dedupe-text-components-macro.md (DONE required)
- **Category**: direction
- **Planned at**: commit `3870c9f`, 2026-07-19

## Why this matters

The components are builder-only: every field is private and set at spawn. A
score counter — the single most common game text — must re-insert a whole new
`I18nText` (`commands.entity(e).insert(I18nText::new("score").with_num_arg("points", p))`)
on every change, re-allocating the key and args each time and reading as a
workaround. `&mut` setters let users write
`text.set_num_arg("points", p)` through a normal query; Bevy's change
detection (`Ref<T>` in `update_translations`) already picks up any `DerefMut`
on the component, so re-rendering is automatic with zero plugin changes.

## Current state

- `src/components/mod.rs` — `define_i18n_text_component!` (plan 004)
  generates `I18nText`, `I18nText2d` (and `I18nTextSpan` if plan 005 landed —
  it gains these setters automatically). Fields: `key: String`,
  `args: Vec<(String, InterpolationType)>`, `locale: Option<String>`,
  cfg-`plurals` `count: Option<Decimal>`. `InterpolationType` lives in
  `mod.rs` (plan 004 moved it).
- The builder bodies to mirror (validation/skip semantics from plan 002):
  `with_locale` normalizes `_`→`-`, parses as `icu_locale_core::Locale`, logs
  `bevy::log::error!` and leaves the field unchanged on failure;
  `with_num_arg` / `with_count` convert via
  `super::utils::try_f64_to_fd` (non-finite → error log, no-op).
- `src/components/i18n_number.rs` — hand-written; fields
  `fixed_decimal: Option<Decimal>`, `locale: Option<String>`.
- `src/plugin.rs` `update_translations` — re-translates when `key.is_changed()`
  (a `Ref<T>`), which any `&mut` access through a query triggers. No plugin
  change needed or allowed.
- Existing test style for mutation-through-world:
  `modified_translation_assets_retranslate_live_text`
  (`tests/translation.rs`) mutates a resource and pumps `app.update()`.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile | `cargo check --all-targets` | exit 0 |
| Tests | `cargo test --all-features` | all pass incl. new tests |
| Minimal | `cargo test --no-default-features` | all pass |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` | exit 0 |
| Format | `cargo fmt` then `cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src/components/mod.rs` (setter methods inside the macro body)
- `src/components/i18n_number.rs` (hand-written setters)
- `tests/translation.rs` (new tests)
- `README.md` (short "Updating text at runtime" subsection under Features)

**Out of scope**:
- `src/plugin.rs` — change detection already covers this; if you think it
  needs a change, STOP.
- `src/components/utils.rs`, `src/plural.rs`, existing builder methods.
- No getters in this plan (YAGNI until someone asks); no `Default`-semantics
  changes.

## Git workflow

- Current worktree branch; conventional commit, e.g.
  `feat: in-place setters for i18n components`.
- Do NOT push.

## Steps

### Step 1: Setters in the macro

Add to the `impl $name` block inside `define_i18n_text_component!` (doc
comments included, written once):

- `pub fn set_key(&mut self, key: impl Into<String>)` — replaces the key.
- `pub fn set_locale(&mut self, locale: impl Into<String>)` — same
  normalize/validate/log semantics as `with_locale`, applied in place. Factor
  the shared logic into a small `pub(crate) fn parse_locale(raw: String) -> Option<String>`
  helper in `src/components/utils.rs`? NO — utils is out of scope; instead
  have `with_locale` delegate to `set_locale`:
  ```rust
  pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
      self.set_locale(locale);
      self
  }
  ```
  (Behavior identical; dedupes the validation into one place.)
- `pub fn set_arg(&mut self, key: impl Into<String>, value: impl ToString)` —
  UPSERT: if an arg with the same name exists, replace its value in place
  (preserving position, since interpolation is first-match-wins); otherwise
  push. Builder `with_arg` keeps its current push-only body (documented
  duplicate-args behavior is part of rust-i18n parity).
- cfg `numbers`: `pub fn set_num_arg(&mut self, key: impl Into<String>, value: impl Into<f64>)`
  — upsert like `set_arg`; non-finite value → error log, arg left unchanged
  (consistent with `with_num_arg`'s skip).
- `pub fn clear_args(&mut self)`.
- cfg `plurals`: `pub fn set_count(&mut self, count: impl Into<f64>)` — same
  `try_f64_to_fd` conversion; non-finite → error log, count left unchanged
  (NOT reset to `None` — an invalid update should not destroy valid state).

**Verify**: `cargo check --all-targets` → exit 0.

### Step 2: Setters on `I18nNumber`

- `pub fn set_number(&mut self, number: impl Into<f64>)` — via
  `utils::try_f64_to_fd`; non-finite → error log, value left unchanged.
- `pub fn set_locale(&mut self, locale: impl Into<String>)` + `with_locale`
  delegating to it, exactly as in step 1.

**Verify**: `cargo check --all-targets` → exit 0.

### Step 3: Tests

In `tests/translation.rs`:

1. `set_num_arg_retranslates_live_text` (`#[cfg(all(feature = "numbers", feature = "yaml"))]`):
   spawn `I18nText::new("messages.cats").with_num_arg("count", 1).with_locale("en")`,
   assert `"You have 1 cats"`; then
   `app.world_mut().get_mut::<I18nText>(id).unwrap().set_num_arg("count", 2000.3)`,
   `app.update()`, assert `"You have 2,000.3 cats"` (upsert replaced, change
   detection re-rendered, number localized).
2. `set_key_retranslates` (`#[cfg(feature = "yaml")]`): spawn
   `I18nText::new("hello").with_locale("en")` → `"Hello world"`; `set_key("text2d")`,
   update, assert `"Hello World (Text2d)"` (en.json ships it).
3. `set_count_retranslates` (`#[cfg(feature = "plurals")]`): spawn
   `I18nText::new("cats").with_count(1).with_locale("en")` → `"You have 1 cat"`;
   `set_count(3)`, update, assert `"You have 3 cats"`.
4. `set_number_reformats` (`#[cfg(feature = "numbers")]`): spawn
   `I18nNumber::new(1.5).with_locale("de")`, assert `"1,5"`; `set_number(24501.2)`,
   update, assert `"24.501,2"`.
5. `invalid_set_locale_keeps_previous` (`#[cfg(feature = "yaml")]`): spawn with
   `.with_locale("ja")` → `"こんにちは世界"`; `set_locale("not a locale!")`,
   update, assert still `"こんにちは世界"` (invalid update is a logged no-op).

**Verify**: `cargo test --all-features` → all pass;
`cargo test --no-default-features` → all pass.

### Step 4: README

"Updating text at runtime" subsection under Features: 4–6 lines + one snippet
(the score-counter case: query `&mut I18nText`, call `set_num_arg`). Match
existing README tone.

**Verify**: `cargo fmt --check` → exit 0.

## Test plan

The five tests in step 3 — each exercises a distinct setter through the full
app loop, proving both the mutation semantics and that change detection
re-renders without any plugin change.

## Done criteria

- [ ] `grep -c "pub fn set_" src/components/mod.rs` ≥ 5 (macro setters)
- [ ] `grep -c "pub fn set_" src/components/i18n_number.rs` = 2
- [ ] `cargo test --all-features` exits 0; 5 new tests present and passing
- [ ] `cargo test --no-default-features` exits 0
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` exits 0
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `git status` shows only in-scope files modified

## STOP conditions

- Change detection does NOT re-render after a setter (test 1 fails on the
  second assert): something is off about `Ref<T>` change semantics —
  investigate the test itself once, then STOP; do not modify `plugin.rs`.
- You find yourself wanting getters, `pub` fields, or a builder rewrite —
  out of scope, note it and move on.
- Existing tests fail.

## Maintenance notes

- `with_locale`-delegates-to-`set_locale` means locale validation lives once;
  future validation changes (e.g. accepting POSIX suffixes like `en_US.UTF-8`)
  happen in one method per component.
- Reviewer: check upsert preserves arg ORDER (first-match-wins interpolation
  makes order observable when users add duplicate names via `with_arg`).
