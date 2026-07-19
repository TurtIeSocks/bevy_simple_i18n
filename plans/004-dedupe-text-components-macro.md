# Plan 004: Deduplicate `I18nText` / `I18nText2d` behind one macro (behavior-preserving)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 3870c9f..HEAD -- src/components/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (refactor of the two flagship components; zero behavior change allowed — the full test suite is the referee)
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit `3870c9f`, 2026-07-19

## Why this matters

`src/components/i18n_text.rs` and `src/components/i18n_text_2d.rs` are
~130-line near-identical twins: same fields, same `I18nComponent` impl shape,
same five builder methods — differing only in the component name, the
`#[require(..)]`ed target (`Text` vs `Text2d`), and doc-example text. Plans
005 (a third component, `I18nTextSpan`) and 006 (six new mutator methods)
would each multiply that duplication; the rule of three has arrived. One
`macro_rules!` definition makes 005 a one-invocation feature and 006 a
write-once feature, and removes the standing risk of the twins drifting
(the pre-0.3 `I18nText2d` update bug was exactly such a drift).

## Current state

- `src/components/i18n_text.rs` — `I18nText`: struct with `key: String`,
  `args: Vec<(String, InterpolationType)>`, `pub(crate) locale: Option<String>`,
  and (under `#[cfg(feature = "plurals")]`, `#[reflect(ignore)]`)
  `count: Option<Decimal>`. Derives
  `Component, Default, Reflect, Debug, Clone`, attributes
  `#[reflect(Component)]` and `#[require(Text)]`. Implements `I18nComponent`
  (`type Target = Text;` — `translate` branches to `translate_plural` when
  `count` is set, else `translate_by_key`) and the builders `new`,
  `with_count` (cfg `plurals`), `with_locale` (normalizes `_`→`-`, validates,
  logs + ignores invalid), `with_arg`, `with_num_arg` (cfg `numbers`,
  skips non-finite values via `try_f64_to_fd`). Also defines
  `pub(crate) enum InterpolationType`.
- `src/components/i18n_text_2d.rs` — `I18nText2d`: same in every respect
  except `#[require(Text2d)]`, `type Target = Text2d;`, and doc text.
- `src/components/mod.rs` — declares the modules, re-exports `*`, and holds
  the `I18nComponent` trait (Target: `Component<Mutability = Mutable> +
  DerefMut<Target = String>`).
- `src/plugin.rs:83-84` — registers both components; `src/plugin.rs:90-91`
  registers `I18nNumber` under cfg `numbers`. Reflection registration at
  `src/plugin.rs:64-65`.
- Verify current builder-method bodies by reading both files first — the
  `with_locale` bodies were just rewritten by plan 002 (normalize + validate +
  `bevy::log::error!` on failure) and MUST be preserved exactly.

Conventions: doc comments explain why; `cargo fmt` formatting; conventional
commits. The crate treats `I18nText`'s rustdoc (with its JSON + usage
examples) as user-facing documentation — it must survive on the generated
types.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile | `cargo check --all-targets` | exit 0 |
| Tests (full) | `cargo test --all-features` | 38 unit + 23 integration + 6 doc, all pass |
| Tests (minimal) | `cargo test --no-default-features` | all pass |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| Format | `cargo fmt` then `cargo fmt --check` | exit 0 |
| Public-API surface | `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src/components/i18n_text.rs`
- `src/components/i18n_text_2d.rs`
- `src/components/mod.rs`

**Out of scope** (do NOT touch):
- `src/components/i18n_number.rs` — different shape (no key/args), stays
  hand-written.
- `src/components/utils.rs`, `src/plural.rs`, `src/plugin.rs`,
  `tests/translation.rs` — no behavior change means no test or registration
  changes. If you find yourself editing a test, you changed behavior: STOP.
- The public API: type names, method names and signatures, derive list,
  `#[require(..)]` targets must all be identical to before.

## Git workflow

- Work on the current worktree branch. Conventional commit, e.g.
  `refactor(components): generate I18nText/I18nText2d from one macro`.
- Do NOT push.

## Steps

### Step 1: Write the macro

In `src/components/mod.rs` (bottom of file, above nothing in particular), add
a `macro_rules! define_i18n_text_component` that takes: the type name, the
target component type, and the struct-level doc attributes, e.g.:

```rust
macro_rules! define_i18n_text_component {
    (
        $(#[$struct_doc:meta])*
        $name:ident, target: $target:ty
    ) => {
        $(#[$struct_doc])*
        #[derive(Component, Default, Reflect, Debug, Clone)]
        #[reflect(Component)]
        #[require($target)]
        pub struct $name { /* fields exactly as today */ }

        impl crate::components::I18nComponent for $name { /* as today, Target = $target */ }

        impl $name { /* new, with_count, with_locale, with_arg, with_num_arg — bodies exactly as today */ }
    };
}
pub(crate) use define_i18n_text_component;
```

Load-bearing details:

- Move `InterpolationType` out of `i18n_text.rs` into `mod.rs` (it is
  `pub(crate)`; keep it `pub(crate)`), since both expansions reference it.
- The `#[cfg(feature = "plurals")]` field/method and
  `#[cfg(feature = "numbers")]` method go INSIDE the macro body unchanged —
  cfg attributes expand fine in macro output.
- Bevy's derives (`Component`, `Reflect`) and `#[require(..)]` must receive a
  concrete type — that is exactly what `$target:ty` provides.
- Use full paths (`bevy::prelude::...` / `crate::...`) inside the macro body
  rather than relying on the invoking module's imports.
- Method doc comments live once, inside the macro. For the struct-level docs
  (which differ per component and contain the user-facing examples), pass
  them at the invocation site via `$(#[$struct_doc:meta])*`.

**Verify**: `cargo check --all-targets` → exit 0 (after step 2; the macro
alone is unused until invoked — `#[allow(unused_macros)]` is NOT needed once
step 2 lands in the same commit).

### Step 2: Replace both files with invocations

`src/components/i18n_text.rs` shrinks to (preserving the existing rustdoc
verbatim as the doc attributes):

```rust
use super::define_i18n_text_component;

define_i18n_text_component!(
    /// Component for spawning translatable text entities ... (existing docs, verbatim)
    I18nText, target: bevy::prelude::Text
);
```

Same for `i18n_text_2d.rs` with `I18nText2d` / `Text2d`. Keep both files (one
per component, matching the repo's file layout) rather than collapsing into
`mod.rs`.

**Verify**: `cargo check --all-targets` → exit 0.

### Step 3: Confirm zero behavior change

**Verify** (all must pass unchanged — no test file was touched):
- `cargo test --all-features` → 38 + 23 + 6 pass.
- `cargo test --no-default-features` → all pass.
- `cargo clippy --all-targets --all-features -- -D warnings` → exit 0.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` → exit 0 (doc links in the
  migrated rustdoc still resolve).
- `cargo fmt --check` → exit 0.

## Test plan

No new tests — this is a behavior-preserving refactor and the existing suite
(including the doc-tests embedded in the struct docs you migrate) is the
referee. If any existing test needs editing to pass, that is a STOP condition,
not a test update.

## Done criteria

- [ ] `grep -c "pub struct I18nText" src/components/i18n_text.rs` → 0 (struct now macro-generated)
- [ ] `grep -c "define_i18n_text_component" src/components/mod.rs src/components/i18n_text.rs src/components/i18n_text_2d.rs` → ≥1 in each
- [ ] `cargo test --all-features` exits 0 with the same test counts as baseline (38/23/6)
- [ ] `cargo test --no-default-features` exits 0
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` exits 0
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `git status` shows only the three in-scope files modified

## STOP conditions

- Any existing test fails and the fix would require editing the test.
- `Reflect` or `Component` derive errors inside the macro expansion that you
  cannot resolve by using full paths — report the exact compiler error.
- The doc-tests in the migrated rustdoc fail to compile after migration
  (attribute-position issue with `$(#[$struct_doc:meta])*`) — report rather
  than deleting examples.
- You need to touch `plugin.rs`, `utils.rs`, or any test file.

## Maintenance notes

- Plans 005 (`I18nTextSpan`) and 006 (mutators) build directly on this macro —
  005 adds an invocation, 006 adds methods inside the macro body.
- Reviewer: diff the macro body against the pre-refactor method bodies
  line-by-line; the with_locale validation and with_num_arg skip logic from
  plan 002 must be byte-identical.
