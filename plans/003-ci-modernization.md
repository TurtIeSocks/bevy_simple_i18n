# Plan 003: Modernize CI — lint gates, wasm check, caching, and an unarchived publish toolchain

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9c4f661..HEAD -- .github/workflows/ Cargo.toml web/Cargo.toml`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW (CI/metadata only; no library code)
- **Depends on**: none — but land BEFORE tagging the v0.4.0 release, because
  `publish.yml` currently uses an archived action
- **Category**: dx
- **Planned at**: commit `9c4f661`, 2026-07-19

## Why this matters

The crate is clippy-clean and fmt-clean today, but CI enforces neither — one
merged PR can silently regress that. CI also never compiles the
wasm32 target, even though wasm support is a headline feature (the manifest
design exists *because of* wasm) and the deployed web demo is the README's
demo link; a wasm-breaking change would only be caught after merge by the
Pages deploy. Every CI job cold-compiles Bevy (~5–10 min) because no job uses
a cache, while the Pages workflow already demonstrates the fix
(`Swatinem/rust-cache@v2`). Finally, `publish.yml` — the workflow that will
ship 0.4.0 — still uses `actions-rs/toolchain@v1`, archived and unmaintained
since 2020, plus `actions/checkout@v3` (deprecated Node 16 runtime). Metadata
gaps ride along: no `[package.metadata.docs.rs]`, so docs.rs will not document
all features together.

## Current state

- `.github/workflows/ci.yml` — jobs: `check` (cargo check --all-targets),
  `docs` (cargo doc, `RUSTDOCFLAGS: -D warnings`), `build_examples`,
  `test` (3-OS matrix; `cargo test --all-features`, plus
  `--no-default-features` on ubuntu). All use `actions/checkout@v3` +
  `dtolnay/rust-toolchain@stable` + an apt-get line for Linux deps. No clippy,
  no fmt, no wasm, no cache.
- `.github/workflows/publish.yml` — tag-triggered publish. Excerpt:

  ```yaml
  # .github/workflows/publish.yml:12-19
        - name: Check out the code
          uses: actions/checkout@v3

        - name: Install Rust
          uses: actions-rs/toolchain@v1     # archived October 2023, unmaintained since 2020
          with:
            toolchain: stable
            override: true
  ```

  plus two manual `actions/cache@v3` blocks for the cargo registry/index, a
  `toml-cli` version check, `cargo publish --dry-run`, `cargo publish`.
- `.github/workflows/page.yaml` — the healthy exemplar: `actions/checkout@v4`,
  `dtolnay/rust-toolchain@master`, `Swatinem/rust-cache@v2` with
  `cache-all-crates: true`, installs the wasm target with
  `rustup target add wasm32-unknown-unknown`, builds `web/` with Trunk.
- `Cargo.toml` — no `[package.metadata.docs.rs]` section, no `rust-version`
  field. Workspace members: root + `web`.
- `web/Cargo.toml` — `version = "0.3.0"` (stale; root is 0.4.0; the web crate
  is unpublished so this is cosmetic).

Verified baseline (2026-07-19, commit `9c4f661`):
`cargo clippy --all-targets --all-features` clean,
`cargo fmt --check` clean, `cargo test --all-features` 36+19+6 green.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| Format | `cargo fmt --check` | exit 0 |
| Wasm check | `rustup target add wasm32-unknown-unknown && cargo check --target wasm32-unknown-unknown` | exit 0 |
| Workflow syntax | `gh workflow list` after push, or a YAML parse (`python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))"`) | no error |
| Bevy MSRV lookup | `cargo metadata --format-version 1 \| python3 -c "import json,sys; print([p['rust_version'] for p in json.load(sys.stdin)['packages'] if p['name']=='bevy'][0])"` | prints a version like `1.88.0` |

## Scope

**In scope** (the only files you should modify):
- `.github/workflows/ci.yml`
- `.github/workflows/publish.yml`
- `Cargo.toml` (metadata additions only)
- `web/Cargo.toml` (version bump only)
- `Cargo.lock` (committed on main 2026-07-19; the `web/Cargo.toml` version
  bump updates the recorded `bevy_simple_i18n_web` version the next time any
  cargo command runs — commit that mechanical lockfile diff together with the
  version bump; make no other lockfile changes)

**Out of scope** (do NOT touch):
- `.github/workflows/page.yaml` — already modern; it is the exemplar, not a
  patient.
- Any `src/`, `tests/`, `examples/`, `web/src/` code.
- Do NOT add new required checks that currently fail (everything gated below
  is verified green at the planned-at commit).

## Git workflow

- Branch: `c/ci-modernization` from up-to-date `main` (or the branch prepared
  for you).
- Conventional commits, one per workflow file is fine, e.g.
  `chore(ci): add clippy, fmt and wasm gates with caching`,
  `chore(ci): replace archived actions-rs toolchain in publish workflow`,
  `chore: docs.rs all-features metadata + MSRV`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Upgrade shared plumbing in `ci.yml`

For **every** job in `.github/workflows/ci.yml`:
- `actions/checkout@v3` → `actions/checkout@v4` (matches `page.yaml`).
- Immediately after the toolchain step, add:

  ```yaml
        - uses: Swatinem/rust-cache@v2
  ```

Keep the existing apt-get dependency lines exactly as they are.

**Verify**: YAML parse command from the table → no error.

### Step 2: Add lint gates to `ci.yml`

Add one new job (mirroring the `check` job's checkout/toolchain/deps
structure; clippy needs the same Linux deps as check):

```yaml
  lint:
    name: Clippy + rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - name: Install Linux dependencies
        run: sudo apt-get update; sudo apt-get install pkg-config libx11-dev libasound2-dev libudev-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libwayland-dev libxkbcommon-dev
      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings
      - name: Rustfmt
        run: cargo fmt --check
```

**Verify**: run both commands locally → exit 0 (baseline is clean; if either
fails locally, STOP — the baseline drifted).

### Step 3: Add a wasm-target check job to `ci.yml`

```yaml
  wasm:
    name: Check wasm32 target
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      - name: Check the library for wasm
        run: cargo check --target wasm32-unknown-unknown
      - name: Check the web demo for wasm
        run: cargo check -p bevy_simple_i18n_web --target wasm32-unknown-unknown
```

No apt-get line — wasm builds don't need the Linux windowing libs.

**Verify**: run both check commands locally (after
`rustup target add wasm32-unknown-unknown`) → exit 0. If the `-p
bevy_simple_i18n_web` check fails locally for a reason unrelated to your
changes (e.g. a bevy feature that needs extra wasm setup), drop ONLY that
second step, keep the library check, and note it in your report.

### Step 4: Fix `publish.yml`

- `actions/checkout@v3` → `actions/checkout@v4`.
- Replace the `actions-rs/toolchain@v1` step with
  `uses: dtolnay/rust-toolchain@stable` (no `with:` needed).
- Replace the two manual `actions/cache@v3` blocks (registry + index) with a
  single `- uses: Swatinem/rust-cache@v2`.
- Keep the toml-cli version check, `--dry-run`, and publish steps unchanged.

**Verify**: YAML parse command → no error; `grep -c "actions-rs" .github/workflows/publish.yml` → `0`.

### Step 5: Cargo metadata

In `Cargo.toml`:

1. Add (anywhere after `[package]`, conventionally right after it):

   ```toml
   [package.metadata.docs.rs]
   all-features = true
   ```

2. Add a `rust-version` field to `[package]` set to the value the MSRV lookup
   command (table above) prints for bevy — the crate cannot compile below
   bevy's own MSRV, so matching it is honest without a bisection exercise.

In `web/Cargo.toml`: `version = "0.3.0"` → `version = "0.4.0"`.

**Verify**: `cargo check --all-targets` → exit 0 (metadata is inert);
`cargo publish --dry-run` → completes through packaging/verification
(network access to crates.io required; if the environment blocks it, note
that and rely on `cargo package --list` succeeding instead).

## Test plan

No code changes → no new unit tests. The verification is:
- All Step 2/3 commands green locally.
- After the PR opens (if the operator pushes), all five CI jobs green,
  including the two new ones.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `grep -rn "checkout@v3\|actions-rs" .github/workflows/` returns no matches
- [ ] `grep -c "Swatinem/rust-cache" .github/workflows/ci.yml` ≥ 4 (every job cached)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `cargo check --target wasm32-unknown-unknown` exits 0
- [ ] `grep -A1 "package.metadata.docs.rs" Cargo.toml` shows `all-features = true`
- [ ] `grep "rust-version" Cargo.toml` shows a concrete version
- [ ] `git status` shows only in-scope files (plus `plans/README.md`) modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Clippy or fmt fails at baseline (before your edits) — the repo drifted from
  the verified-clean state this plan assumes; the fix belongs in its own
  change, not smuggled into a CI PR.
- The wasm library check fails at baseline — same reasoning.
- `cargo metadata` reports no `rust_version` for bevy — pick nothing; report
  and leave `rust-version` out rather than guessing.

## Maintenance notes

- When bevy is next bumped, `rust-version` must be re-checked against the new
  bevy MSRV.
- The publish workflow runs rarely; the reviewer should eyeball the final
  `publish.yml` extra carefully since a broken publish is only discovered at
  release time. Tagging `v0.4.0` should happen only after this lands.
- Deferred deliberately: a feature-combination matrix (e.g. `plurals` without
  `yaml`) — the two existing test invocations cover the realistic extremes;
  add a matrix only if a feature-combination bug actually appears.
