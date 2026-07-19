# Plan 001: Make the locale buttons in the example and web demo appear after translations load

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9c4f661..HEAD -- examples/changing_locale.rs web/src/main.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `9c4f661`, 2026-07-19

## Why this matters

Since the 0.4.0 rewrite, translations load **asynchronously** through the Bevy
`AssetServer` instead of being embedded at compile time. Both the
`changing_locale` example and the web demo (deployed to GitHub Pages — it is
the README's "Demo" link) read `I18n::locales()` inside a `Startup` system to
spawn one locale-switching button per available locale. At `Startup`, on frame
zero, the manifest and translation files have not loaded yet, so `locales()`
returns an **empty slice** — zero buttons spawn, and the demo's locale
switcher is silently gone. Before 0.4.0 this worked because locales were baked
into the binary and available immediately. This is a user-visible regression
in the project's shop window.

## Current state

- `examples/changing_locale.rs` — desktop example. `setup` (a `Startup`
  system) spawns the UI, including the button row; `button_system` (an
  `Update` system) handles clicks.
- `web/src/main.rs` — the wasm demo built by Trunk and deployed by
  `.github/workflows/page.yaml`. Near-identical copy of the example: same
  `setup` / `button_system` pair, same bug.

The buggy pattern, `examples/changing_locale.rs:14` and `:99-135` (the web
copy is `web/src/main.rs:34` and `:119-155`):

```rust
fn setup(mut commands: Commands, i18n_res: Res<I18n>) {
    ...
            parent
                .spawn(Node {
                    display: Display::Flex,
                    ...
                    ..default()
                })
                .with_children(|parent| {
                    for locale in i18n_res.locales() {   // EMPTY at Startup
                        parent.spawn((Button, ...))
                              .with_child((Text::new(locale), ...));
                    }
                });
```

Facts you need about the plugin's runtime (all in `src/`, do not modify them):

- `I18n` is a Bevy `Resource` (`src/resources.rs:26`). `I18n::locales()`
  returns `&[String]`, empty until the asset-driven table is built.
- The table is rebuilt by `sync_translations` in `PreUpdate`
  (`src/plugin.rs:82`), which mutates `Res<I18n>` — so `i18n.is_changed()` is
  `true` on any frame where the table was (re)built, including hot reloads.
- `I18n::ready()` (`src/resources.rs:93`) turns `true` once the manifest and
  all translation files finished loading (or failed terminally).

Repo conventions: plain Bevy 0.19 idioms, `cargo fmt` formatting, doc comments
on systems. Commit style is conventional commits (see `git log --oneline`:
`fix(docs): ...`, `feat: ...`).

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Compile everything | `cargo check --all-targets` | exit 0 |
| Build the example | `cargo build --example changing_locale` | exit 0 |
| Run the example (manual check) | `cargo run --example changing_locale` | window opens, locale buttons visible |
| Tests | `cargo test --all-features` | all pass |
| Format | `cargo fmt` then `cargo fmt --check` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `examples/changing_locale.rs`
- `web/src/main.rs`

**Out of scope** (do NOT touch, even though they look related):
- Anything under `src/` — the plugin behaves correctly; only the two demo
  binaries misuse it. Do not add a "run system when ready" helper to the
  plugin in this plan.
- `examples/basic.rs` — it does not enumerate locales; unaffected.
- `.github/workflows/page.yaml` — the deploy pipeline is fine.

## Git workflow

- Branch: work on the current branch if one was prepared for you; otherwise
  `git checkout -b c/fix-demo-locale-buttons` from up-to-date `main`.
- One commit, message: `fix(examples): spawn locale buttons after translations load`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Fix `examples/changing_locale.rs`

1. Add a marker component near the top of the file:

```rust
/// Marks the container the locale buttons are (re)built under.
#[derive(Component)]
struct LocaleButtonRow;
```

2. In `setup`, remove the `i18n_res: Res<I18n>` system parameter and replace
   the button-row block (the `parent.spawn(Node { ... }).with_children(|parent| { for locale in i18n_res.locales() { ... } })`
   at the end of `setup`) with spawning the same container `Node` **empty**,
   tagged with `LocaleButtonRow`:

```rust
            // Locale buttons are spawned by `spawn_locale_buttons` once the
            // translation table has loaded (assets are async as of 0.4).
            parent.spawn((
                Node {
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(10.),
                    column_gap: Val::Px(10.),
                    ..default()
                },
                LocaleButtonRow,
            ));
```

3. Add a new `Update` system that rebuilds the row whenever `I18n` changes
   (covers initial load AND hot-reload-added locales), and register it in
   `main()` with `.add_systems(Update, (button_system, spawn_locale_buttons))`:

```rust
/// (Re)builds one button per available locale. `I18n` changes when the
/// translation table is built or rebuilt, so this also picks up locales added
/// by hot reload.
fn spawn_locale_buttons(
    mut commands: Commands,
    i18n: Res<I18n>,
    row: Single<Entity, With<LocaleButtonRow>>,
) {
    if !i18n.is_changed() {
        return;
    }
    let mut row = commands.entity(*row);
    row.despawn_related::<Children>();
    row.with_children(|parent| {
        for locale in i18n.locales() {
            parent
                .spawn((
                    Button,
                    Node {
                        min_width: Val::Px(200.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::BLACK),
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                ))
                .with_child((
                    Text::new(locale),
                    TextFont {
                        font_size: FontSize::Px(50.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                ));
        }
    });
}
```

   Notes for the executor:
   - The button/`Node` styling values above are copied verbatim from the
     current code — keep them identical.
   - `Single<Entity, With<LocaleButtonRow>>` is the Bevy 0.19 single-result
     query parameter; the system is skipped automatically until the row
     entity exists. If the Bevy version in `Cargo.toml` rejects `Single`,
     use `Query<Entity, With<LocaleButtonRow>>` + `let Ok(row) = query.single() else { return; }`.
   - `despawn_related::<Children>()` is the Bevy 0.19 replacement for the old
     `despawn_descendants()`. If it does not compile, check
     `EntityCommands` methods for the current children-despawn API and STOP
     if none exists.

**Verify**: `cargo build --example changing_locale` → exit 0.

### Step 2: Apply the identical fix to `web/src/main.rs`

Same three edits (marker component, empty tagged container in `setup`, new
`spawn_locale_buttons` system registered in `main()`). The two files are
intentionally near-identical copies — keep them that way.

**Verify**: `cargo check --all-targets` → exit 0 (this compiles the `web`
workspace member for the host target).

### Step 3: Full verification

**Verify**:
- `cargo test --all-features` → all pass (no behavior change in the library).
- `cargo fmt --check` → exit 0.
- If a display is available: `cargo run --example changing_locale` → the
  locale buttons (`cs`, `en`, `ja`, `uk`, `zh-TW`) appear within a moment of
  launch, and clicking one switches the text. If headless, note in your
  report that the manual check was skipped.

## Test plan

No new automated tests: the affected code lives in example/demo binaries,
which the test suite does not execute. The library behavior they exercise
(`I18n::locales()` populated after load) is already covered by
`available_locales_are_sorted_and_complete` in `tests/translation.rs:175`.
Manual verification is Step 3.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cargo check --all-targets` exits 0
- [ ] `cargo test --all-features` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `grep -n "for locale in i18n_res.locales()" examples/changing_locale.rs web/src/main.rs` returns no matches (the Startup-time enumeration is gone)
- [ ] `grep -c "LocaleButtonRow" examples/changing_locale.rs` ≥ 2 and same for `web/src/main.rs`
- [ ] `git status` shows only the two in-scope files (plus `plans/README.md`) modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The excerpts in "Current state" don't match the live files (drift).
- `despawn_related::<Children>()` and every obvious children-despawn
  alternative fail to compile — report the compiler error instead of
  inventing an entity-management workaround.
- The example compiles but locale buttons still don't appear when run — that
  would point at a plugin bug, which is out of scope here; report it.

## Maintenance notes

- If the plugin later grows a "translations ready" run condition or observer
  API, these systems are the natural first consumer — simplify them then.
- Reviewer should check the two files stayed structurally identical (they are
  deliberate copies), and that button styling is unchanged.
- Deliberately deferred: deduplicating the example and web demo into one
  shared module — the duplication predates this plan and is a cosmetic
  concern.
