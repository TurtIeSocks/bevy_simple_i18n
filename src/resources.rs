use bevy::{ecs::reflect::ReflectResource, platform::collections::HashMap, prelude::*, text::Font};
use icu_locid::Locale;

/// Resource for managing the current locale and getting the available locales
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
    locales: Vec<String>,
    current: String,
}

impl I18n {
    pub fn set_locale(&mut self, locale: impl Into<String>) {
        let next_locale: String = locale.into();
        if let Err(err) = next_locale.parse::<Locale>() {
            bevy::log::error!("Invalid locale: {}", err);
            return;
        }
        rust_i18n::set_locale(&next_locale);
        bevy::log::debug!("Locale changed from {} to {}", self.current, next_locale);
        self.current = next_locale;
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn locales(&self) -> &[String] {
        &self.locales
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self {
            current: rust_i18n::locale().to_string(),
            locales: rust_i18n::available_locales!()
                .into_iter()
                .map(|s| s.into())
                .collect(),
        }
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
