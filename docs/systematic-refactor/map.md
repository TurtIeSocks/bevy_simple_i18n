# Refactor Map — rust-i18n → native Bevy i18n (target: 0.4.0)

Primary deliverable. Actions: port | port + redesign | rewrite | write fresh | drop | keep.

## New Structure

```
(repo root)
  build.rs
    Action: DROP — entire file. Both jobs move to runtime:
            locale embedding → asset loading; font discovery → manifest declaration.
            Kills: BEVY_ASSET_PATH env var, cargo-emit build-dep, docs.rs special case,
            wasm double-shipping, workspace-detection heuristic.

  Cargo.toml
    Action: rewrite deps
    Notes: - rust-i18n, - cargo-emit (build-deps gone entirely)
           + serde, + serde_json, + ron (manifest)
           + serde_yml  (feature "yaml", default on — needed for v2_example.yml compat)
           + toml       (feature "toml", default on)
           icu bump SEPARATE commit within 0.4: icu_locid 1.5 → icu_locale_core 2.2
           (icu_locid formally deprecated), fixed_decimal 0.5.6 → 0.7 (FixedDecimal→Decimal),
           icu_decimal 1.5 → 2.2 (FixedDecimalFormatter→DecimalFormatter, preference-bag ctor).

src/
  lib.rs
    ← was lib.rs with include!(OUT_DIR/bevy_simple_i18n.rs)
    Action: port (delete include! line, add new modules to prelude)

  assets.rs                                    ← NEW module, no old equivalent
    I18nManifest (Asset)
      ← replaces build.rs locale discovery + FONT_FAMILIES codegen
      Action: write fresh
      Notes: RON asset, conventional path "locales/i18n.ron". Shape:
             (
                 default_locale: "en",            // optional, default "en"
                 fallback: [],                    // optional explicit fallback locales (parity: empty)
                 files: ["en.json", "v2_example.yml"],   // relative to manifest dir; ORDER = merge order (later wins)
                 fonts: [ (family: "NotoSans", dir: "fonts/NotoSans",
                           files: ["fallback.ttf", "ja.ttf", "zh.ttf"]) ],
             )
             Loader resolves `files` via load_context.load::<TranslationFile>(sibling path)
             → deferred dep handles → manifest signals LoadedWithDependencies when all
             locale files are in. wasm-safe (no dir listing anywhere).
    TranslationFile (Asset)
      ← was rust_i18n::i18n! compile-time embed
      Action: write fresh
      Notes: HashMap<locale, HashMap<flat_key, String>> for ONE file.
             Loader extensions: ["json", "yml", "yaml", "toml"]; typed loads via
             load_context.load::<TranslationFile> dodge ".json" loader collisions
             (typed dispatch filters by asset type — bevy-0.19-asset-facts §1).

  parse.rs                                     ← NEW, no old equivalent
    ← replaces rust-i18n-support parsing (behavior port, see research/rust-i18n-replication-spec.md)
    Action: write fresh (pure functions, unit-testable without App)
    Notes: v1/v2 detection (_version == 2 by u64), v1 locale-from-file-stem
           (split('.').last()), v2 string-vs-map disambiguation with dot-joined
           nested keys, flatten with leaf coercion (Null→"", Bool→"true"/"false",
           Number→Display, Array→""), deep merge (scalar: later wins, maps: recurse).
           CONSCIOUS DIVERGENCES (documented): (1) strip "_version" key in v1 files
           (rust-i18n leaks it as a translation); (2) parse failure = log error + skip
           file (rust-i18n: compile-time panic); (3) merge order = manifest file order,
           deterministic (rust-i18n: unspecified glob order).

  interpolate.rs (or fold into parse.rs)       ← NEW
    ← was rust_i18n::replace_patterns
    Action: rewrite
    Notes: adjacent-only %{name} state machine. Semantics kept: first-match-wins on
           duplicate names, unmatched pattern stays verbatim incl. '%', no escaping,
           unclosed pattern → input untouched. CONSCIOUS DIVERGENCE: the
           "% anywhere before {" quirk (spec §3.1) is NOT replicated — adjacent %{ only.

  resources.rs
    I18n (Resource)
      ← was I18n mirroring rust-i18n's global AtomicStr
      Action: rewrite — becomes the SINGLE source of truth
      Notes: fields: current locale, merged translation table
             (HashMap<locale, HashMap<key, String>>, #[reflect(ignore)]),
             available locales (sorted byte-lexicographic — parity),
             fallback list from manifest, ready flag.
             pub fn translate(&self, locale, key) -> Option<&str>: exact → BCP-47
             truncation chain (rfind('-') + trim "-x" tails) → explicit fallback list
             → None; caller echoes key verbatim on None + warn! once.
             set_locale/current/locales keep signatures (set_locale drops the
             rust_i18n::set_locale forward). Default::default() → current = "en".
             Table writes go through ResMut → change detection fires update systems
             for free (locale switch and hot reload share one reactive path).
    FontFolder
      ← was resources.rs FontFolder
      Action: keep (BCP-47 truncation logic shared with I18n::translate)
    FontManager
      Action: keep

  plugin.rs
    I18nPlugin
      ← was unit struct
      Action: port + redesign
      Notes: gains config: manifest asset path (Default = "locales/i18n.ron").
             Startup system loads manifest handle into private resource.
    sync_store (system)                        ← NEW
      Action: write fresh
      Notes: MessageReader<AssetEvent<TranslationFile>> + <AssetEvent<I18nManifest>>
             (0.19: Message/MessageReader, NOT EventReader). On any relevant
             Added/Modified/LoadedWithDependencies → rebuild I18n table from
             Assets<TranslationFile> in manifest order; flip ready on manifest
             LoadedWithDependencies. Full rebuild each time (tables are tiny —
             ponytail: no incremental diffing).
    load_dynamic_fonts (system)
      ← was PreStartup iteration over build-generated FONT_FAMILIES const
      Action: port + redesign — runs when manifest asset arrives; reads manifest
             fonts section; FontManager population identical to today.
    update_translations::<T> (system)
      ← was update_translations
      Action: port (shape unchanged — change-detection loop already idiomatic)
      Notes: now takes translations from Res<I18n> and passes &I18n into trait calls.
             Ordering: .after(sync_store) or chain in one Update set.
    I18nComponentRegistration::register_i18n_component::<T>
      Action: keep (public API unchanged)

  components/mod.rs — I18nComponent trait
    ← was fn locale(&self) -> String / fn translate(&self) -> String  (read globals)
    Action: port + redesign (BREAKING for third-party impls)
    Notes: fn locale<'a>(&'a self, i18n: &'a I18n) -> &'a str  (entity override or i18n.current())
           fn translate(&self, i18n: &I18n) -> String
           Only signature changes; component structs untouched.

  components/{i18n_text.rs, i18n_text_2d.rs, i18n_number.rs, i18n_font.rs}
    Action: keep — public builder API FROZEN (new/with_locale/with_arg/with_num_arg).
    Notes: internals swap rust_i18n::locale() for the passed &I18n. I18nFont unchanged.

  components/utils.rs
    translate_by_key
      ← was t!(key, locale) + rust_i18n::replace_patterns
      Action: rewrite (3 lines change): i18n.translate(...) + interpolate::apply(...)
    f64_to_fd / get_formatter
      Action: keep (icu rename fallout lands in the separate icu-bump commit)

tests/translation.rs
  Action: port + extend
  Notes: helper pumps app.update() until i18n.ready() (asset IO is async now).
         ADD parity vectors from research/rust-i18n-replication-spec.md §7:
         truncation (zh-TW-whatever→zh-TW, en-US→en), miss→key-echo ("does.not.exist"),
         no-default-fallback ("text2d" for ja → "text2d"), merge order (v2_example.yml
         overrides en.json: hello/en == "Hello world"), interpolation edge cases
         (unmatched %{b} stays, duplicate name first-wins). ADD hot-reload test if
         feasible natively (write temp asset, assert Modified propagates).

assets/locales/i18n.ron                        ← NEW (also web/dist copy via Trunk)
  Action: write fresh
  Notes: files: ["en.json", "ja.json", "zh-TW.json", "v2_example.yml"] — this exact
         order preserves today's test-observed merge results.
         fonts: NotoSans entry replacing build-time discovery.

README.md
  Action: rewrite sections: Project Status (goal achieved), File Structure (+ manifest),
         DELETE "Asset Path & Workspace Projects" section (wart gone), add Migration
         Guide 0.3 → 0.4 (add manifest file; %{} files unchanged; trait signature note),
         add Hot Reload section (file_watcher feature; not on wasm).

## Dropped (not in new codebase)

- build.rs — replaced by manifest + loaders
- BEVY_ASSET_PATH env-var protocol — obsolete (runtime assets resolve like every other Bevy asset)
- rust-i18n, rust-i18n-support, rust-i18n-macro, cargo-emit deps
- Compile-time locale baking on wasm (locales now fetched at runtime like all Bevy assets)

## Deliberately NOT in this refactor (follow-ups, roughly priority-ordered)

1. icu 2.x bump — same release, separate commit (mechanical renames).
2. System-locale auto-detect (bevy_device_lang 0.6 — bevy-independent, Android/iOS-correct;
   or sys-locale + js feature) — 0.4.x feature flag.
3. CLDR plural sub-keys via icu_plurals 2.x (bevy-intl-style optional plural maps) — 0.5 candidate.
4. embedded_asset! default-locale fallback shipped inside crate — only if users ask.
5. Fluent/MF2 — non-goal; bevy_fluent owns that niche, MF2 has no production Rust impl (2026-07).
