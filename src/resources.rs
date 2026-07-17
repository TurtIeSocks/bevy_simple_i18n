use bevy::{ecs::reflect::ReflectResource, platform::collections::HashMap, prelude::*, text::Font};
use icu_locale_core::Locale;

use crate::parse::Table;

/// Resource for managing the current locale and looking up translations.
///
/// This is the single source of truth: the current locale, the merged translation
/// table (built from the loaded [`TranslationFile`](crate::prelude::TranslationFile)
/// assets), the available locales and the fallback chain all live here. Mutating it
/// (switching locale, or the sync system rebuilding the table after an asset load /
/// hot reload) triggers change detection, which re-translates every registered i18n
/// component.
///
/// # Example
/// ```
/// use bevy::prelude::*;
/// use bevy_simple_i18n::prelude::*;
///
/// fn update_locale(mut i18n_res: ResMut<I18n>) {
///     i18n_res.set_locale("en");
/// }
/// ```
#[derive(Debug, Resource, Reflect)]
#[reflect(Resource)]
pub struct I18n {
    /// Currently active locale.
    current: String,
    /// Locales present in the merged table, sorted ascending (byte-lexicographic).
    locales: Vec<String>,
    /// `locale -> (flat key -> text)`, merged in manifest order (later files win).
    #[reflect(ignore)]
    translations: Table,
    /// Explicit fallback locales from the manifest, tried in order after the
    /// requested locale's truncation chain. Empty by default (rust-i18n parity:
    /// a miss echoes the key, it does NOT silently fall back to the default locale).
    #[reflect(ignore)]
    fallback: Vec<String>,
    /// True once the manifest and all its translation files finished loading.
    ready: bool,
    /// Set when the user called [`set_locale`](Self::set_locale); prevents the
    /// manifest's `default_locale` from overriding an explicit choice.
    #[reflect(ignore)]
    explicit: bool,
}

impl Default for I18n {
    fn default() -> Self {
        Self {
            current: "en".to_string(),
            locales: Vec::new(),
            translations: Table::default(),
            fallback: Vec::new(),
            ready: false,
            explicit: false,
        }
    }
}

impl I18n {
    /// Sets the active locale. Invalid BCP-47 identifiers are rejected with an error
    /// log. Every registered i18n component re-translates automatically.
    pub fn set_locale(&mut self, locale: impl Into<String>) {
        let next_locale: String = locale.into();
        if let Err(err) = next_locale.parse::<Locale>() {
            bevy::log::error!("Invalid locale: {}", err);
            return;
        }
        bevy::log::debug!("Locale changed from {} to {}", self.current, next_locale);
        self.current = next_locale;
        self.explicit = true;
    }

    /// The currently active locale.
    pub fn current(&self) -> &str {
        &self.current
    }

    /// All locales present in the loaded translation files, sorted ascending.
    pub fn locales(&self) -> &[String] {
        &self.locales
    }

    /// True once the manifest and all of its translation files have loaded.
    ///
    /// Components spawned earlier self-heal: they render their key first and
    /// re-translate when the table arrives.
    pub fn ready(&self) -> bool {
        self.ready
    }

    /// Looks up `key` for `locale`.
    ///
    /// Resolution order (rust-i18n parity): exact locale, then the BCP-47
    /// truncation chain (`zh-Hant-CN` → `zh-Hant` → `zh`, trimming `-x`
    /// private-use tails), then the explicit fallback locales. `None` on a
    /// complete miss — callers render the key verbatim.
    pub fn translate(&self, locale: &str, key: &str) -> Option<&str> {
        if let Some(text) = self.lookup(locale, key) {
            return Some(text);
        }
        let mut chain = locale;
        while let Some(index) = chain.rfind('-') {
            chain = chain[..index].trim_end_matches("-x");
            if let Some(text) = self.lookup(chain, key) {
                return Some(text);
            }
        }
        self.fallback
            .iter()
            .find_map(|fallback| self.lookup(fallback, key))
    }

    fn lookup(&self, locale: &str, key: &str) -> Option<&str> {
        self.translations.get(locale)?.get(key).map(String::as_str)
    }

    /// Replaces the translation table (called by the asset sync system).
    pub(crate) fn apply_table(
        &mut self,
        table: Table,
        default_locale: &str,
        fallback: Vec<String>,
        ready: bool,
    ) {
        let mut locales: Vec<String> = table.keys().cloned().collect();
        locales.sort();
        self.translations = table;
        self.locales = locales;
        self.fallback = fallback;
        self.ready = ready;
        if !self.explicit && !default_locale.is_empty() && self.current != default_locale {
            // Same validation as set_locale: a typo'd manifest default_locale must not
            // poison `current` (the numbers formatter panics on unparseable locales).
            if default_locale.parse::<Locale>().is_ok() {
                self.current = default_locale.to_string();
            } else {
                bevy::log::error!(
                    "Invalid `default_locale` in i18n manifest: {default_locale:?}; \
                     keeping {:?}",
                    self.current
                );
            }
        }
    }

    /// Latches readiness even when loading failed terminally (missing manifest),
    /// so `ready()`-gated loading screens don't hang forever.
    pub(crate) fn mark_ready(&mut self) {
        self.ready = true;
    }
}

/// Internal struct for managing fonts for a specific font family.
///
/// It attempts to find a specified font for the most specific locale.
///
/// If unsuccessful, it will split the locale at the last `-` and try again.
///
/// `en-US` -> `en` -> `fallback`
///
/// If still unsuccessful, it will return the fallback font.
#[derive(Debug, Default, Reflect)]
pub(crate) struct FontFolder {
    pub(crate) fallback: Handle<Font>,
    pub(crate) fonts: HashMap<String, Handle<Font>>,
}

impl FontFolder {
    pub(crate) fn get(&self, locale: impl Into<String>) -> Handle<Font> {
        let locale: String = locale.into();
        let mut locale = locale.as_str();

        bevy::log::debug!("Evaluating font for {} locale", locale);
        while !locale.is_empty() {
            if let Some(font) = self.fonts.get(locale) {
                bevy::log::debug!("Font for {} locale found", locale);
                return font.clone();
            }
            if let Some(index) = locale.rfind('-') {
                bevy::log::debug!("Font for {} locale was not found", locale);
                locale = &locale[..index];
            } else {
                break;
            }
        }

        bevy::log::debug!("Returning the fallback font");
        self.fallback.clone()
    }
}

/// Resource for managing fonts for different font families
#[derive(Debug, Reflect, Default, Resource)]
#[reflect(Resource)]
pub(crate) struct FontManager {
    pub(crate) fonts: HashMap<String, FontFolder>,
}

impl FontManager {
    pub(crate) fn insert(&mut self, family: impl Into<String>, font_folder: FontFolder) {
        let family: String = family.into();
        bevy::log::debug!("Font family {} added", family);
        self.fonts.insert(family, font_folder);
    }

    pub(crate) fn get(&self, family: &str, locale: impl Into<String>) -> Handle<Font> {
        if let Some(folder) = self.fonts.get(family) {
            bevy::log::debug!("Found font family: {}", family);
            folder.get(locale)
        } else {
            bevy::log::debug!("Font {} was not found, using default", family);
            Handle::<Font>::default()
        }
    }
}
