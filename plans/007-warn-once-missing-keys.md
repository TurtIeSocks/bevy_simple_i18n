# Plan 007: Warn once per missing (locale, key) instead of every retranslate

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on. If
> anything in the "STOP conditions" section occurs, stop and report — do not
> improvise. Your reviewer maintains `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 3870c9f..HEAD -- src/components/utils.rs src/resources.rs`
> If either file changed since this plan was written, compare the "Current
> state" excerpts against the live code before proceeding; on a mismatch,
> treat it as a STOP condition. (Concurrent plans 004–006 do not touch these
> two files; a diff here means real drift.)

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none (independent of 004–006; touches disjoint files)
- **Category**: dx
- **Planned at**: commit `3870c9f`, 2026-07-19

## Why this matters

`translate_resolved` logs `warn!("Missing translation for key ...")` on EVERY
translate of a missing key — and every registered component re-translates on
every `I18n` change. A scene with 50 entities referencing 3 missing keys logs
150 warnings per locale switch, drowning the console precisely when the
developer is trying to read it. The warning itself is valuable (the crate's
"visible, not invisible" failure philosophy); the repetition is not. Dedupe:
warn the FIRST time a (locale, key) miss is seen, log at `debug!` thereafter,
and reset the seen-set when the translation table is rebuilt (a hot reload
that ADDS the key should re-warn if the key goes missing again later).

## Current state

- `src/components/utils.rs`, `translate_resolved` (~line 90-115 in the
  pre-004 tree — re-locate by grepping `Missing translation for key`):

```rust
let translated = match i18n.translate(locale, key) {
    Some(text) => text,
    None => {
        if i18n.ready() {
            bevy::log::warn!("Missing translation for key `{key}` (locale `{locale}`)");
        } else {
            bevy::log::debug!(
                "Translation for key `{key}` requested before locale assets loaded"
            );
        }
        key
    }
};
```

- `src/resources.rs` — the `I18n` resource. Design constraint from the crate's
  own charter (`docs/systematic-refactor/design.md` D3): ALL i18n state lives
  in the ECS resource — no global statics, no `static` seen-sets. The
  seen-set therefore goes on `I18n`. `translate`/`translate_resolved` only
  have `&I18n`, so the set needs interior mutability:
  `missed: std::sync::Mutex<bevy::platform::collections::HashSet<(String, String)>>`
  (`Mutex`, not `RwLock` — the lock is touched only on the miss path, which is
  rare and already logging). Mark it `#[reflect(ignore)]`, skip it in `Debug`
  if the derive complains (switch to a manual `Debug` impl only if needed —
  `Mutex<HashSet<..>>` does implement `Debug`, so likely no change).
- `I18n` implements `Default` manually (`src/resources.rs:51-63`) — add the
  field there. NOTE: `I18n` derives `Clone`? It does NOT (only
  `Debug, Resource, Reflect`) — so `Mutex` is fine.
- `apply_table` (`src/resources.rs`, `pub(crate) fn apply_table`) — the reset
  point: clear the set whenever the table is rebuilt.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile | `cargo check --all-targets` | exit 0 |
| Tests | `cargo test --all-features` | all pass incl. new test |
| Minimal | `cargo test --no-default-features` | all pass |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| Format | `cargo fmt` then `cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src/resources.rs` (field + `note_miss` helper + reset in `apply_table`)
- `src/components/utils.rs` (`translate_resolved` calls the helper)

**Out of scope**:
- Everything else. No log-capture test infrastructure; no config knob for the
  behavior (YAGNI).

## Git workflow

- Current worktree branch; conventional commit, e.g.
  `feat: warn once per missing (locale, key), debug thereafter`.
- Do NOT push.

## Steps

### Step 1: Seen-set on `I18n`

In `src/resources.rs`:

- Add the field with a doc comment explaining the why (log dedupe, reset on
  table rebuild, lives here per the no-globals charter):

```rust
/// (locale, key) pairs already warned about as missing — so a missing key
/// warns once, not once per entity per retranslate. Interior mutability
/// because lookups take `&self`; reset on every table rebuild so hot
/// reloads re-report. Lives here (not a static) per the crate's
/// no-global-state charter.
#[reflect(ignore)]
missed: std::sync::Mutex<HashSet<(String, String)>>,
```

  (`bevy::platform::collections::HashSet` is already what the file's
  `HashMap` import comes from — extend that import.)
- Initialize in `Default::default()` (`Mutex::new(HashSet::default())`).
- Reset in `apply_table`: `self.missed.lock().unwrap().clear();` (a poisoned
  lock here means another thread panicked mid-log — `unwrap` is fine, match
  crate style; or use `.lock().unwrap_or_else(|e| e.into_inner())` if clippy
  objects).
- Add the helper:

```rust
/// Records a missing (locale, key) lookup. Returns `true` the FIRST time this
/// pair is seen since the last table rebuild — the caller warns on `true`,
/// logs at debug on `false`.
pub(crate) fn note_miss(&self, locale: &str, key: &str) -> bool {
    self.missed
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert((locale.to_string(), key.to_string()))
}
```

**Verify**: `cargo check --all-targets` → exit 0.

### Step 2: Use it in `translate_resolved`

Replace the `if i18n.ready()` warn branch:

```rust
None => {
    if !i18n.ready() {
        bevy::log::debug!(
            "Translation for key `{key}` requested before locale assets loaded"
        );
    } else if i18n.note_miss(locale, key) {
        bevy::log::warn!("Missing translation for key `{key}` (locale `{locale}`)");
    } else {
        bevy::log::debug!("Missing translation for key `{key}` (locale `{locale}`)");
    }
    key
}
```

**Verify**: `cargo check --all-targets` → exit 0.

### Step 3: Unit tests

In the existing `mod tests` in `src/resources.rs`:

```rust
#[test]
fn note_miss_dedupes_until_table_rebuild() {
    let mut i18n = I18n::default();
    assert!(i18n.note_miss("en", "gone"), "first miss reports");
    assert!(!i18n.note_miss("en", "gone"), "repeat is deduped");
    assert!(i18n.note_miss("ja", "gone"), "different locale is a new pair");
    i18n.apply_table(Table::default(), "en", Vec::new(), true);
    assert!(i18n.note_miss("en", "gone"), "table rebuild resets the set");
}
```

**Verify**: `cargo test --all-features` → all pass;
`cargo test --no-default-features` → all pass;
`cargo clippy --all-targets --all-features -- -D warnings` → exit 0;
`cargo fmt --check` → exit 0.

## Test plan

The unit test above pins the dedupe contract (first-report, repeat-dedupe,
per-pair granularity, rebuild reset). The warn/debug branch itself is a
straight consumer of that contract; log-output capture infrastructure is
deliberately not added.

## Done criteria

- [ ] `grep -c "note_miss" src/resources.rs src/components/utils.rs` → ≥2 and ≥1
- [ ] `cargo test --all-features` exits 0; `note_miss_dedupes_until_table_rebuild` passes
- [ ] `cargo test --no-default-features` exits 0
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `git status` shows only the two in-scope files modified

## STOP conditions

- The `Reflect` derive rejects the `Mutex` field even with
  `#[reflect(ignore)]` — report the compiler error (a `#[reflect(opaque)]` or
  Default-bound issue needs a human call).
- Any existing test fails.
- You want to add a config option or log-capture test harness — out of scope.

## Maintenance notes

- If per-entity `warn!` sites appear elsewhere later (fonts?), reuse
  `note_miss`-style dedupe on the owning resource, not statics.
- Reviewer: confirm the pre-ready debug branch is unchanged (spawn-before-load
  stays debug, not warn — see `text_spawned_before_assets_load_self_heals`).
