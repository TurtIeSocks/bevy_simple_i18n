# rust-i18n v3.1.5 Replication Spec (for native Bevy i18n, behavior parity)

Source read: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/{rust-i18n-3.1.5,rust-i18n-macro-3.1.5,rust-i18n-support-3.1.5}`. Crate surface confirmed in worktree: `build.rs` (emits `rust_i18n::i18n!("<abs assets dir>")`), `src/components/utils.rs:49-51` (`t!(key, locale = locale)` + `replace_patterns`), `src/resources.rs:29,46-47` (`set_locale`/`locale()`/`available_locales!`).

## Effective config TODAY (what bevy_simple_i18n actually gets)

`i18n!(path)` with no options; macro also always loads `[package.metadata.i18n]` from the compiling crate's Cargo.toml (macro/src/lib.rs:125-146, 191 — note: no `metadata=false` branch exists in 3.1.5 despite docs; unknown options silently ignored, macro/src/lib.rs:113). bevy_simple_i18n's Cargo.toml has no i18n metadata → `I18nConfig::default()` (support/src/config.rs:34-47): `default_locale="en"`, `fallback=[]` → **`_RUST_I18N_FALLBACK_LOCALE = None`**, `minify_key=false` (so minify_key machinery is entirely inert — key used verbatim). The generated `default_locale` block (macro/src/lib.rs:284-295) is a no-op (sets locale to itself).

## 1. File discovery + formats

1.1 [MUST] Scan glob `{dir}/**/*.{yml,yaml,json,toml}` — recursive, exactly those 4 extensions (support/src/lib.rs:68). Since the crate points it at the whole game `assets/` dir, ANY matching file anywhere under assets is treated as a locale file (fonts `.ttf/.otf` ignored by glob).
1.2 [MUST] Missing dir → empty translation set, no error (support/src/lib.rs:75-80). bevy build.rs relies on this ("locales" dummy path when no assets folder, build.rs:25-28).
1.3 [MUST] Parse: yml/yaml via serde_yaml→`serde_json::Value`, json via serde_json, toml via `toml::from_str::<serde_json::Value>` (support/src/lib.rs:127-136). Parse failure = panic at compile time ("Parse file `{path}` failed") — runtime port should log error + skip instead.
1.4 [MUST] Version detect: root key `_version`, `as_u64()`, value `2` → v2 format; anything else (absent, non-int, 1, 3…) → v1 (support/src/lib.rs:239-245).
1.5 [MUST] **v1**: whole file = translations for ONE locale; locale = `file_stem().split('.').last()` — `en.json`→`en`, `app.en.yml`→`en`, `foo.yml`→locale `foo` (support/src/lib.rs:92-96). Directory names irrelevant.
1.6 [MUST-QUIRK] v1 does NOT strip `_version` — `en.json` containing `"_version": 1` yields flat key `_version`→`"1"` in locale `en` (observable: `t!("_version", locale="en")` == `"1"`). Repo's en/ja/zh-TW.json all carry `_version: 1` today. Recommend dropping key in new impl; divergence is harmless but note it.
1.7 [MUST] **v2** (`_version: 2`): root object; each entry `key: {locale: text}`; string value = locale→text pair; object value = nested key, recursed with dot-joined prefix (`format_keys` joins non-empty segments with `.`, support/src/lib.rs:189-254). Locale-vs-nested-key disambiguation is purely by value type (string vs map). Filename irrelevant for v2. v2 file yielding zero entries → error "Invalid locale file format, please check the version field".
1.8 [MUST] Key flattening (v1): nested objects flatten to dot keys (`a: {b: c}` → `a.b`); keys already containing literal dots stay verbatim (`"messages.hello"` key in JSON = flat key `messages.hello`) — both forms identical after flatten (support/src/lib.rs:256-289).
1.9 [MUST] Leaf coercion at flatten: String→as-is; Null→`""`; Bool→`"true"`/`"false"`; Number→`format!("{}", n)` (serde_json Display, e.g. `1`→`"1"`, `1.5`→`"1.5"`); **Array→`""`** (support/src/lib.rs:274-286).
1.10 [MUST] Multi-file merge: per-locale JSON deep merge in glob iteration order; scalar leaves = **later file wins**, objects merge recursively (`merge_value`, support/src/lib.rs:29-40, 111-116). Order itself is unspecified (globwalk/walkdir readdir order) — but repo tests DEPEND on `v2_example.yml` overriding `en.json`/`ja.json`: `tests/translation.rs` asserts `hello`(en)==`"Hello world"` (v2 value; en.json has `"Hello World"`), `hello`(ja)==`"こんにちは世界"` (ja.json has `"こんにちは"`), `messages.hello`(en)==`"Hello, %{name}"` (en.json has `"Hello %{name}"`). New impl MUST define deterministic order where v2_example.yml loads after {en,ja}.json — lexicographic-by-path ascending satisfies this and is the safe choice.
1.11 [NICE] `RUST_I18N_DEBUG=1` prints load diagnostics (compile-time only).

## 2. t! lookup + missing-key + fallback

t! expansion (macro/src/tr.rs:433-465): `locale` is filtered out of args; bevy calls `t!(key, locale = locale)` with NO interpolation args → args-empty branch.

2.1 [MUST] Lookup order for `translate(locale, key)` (generated `_rust_i18n_try_translate`, macro/src/lib.rs:373-389):
   a. exact `(locale, key)`;
   b. locale-truncation chain: repeatedly `locale.rfind('-').map(|n| locale[..n].trim_end_matches("-x"))` — `zh-TW`→`zh`; `zh-Hant-CN`→`zh-Hant`→`zh`; `-x` private-use tails trimmed (`aa-x-b`→ truncate to `aa-x` → trim → `aa`). Stops when no `-` left.
   c. explicit fallback-locale list — **None today** → skipped. So **NO fallback to default/"en"**: key present only in `en` but requested with `locale="ja"` → miss.
2.2 [MUST] Miss (all lookups fail) → t! returns the **key string verbatim** (`Cow::Borrowed(key)`) — e.g. `t!("nope", locale="en")` == `"nope"`. NOT `"en.nope"` — the `format!("{locale}.{key}")` function `_rust_i18n_translate` (macro/src/lib.rs:359-367) is generated but dead on the t! path in 3.1.5.
2.3 [MUST] Lookup is exact string match on flattened key + exact locale string (case-sensitive, no normalization). Backend = `HashMap<locale, HashMap<key, value>>` (support/src/backend.rs:93-99).
2.4 [NICE] `i18n!(path, fallback = "en")` / `fallback = ["en","es"]` would add step (c): try each listed locale in order. Crate never sets it. (Also `[package.metadata.i18n] fallback=[...]` in the *consuming game's* Cargo.toml would NOT apply — macro reads the Cargo.toml of bevy_simple_i18n itself, where i18n! expands.)
2.5 [NICE] `log-miss-tr` cargo feature logs misses via `log` crate — off today.
2.6 [NICE] t!'s own arg interpolation + format specifiers (`sn = 123 : {:08}`) — unused; crate interpolates via `replace_patterns` separately.

## 3. replace_patterns exact semantics

`pub fn replace_patterns(input: &str, patterns: &[&str], values: &[String]) -> String` (rust-i18n-3.1.5/src/lib.rs:46-92). Byte-level state machine, NOT plain substring replace:

3.1 [MUST] States: `%` (at ANY point, even inside a pending pattern) → stage 1; `{` while stage 1 → record start; `}` while stage 2 → record end; all other bytes leave state unchanged. Consequence: `%` does NOT need to be adjacent to `{` — `"% foo {a} bar"` with `a=1` → `"% foo1 bar"` (the byte immediately before `{` is consumed as if it were `%`). Replicate or consciously diverge (adjacent-only `%{name}` matches all real locale files; the non-adjacent case is a quirk no test exercises — tag the quirk itself [NICE]).
3.2 [MUST] Match: pattern name = bytes between `{`/`}`; linear search `patterns.iter().zip(values)` — first match wins on duplicate names; zip truncates to shorter slice if lengths differ.
3.3 [MUST] Matched → emit value; the char before `{` (the `%`) is dropped. `"Hello, %{name}!"` + `("name", "world")` → `"Hello, world!"`.
3.4 [MUST] Unmatched name → pattern emitted verbatim INCLUDING preceding byte: `"Hi %{name}"` with no args → `"Hi %{name}"` unchanged. (This is what a locale string with a placeholder-but-no-arg renders as.)
3.5 [MUST] No escaping mechanism whatsoever — cannot render literal `%{...}` if a matching pattern is supplied. `{a}` without any prior `%` in the string → untouched. Unclosed `%{name` → whole input untouched (odd position list, `chunks_exact(2)` drops remainder).
3.6 [MUST] UTF-8 safe by construction (`%{}`are ASCII; splits only at ASCII positions); output built as bytes + `from_utf8_unchecked`.
3.7 [MUST] Crate call site passes t! output (possibly the miss-fallback key) — interpolation applies even to missed keys (utils.rs:49-51).

## 4. set_locale / locale()

4.1 [MUST] Global state, initial value `"en"` (`static CURRENT_LOCALE: Lazy<AtomicStr> = AtomicStr::from("en")`, rust-i18n-3.1.5/src/lib.rs:16). `AtomicStr` = `arc_swap::ArcSwapAny<triomphe::Arc<String>>` (support/src/atomic_str.rs). No validation, no normalization (bevy layer validates via `icu_locid` before calling, resources.rs:25-28).
4.2 [MUST→translate] In Bevy port this becomes the `I18n` resource's `current` field; parity requirements: default `"en"`, `I18n::default().current()` == `"en"` before any set. Components without explicit locale resolve against it (i18n_text.rs:59, i18n_text_2d.rs:56, i18n_number.rs:44).

## 5. available_locales!()

5.1 [MUST] = set of locale keys present in merged translation data (HashMap keys → inherently deduped), **sorted ascending byte-lexicographic** (`Vec<&str>.sort()`; sorted twice — backend.rs:89 + macro/src/lib.rs:396). Uppercase before lowercase: `zh-TW` sorts after `uk` etc. Locales from BOTH v1 filenames and v2 body keys count. Today's repo set ≈ `["cs","da","de","en","es","fi","fr","hu","it","ja","ko","nl","no","pl","pt","ru","sl","sv","th","tr","uk","zh-TW", …]` (all v2_example.yml locales + en/ja/zh-TW).
5.2 [MUST] A locale existing with ANY key (even just `_version`) counts as available.

## 6. i18n! codegen — anything else that matters

6.1 [MUST] Path resolution: `CARGO_MANIFEST_DIR.join(arg)` — bevy build.rs passes absolute assets path so join is identity; runtime port replaces with Bevy asset paths (`assets/**`).
6.2 [NICE] `minify_key` family (defaults: off, len 24, prefix `""`, thresh 127; siphash13+base62 of long literal messages) — entirely unused (off).
6.3 [NICE] `backend = expr` extension (CombinedBackend, later backend shadows earlier) — unused.
6.4 [NICE] `tkv!` macro — unused.
6.5 [MUST] Data model to replicate: `HashMap<locale, HashMap<flat_key, String>>`; lookups are O(1) per (locale,key); truncation-fallback chain computed per call, uncached.

## Parity test vectors (assert these against the new impl, current assets/)

- `t("hello", "en")` → `"Hello world"`; `t("hello", "ja")` → `"こんにちは世界"` (merge order!); `t("hello", "zh-TW")` → `"你好世界"`.
- `t("messages.hello","en")` + `{name:"Bevy"}` → `"Hello, Bevy"`.
- Truncation: `t("hello", "zh-TW-whatever")` → resolves via `zh-TW`; `t("hello","en-US")` → `"Hello world"` via `en`.
- Miss: `t("does.not.exist", "en")` → `"does.not.exist"`; `t("hello", "xx")` → `"hello"` (no default-locale fallback).
- `t("text2d", "ja")` → `"text2d"` (key only in en.json → miss for ja; no fallback).
- `replace_patterns("Hi %{a} %{a}", ["a","a"], ["1","2"])` → `"Hi 1 1"`; `replace_patterns("Hi %{b}", ["a"], ["1"])` → `"Hi %{b}"`.
- `available_locales` sorted, contains `"zh-TW"` and every v2 locale; `I18n::default().current()` == `"en"`.