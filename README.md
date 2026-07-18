# bevy_simple_i18n

[![crates.io](https://img.shields.io/crates/v/bevy_simple_i18n)](https://crates.io/crates/bevy_simple_i18n)
[![license](https://img.shields.io/crates/l/bevy_simple_i18n)](https://github.com/TurtIeSocks/bevy_simple_i18n#license)

An opinionated but dead simple internationalization library for the Bevy game engine.

## Project Status

As of 0.4, this crate is fully native Bevy: locale files are regular Bevy **assets**
loaded at runtime through the `AssetServer`, locale state lives in an ECS resource, and
translations **hot reload**. There is no build script, no compile-time embedding, and no
global state. (Earlier versions wrapped the `rust-i18n` crate, which baked every locale
file into the binary at compile time — see the [migration guide](#migrating-from-03)
below.)

## [Demo](https://turtiesocks.github.io/bevy_simple_i18n/)

## Usage

### CLI

```sh
cargo add bevy_simple_i18n
```

### main.rs

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(I18nPlugin::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((I18nText::new("hello"), I18nFont::new("NotoSans")));
    commands.spawn((I18nNumber::new(2503.10), I18nFont::new("NotoSans")));
}
```

### The manifest

The plugin loads a small RON manifest (default: `assets/locales/i18n.ron`) that declares
your locale files and dynamic font families. It exists because wasm and Android builds
cannot list asset directories — so the manifest is the one place that says what to load:

```ron
(
    // Optional. The starting locale (defaults to "en"). Ignored after you call
    // `I18n::set_locale` yourself.
    default_locale: "en",

    // Optional. Locales to try when a key is missing everywhere else.
    // Empty by default: a missing key renders the key itself.
    fallback: [],

    // Locale files, relative to this manifest's directory.
    // LIST ORDER IS MERGE ORDER: later files win on conflicting keys.
    files: [
        "en.json",
        "ja.json",
        "v2_example.yml",
    ],

    // Dynamic font families. `dir` is relative to the asset root; the file stem is
    // the locale it serves ("fallback" marks the family's fallback font).
    fonts: [
        (
            family: "NotoSans",
            dir: "fonts/NotoSans",
            files: ["fallback.ttf", "ja.ttf", "zh-TW.ttf"],
        ),
    ],
)
```

Custom manifest location: `I18nPlugin::with_manifest("i18n/manifest.ron")`. Manifest
paths are **asset paths, relative to the asset root** — for a manifest on disk at
`assets/locales/i18n.ron`, the asset path is `locales/i18n.ron` (omit the `assets/`
prefix).

## File Structure

```ts
.
├── assets
│   ├── locales
│   │   ├── i18n.ron            // the manifest
│   │   ├── {locale_file}.json
│   │   ├── {locale_file}.yml   // feature "yaml" (default on)
│   │   └── {locale_file}.toml  // feature "toml" (default on)
│   └── fonts
│       └── {font_name}
│           ├── fallback.ttf
│           └── {locale}.ttf
└── Cargo.toml
```

## Locale Files

Both classic formats are supported, in JSON, YAML and TOML:

**v1** — one file per locale; the file stem is the locale (`en.json`, `app.ja.yml`):

```json
{
  "hello": "Hello World",
  "messages.hello": "Hello, %{name}"
}
```

**v2** — one file for many locales, marked by `_version: 2`:

```yml
_version: 2
hello:
  en: Hello world
  zh-TW: 你好世界
  ja: こんにちは世界
```

Nested maps flatten to dot keys (`a: { b: x }` == `"a.b": x`). If several files define
the same key for the same locale, the file listed **later in the manifest wins**.

## Features

### Text Translations

```rust
commands.spawn(I18nText::new("hello"));
```

### Number Localization

```rust
commands.spawn(I18nNumber::new(2350.54));
```

### Interpolation

```rust
commands.spawn(I18nText::new("messages.hello").with_arg("name", "world"));
commands.spawn(I18nText::new("messages.cats").with_num_arg("count", 20));
```

### Dynamic Fonts

Declare a font family in the manifest (see above), then:

```rust
commands.spawn((I18nText::new("hello"), I18nFont::new("NotoSans")));
```

When the locale is `ja`, `ja.ttf` is used. A locale without its own file walks up the
locale chain (`zh-TW` → `zh`) and finally lands on `fallback.ttf`.

### Automatic Text Re-Rendering

Change the locale on the [`I18n`] resource and every i18n entity re-renders — no
boilerplate:

```rust
fn change_locale(mut i18n: ResMut<I18n>) {
    i18n.set_locale("zh-TW");
}
```

Locale resolution on lookup follows the BCP-47 truncation chain: `zh-Hant-CN` tries
`zh-Hant-CN`, then `zh-Hant`, then `zh`, then the manifest's `fallback` locales. A
complete miss renders the key itself (and logs a warning), so untranslated text is
visible instead of invisible.

### Locale Auto-Detection

With the `detect` feature (default on), the system/device locale is detected at
startup — on desktop, iOS, Android and wasm (via `bevy_device_lang`). It is used as
the starting locale **only if your game ships it** (or a parent of it: a `de-AT`
device with a `de` locale file uses `de-AT`); otherwise the manifest's
`default_locale` applies. Calling `I18n::set_locale` (e.g. restoring the user's
saved choice) always wins over detection. The raw detected tag is available via
`I18n::detected()`.

### Hot Reload

Enable Bevy's `file_watcher` cargo feature and edit a locale file while the game runs —
all live text re-translates instantly. Great for translators: no recompile, no restart.
(Not available on wasm, where Bevy has no file watcher.)

You can also edit translations from code (in-game translation tools, downloaded
language packs):

```rust
fn tweak(mut files: ResMut<Assets<TranslationFile>>) {
    for (_, file) in files.iter_mut() {
        file.set_translation("en", "hello", "Hi there!");
    }
}
```

### Load Readiness

Assets load asynchronously. Text spawned before the translations arrive renders its key
and self-heals once loading completes. If you want a loading screen instead, gate on
[`I18n::ready`]:

```rust
fn loading_screen_done(i18n: Res<I18n>) -> bool {
    i18n.ready()
}
```

## Traits

### `I18nComponent`

Implement this for your own component to drive any `String`-carrying text component
from a translation key. Both methods receive the [`I18n`] resource:

```rust
impl I18nComponent for MyLabel {
    type Target = Text;
    fn locale<'a>(&'a self, i18n: &'a I18n) -> &'a str {
        self.locale.as_deref().unwrap_or_else(|| i18n.current())
    }
    fn translate(&self, i18n: &I18n) -> String {
        i18n.translate(self.locale(i18n), &self.key).unwrap_or(&self.key).to_string()
    }
}
```

### `I18nComponentRegistration`

Registers your component for automatic re-translation (and dynamic font support):

```rust
app.register_i18n_component::<MyLabel>();
```

## Migrating from 0.3

1. Add a manifest at `assets/locales/i18n.ron` listing your locale files (see
   [The manifest](#the-manifest)). Your locale files themselves need no changes.
2. `I18nPlugin` → `I18nPlugin::default()`.
3. Delete any `BEVY_ASSET_PATH` setup — it no longer exists. Workspace projects need no
   special configuration anymore.
4. If you implemented `I18nComponent` yourself, both methods changed signature:
   `fn locale(&self) -> String` is now `fn locale<'a>(&'a self, i18n: &'a I18n) -> &'a str`,
   and `fn translate(&self) -> String` is now `fn translate(&self, i18n: &I18n) -> String`.
5. Note: locale files merge in **manifest order** (deterministic). Previously the merge
   order across files was filesystem-dependent; if you relied on a specific override
   order, encode it in the manifest's `files` list.

## Cargo Features

| Feature   | Default | Effect                                        |
| --------- | ------- | --------------------------------------------- |
| `numbers` | yes     | `I18nNumber` + `with_num_arg` (icu4x)         |
| `yaml`    | yes     | `.yml` / `.yaml` locale files                 |
| `toml`    | yes     | `.toml` locale files                          |
| `detect`  | yes     | system-locale auto-detect (`bevy_device_lang`) |

## Bevy support table

| bevy | bevy_simple_i18n |
| ---- | ---------------- |
| 0.19 | 0.3, 0.4         |
| 0.15 | 0.1              |

## Credits

- [Fonts](https://fonts.google.com/noto/fonts)
