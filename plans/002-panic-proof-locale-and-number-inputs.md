# Plan 002: Stop panicking on user-supplied locales and non-finite numbers; build the number formatter lazily

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9c4f661..HEAD -- src/components/ src/plural.rs src/resources.rs tests/translation.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED (touches the hot translate path; extensive parity tests exist and must stay green)
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `9c4f661`, 2026-07-19

## Why this matters

With the default `numbers` feature on, **every** translation call constructs
an ICU `DecimalFormatter` for the component's locale — even when there are no
number arguments at all. The constructor chain panics on an unparseable
locale. `I18n::set_locale` validates its input, but the per-entity
`.with_locale(...)` builder does not — so
`I18nText::new("hello").with_locale("en_US")` (underscore instead of hyphen, a
mistake every platform's locale APIs encourage) **crashes the game** at the
next translate. The `plurals` path has the same panic via manifest-supplied
`fallback` locales — and the manifest is runtime data, editable by modders and
language packs; data must never crash the game. Separately,
`I18nNumber::new(f64::NAN)` and `.with_num_arg("n", f64::NAN)` panic outright.
Fixing this also removes a per-entity-per-retranslate allocation: the
formatter should only be built when a number actually needs formatting.

## Current state

All panics funnel through three helpers in
`src/components/utils.rs`:

```rust
// src/components/utils.rs:4-8
pub(crate) fn f64_to_fd(value: f64) -> fixed_decimal::Decimal {
    fixed_decimal::Decimal::try_from_f64(value, fixed_decimal::FloatPrecision::RoundTrip)
        .unwrap_or_else(|err| panic!("Failed to parse Decimal from f64 {value}: {err}"))
}

// src/components/utils.rs:19-27
pub(crate) fn resolve_locale(locale: &str, label: impl std::fmt::Display) -> icu_locale_core::Locale {
    locale.parse()
        .unwrap_or_else(|err| panic!("Invalid locale: {locale} for key: {label}: {err}"))
}

// src/components/utils.rs:29-38
pub(super) fn get_formatter(locale: &str, label: impl std::fmt::Display) -> icu_decimal::DecimalFormatter {
    let locale = resolve_locale(locale, &label);
    icu_decimal::DecimalFormatter::try_new((&locale).into(), Default::default()).unwrap_or_else(
        |err| panic!("Failed to create DecimalFormatter for {label} with locale {locale}: {err}"),
    )
}
```

The eager formatter construction (even with zero args) is in `build_args`:

```rust
// src/components/utils.rs:68-88
fn build_args<'a>(locale: &str, key: &str, args: &'a [(String, InterpolationType)])
    -> (Vec<&'a str>, Vec<String>)
{
    #[cfg(not(feature = "numbers"))]
    let _ = (locale, key);
    #[cfg(feature = "numbers")]
    let fdf = get_formatter(locale, key);          // <-- built unconditionally

    args.iter()
        .map(|(k, interpolation_type)| {
            let value = match interpolation_type {
                InterpolationType::String(v) => v.clone(),
                #[cfg(feature = "numbers")]
                InterpolationType::Number(v) => fdf.format_to_string(v),
            };
            (k.as_str(), value)
        })
        .unzip()
}
```

Panic entry points (callers that accept unvalidated input):

- `I18nText::with_locale` — `src/components/i18n_text.rs:110-113` — stores the
  string raw. Same for `I18nText2d::with_locale`
  (`src/components/i18n_text_2d.rs:96-99`) and `I18nNumber::with_locale`
  (`src/components/i18n_number.rs:61-64`).
- `I18nNumber::new` — `src/components/i18n_number.rs:53-58` — calls the
  panicking `f64_to_fd`. Its `translate` (`i18n_number.rs:45-48`) calls
  `get_formatter`.
- `with_num_arg` on both text components calls the panicking `f64_to_fd`
  (`i18n_text.rs:128-134`, `i18n_text_2d.rs:114-120`).
- `src/plural.rs:49-50` — calls `resolve_locale(candidate_locale, key)` where
  `candidate_locale` iterates `I18n::candidate_locales`, which includes the
  **manifest's `fallback` list, stored unvalidated** (see
  `src/resources.rs:127-139` `apply_table` and `src/assets.rs:191-197`
  manifest loader).
- `translate_plural` — `src/components/utils.rs:53-66` — calls
  `get_formatter(locale, key)` to format `%{count}`.

The two existing *good* precedents to imitate:

- `I18n::set_locale` validates and rejects with an error log
  (`src/resources.rs:68-77`).
- `detect_system_locale` normalizes underscores then validates
  (`src/plugin.rs:150-161`):
  ```rust
  match raw.replace('_', "-").parse::<icu_locale_core::Locale>() { ... }
  ```
- `try_f64_to_fd` is the existing non-panicking Decimal conversion
  (`src/components/utils.rs:12-17`), currently used only by `with_count`.

Conventions: doc comments explain *why*; parity with rust-i18n behavior is
pinned by `tests/translation.rs` and the unit tests in each module — those
semantics must not change for valid inputs. Conventional-commit messages.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile | `cargo check --all-targets` | exit 0 |
| Tests (full) | `cargo test --all-features` | all pass (baseline at plan time: 36 unit + 19 integration + 6 doc) |
| Tests (minimal) | `cargo test --no-default-features` | all pass |
| Lint | `cargo clippy --all-targets --all-features` | exit 0, no new warnings (baseline: clean) |
| Format | `cargo fmt` then `cargo fmt --check` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src/components/utils.rs`
- `src/components/i18n_text.rs`
- `src/components/i18n_text_2d.rs`
- `src/components/i18n_number.rs`
- `src/plural.rs`
- `tests/translation.rs` (add tests)

**Out of scope** (do NOT touch, even though they look related):
- `src/resources.rs` — `set_locale` already validates; `candidate_locales`
  must keep returning raw strings (translation lookup by string key works
  fine with any string; only ICU calls need parsed locales).
- `src/assets.rs` / manifest loading — do not reject manifest `fallback`
  entries at load time; the graceful degradation in this plan makes that
  unnecessary, and dropping entries would change lookup behavior.
- `src/interpolate.rs`, `src/parse.rs` — untouched by this change.
- Public API shape: `with_locale`, `new`, `with_num_arg` signatures must not
  change.

## Git workflow

- Branch: `c/panic-proof-i18n-inputs` from up-to-date `main` (or the branch
  prepared for you).
- Commit per logical unit (see steps); conventional style, e.g.
  `fix: degrade gracefully on invalid locales and non-finite numbers`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Make the three helpers non-panicking

In `src/components/utils.rs`:

1. `resolve_locale` → return `Option<icu_locale_core::Locale>`; on parse
   failure, `bevy::log::error!` (keep the same message content, minus the
   panic) and return `None`. Rename to `try_resolve_locale` only if clippy
   complains; otherwise keep the name to minimize churn.
2. `get_formatter` → return `Option<icu_decimal::DecimalFormatter>`:
   `resolve_locale(...)?`, then on `DecimalFormatter::try_new` failure log
   `error!` and return `None` (formatter-data failure is practically
   unreachable with compiled data but must not panic either).
3. `f64_to_fd` → delete it, and make `try_f64_to_fd` available under
   `#[cfg(feature = "numbers")]` as well as `plurals` (adjust the `cfg` to
   `#[cfg(any(feature = "numbers", feature = "plurals"))]` — note `plurals`
   already implies `numbers` in `Cargo.toml:26`, so plain
   `#[cfg(feature = "numbers")]` is also correct and simpler; prefer that).
   Update its log message so it fits both counts and number values, e.g.
   "Ignoring non-finite number {value}: {err}".

Fallback behavior for a `None` formatter, used by the next steps: **format
the number with `Decimal`'s own `Display`** (`value.to_string()`), which is
locale-neutral but correct digits — text still renders, an error is logged
once per offending translate.

**Verify**: `cargo check --all-targets` → compile errors ONLY at the call
sites you will fix in steps 2–4 (or exit 0 if you fix them in the same pass).

### Step 2: Fix the call sites in `utils.rs` itself

1. `build_args`: build the formatter **lazily and at most once** — only when
   the first `InterpolationType::Number` value is met:

```rust
#[cfg(feature = "numbers")]
let mut fdf: Option<Option<icu_decimal::DecimalFormatter>> = None; // memoized try
...
InterpolationType::Number(v) => fdf
    .get_or_insert_with(|| get_formatter(locale, key))
    .as_ref()
    .map(|f| f.format_to_string(v))
    .unwrap_or_else(|| v.to_string()),
```

   (Any equivalent memoization is fine; the requirements are: zero formatter
   construction when there are no number args, at most one construction
   otherwise, `Display` fallback when construction fails.)

2. `translate_plural`: replace the panicking
   `get_formatter(locale, key).format_to_string(count)` with the same
   `Option` handling (`Display` fallback for `%{count}`).

**Verify**: `cargo check --no-default-features` and
`cargo check --all-targets` → exit 0 except remaining call sites in
`i18n_number.rs` / `plural.rs` / builders.

### Step 3: Fix `I18nNumber` and the builders

1. `I18nNumber::new`: use the non-panicking conversion. Change the field to
   `pub(crate) fixed_decimal: Option<Decimal>` (it is `pub(crate)`, not public
   API). `translate` renders `Some(d)` via the (now-`Option`) formatter with
   `Display` fallback, and `None` as an **empty string** after logging at
   construction time. Update the `Default` derive if needed
   (`Option<Decimal>` defaults to `None` — acceptable: an
   `I18nNumber::default()` renders empty, same spirit as today's
   `Decimal::default()` rendering "0"; if you prefer preserving "0", use
   `try_f64_to_fd(0.0)` in a manual `Default` impl — either is fine, document
   the choice in the commit message).
2. `with_num_arg` (both text components): on a non-finite value, log the
   error (via the shared helper) and **skip pushing the arg** — the
   `%{name}` pattern then stays verbatim in the output, which is this crate's
   established "visible, not invisible" failure mode (see
   `src/interpolate.rs:3-6` doc comment).
3. `with_locale` (all three components): normalize + validate, imitating
   `detect_system_locale` (`src/plugin.rs:153`):

```rust
pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
    let raw: String = locale.into();
    let normalized = raw.replace('_', "-");
    match normalized.parse::<icu_locale_core::Locale>() {
        Ok(_) => self.locale = Some(normalized),
        Err(err) => bevy::log::error!("Ignoring invalid locale {raw:?}: {err}"),
    }
    self
}
```

   Keep the three implementations textually identical (they are deliberate
   triplets). Note this is a small behavior *improvement*: `en_US` now works
   instead of silently missing (pre-plan it panicked), and invalid input
   falls back to the global locale.

**Verify**: `cargo check --all-targets` → exit 0.

### Step 4: Fix the plural path

In `src/plural.rs:49-50`, `resolve_locale` now returns `Option`; on `None`,
skip the CLDR-category block for that candidate locale and continue with the
`other` / bare-key candidates (the surrounding loop structure already
supports this — the category block is already conditional on
`try_new_cardinal` succeeding):

```rust
if let Some(parsed) = crate::components::utils::resolve_locale(candidate_locale, key) {
    if let Ok(rules) = icu_plurals::PluralRules::try_new_cardinal((&parsed).into()) {
        ...existing category logic...
    }
}
```

**Verify**: `cargo test --all-features` → all pass (the existing
`src/plural.rs` unit tests pin the resolution order).

### Step 5: Add regression tests

In `tests/translation.rs`, following the style of the existing
`non_finite_counts_do_not_panic` test (`tests/translation.rs:254-267`):

**Verify**: `cargo test --all-features` → all pass including the new tests;
`cargo test --no-default-features` → all pass.

## Test plan

New integration tests in `tests/translation.rs` (each is the panic scenario
it guards, so simply *completing without panicking* plus the asserted render
is the regression check):

1. `underscore_locale_is_normalized_not_fatal` — spawn
   `I18nText::new("hello").with_locale("ja_JP")` (underscore); assert the text
   renders `"こんにちは世界"` (normalized to `ja-JP`, truncation chain finds
   `ja`). Gate with `#[cfg(feature = "yaml")]` like the neighboring tests.
2. `invalid_locale_falls_back_to_global_not_panic` — spawn
   `I18nText::new("hello").with_locale("not a locale!")`; assert it renders
   the **global** locale's translation (invalid override ignored).
3. `invalid_manifest_fallback_does_not_panic_plurals` — `#[cfg(feature = "plurals")]`:
   build an `I18n` scenario where the fallback chain contains an invalid tag.
   Simplest route: unit test in `src/plural.rs` instead — construct `I18n`
   via `apply_table(table, "en", vec!["!!bad!!".into()], true)` (the pattern
   used by `fallback_locale_plurals_use_the_fallback_locales_categories`,
   `src/plural.rs:156-181`) and assert `resolve_plural_key` returns without
   panicking. Put it where it is easiest; either file is acceptable.
4. `nan_number_renders_empty_not_panic` — `#[cfg(feature = "numbers")]`:
   spawn `I18nNumber::new(f64::NAN)`; assert text is `""`.
5. `nan_num_arg_leaves_pattern_verbatim` —
   `#[cfg(all(feature = "numbers", feature = "yaml"))]`: spawn
   `I18nText::new("messages.cats").with_num_arg("count", f64::INFINITY).with_locale("en")`;
   assert `"You have %{count} cats"`.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `grep -n "panic!" src/components/utils.rs` returns no matches
- [ ] `cargo test --all-features` exits 0, including the 5 new tests
- [ ] `cargo test --no-default-features` exits 0
- [ ] `cargo clippy --all-targets --all-features` exits 0 with no new warnings
- [ ] `cargo fmt --check` exits 0
- [ ] `git status` shows only in-scope files (plus `plans/README.md`) modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Excerpts don't match the live code (drift).
- Any pre-existing parity test fails after your change — the rust-i18n parity
  semantics for VALID inputs are contractual; report the failing vector, do
  not adjust the test's expectation.
- Making `I18nNumber.fixed_decimal` an `Option` cascades into public API
  breakage you didn't anticipate (something outside the crate reads it).
- The `Display` fallback for `Decimal` produces scientific notation or another
  surprising format for plain values (check `Decimal::to_string` on `2503.1`
  in a quick unit assert; expected `"2503.1"`). If it does, report — a
  different fallback needs a human decision.

## Maintenance notes

- Future formatter caching (a `Res` keyed by locale) would supersede the
  lazy-build in `build_args`; the lazy shape here makes that a drop-in later.
- Reviewer scrutiny: the plural resolution order (exact int → category →
  other → bare key, per locale before next locale) must be byte-identical for
  valid locales; only the invalid-locale behavior changes.
- Deferred deliberately: caching `DecimalFormatter`/`PluralRules` per locale
  (perf, not correctness), and deduplicating the `I18nText`/`I18nText2d`
  builder triplets via a macro — both listed as separate findings in the
  audit, not planned yet.
