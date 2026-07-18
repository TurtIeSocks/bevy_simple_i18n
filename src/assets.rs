//! The two asset types that replace the old compile-time embedding:
//!
//! - [`TranslationFile`]: one parsed locale file (any of the supported formats).
//! - [`I18nManifest`]: the RON manifest that declares which locale files and font
//!   families exist. It is the runtime answer to "which files should I load?" —
//!   required because wasm (and Android) cannot enumerate asset directories.

use std::path::{Path, PathBuf};

use bevy::{
    asset::{io::AssetSourceId, io::Reader, AssetLoader, AssetPath, LoadContext},
    prelude::*,
};

use crate::parse;

/// A single parsed locale file: `locale -> (flat key -> text)`.
///
/// Produced by [`TranslationFileLoader`] from `.json`, `.yml`/`.yaml` (feature
/// `yaml`) and `.toml` (feature `toml`) files, in both rust-i18n file formats
/// (v1 per-locale files and v2 `_version: 2` multi-locale files).
#[derive(Asset, TypePath, Debug)]
pub struct TranslationFile {
    pub(crate) table: parse::Table,
}

impl TranslationFile {
    /// Sets or overrides a single translation at runtime.
    ///
    /// Mutating a loaded asset through [`Assets::get_mut`](bevy::asset::Assets::get_mut)
    /// emits [`AssetEvent::Modified`](bevy::asset::AssetEvent), so live text
    /// re-translates immediately — the same path a `file_watcher` hot reload takes.
    pub fn set_translation(
        &mut self,
        locale: impl Into<String>,
        key: impl Into<String>,
        text: impl Into<String>,
    ) {
        self.table
            .entry(locale.into())
            .or_default()
            .insert(key.into(), text.into());
    }
}

/// The i18n manifest asset, loaded from RON (default path: `locales/i18n.ron`).
///
/// ```ron
/// (
///     default_locale: "en",       // optional, defaults to "en"
///     fallback: [],               // optional explicit fallback locales
///     files: [                    // paths relative to the manifest's directory;
///         "en.json",              // LIST ORDER IS MERGE ORDER — later files win
///         "extra.yml",
///     ],
///     fonts: [                    // dynamic font families; `dir` is relative to
///         (                       // the asset root
///             family: "NotoSans",
///             dir: "fonts/NotoSans",
///             files: ["fallback.ttf", "ja.ttf"],
///         ),
///     ],
/// )
/// ```
#[derive(Asset, TypePath, Debug)]
pub struct I18nManifest {
    pub(crate) default_locale: String,
    pub(crate) fallback: Vec<String>,
    /// Strong handles to the declared translation files, in manifest order.
    /// These are load-context dependencies: the manifest asset reports
    /// `LoadedWithDependencies` only once every translation file is in.
    pub(crate) files: Vec<Handle<TranslationFile>>,
    pub(crate) fonts: Vec<FontFamilyEntry>,
    /// The asset source the manifest was loaded from (`assets/`, `embedded://`, …).
    /// Font paths resolve against this source so a manifest from a custom source
    /// stays self-contained.
    pub(crate) source: AssetSourceId<'static>,
}

/// One `fonts:` entry of the manifest.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FontFamilyEntry {
    pub family: String,
    /// Directory relative to the asset root, e.g. `fonts/NotoSans`.
    pub dir: String,
    /// Font files inside `dir`. The stem is the locale (`ja.ttf` serves `ja`);
    /// the stem `fallback` marks the family's fallback font.
    pub files: Vec<String>,
}

/// Serde shape of the manifest RON file.
#[derive(serde::Deserialize)]
struct ManifestRon {
    #[serde(default = "default_locale_en")]
    default_locale: String,
    #[serde(default)]
    fallback: Vec<String>,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    fonts: Vec<FontFamilyEntry>,
}

fn default_locale_en() -> String {
    "en".to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum I18nLoaderError {
    #[error("failed to read locale asset: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Parse(#[from] parse::ParseError),
    #[error("failed to parse i18n manifest RON: {0}")]
    Ron(#[from] ron::error::SpannedError),
}

#[derive(Default, TypePath)]
pub struct TranslationFileLoader;

impl AssetLoader for TranslationFileLoader {
    type Asset = TranslationFile;
    type Settings = ();
    type Error = I18nLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let path = load_context.path().path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let table = parse::parse_file(stem, ext, &bytes)?;
        Ok(TranslationFile { table })
    }

    // No `extensions()` override, deliberately. This crate only ever loads
    // translation files TYPED (`load::<TranslationFile>`), which selects the loader
    // by asset type. Claiming bare "json"/"yml"/"toml" would hijack other crates'
    // UNTYPED loads (`load_untyped`, `load_folder`) for those extensions, where
    // bevy picks the last-registered loader with no warning.
}

#[derive(Default, TypePath)]
pub struct I18nManifestLoader;

impl AssetLoader for I18nManifestLoader {
    type Asset = I18nManifest;
    type Settings = ();
    type Error = I18nLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let manifest: ManifestRon = ron::de::from_bytes(&bytes)?;

        // `files` entries are relative to the manifest's own directory, and stay on
        // the manifest's asset source (`embedded://locales/i18n.ron` pulls its files
        // from `embedded://locales/`, not from the default `assets/` root).
        let source = load_context.path().source().clone_owned();
        let dir: PathBuf = load_context
            .path()
            .path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let files = manifest
            .files
            .iter()
            .map(|file| {
                let path = AssetPath::from(dir.join(file)).with_source(source.clone());
                load_context.load::<TranslationFile>(path)
            })
            .collect();

        Ok(I18nManifest {
            default_locale: manifest.default_locale,
            fallback: manifest.fallback,
            files,
            fonts: manifest.fonts,
            source,
        })
    }

    // No `extensions()` override — same reasoning as TranslationFileLoader: the
    // manifest is always loaded typed, and claiming bare "ron" would hijack other
    // crates' untyped .ron loads (scenes, bevy_common_assets, …).
}
