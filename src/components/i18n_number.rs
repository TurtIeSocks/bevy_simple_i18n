use bevy::prelude::*;
use fixed_decimal::Decimal;

use super::{utils, I18nComponent};

/// Component for spawning translatable number entities that are managed by `bevy_simple_i18n`
///
/// A Bevy [`Text`] component is inserted automatically (via `#[require(Text)]`) and kept in sync
/// with the localized number.
///
/// Updates automatically whenever the locale is changed using the [`crate::resources::I18n`] resource
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_simple_i18n::prelude::*;
/// # fn system(mut commands: Commands) {
/// // Basic usage
/// commands.spawn(I18nNumber::new(200.40));
///
/// // With forced locale
/// // overrides the global
/// // does not update when the locale is changed
/// commands.spawn(I18nNumber::new(12051).with_locale("ja"));
/// # }
/// ```
#[derive(Component, Default, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(Text)]
pub struct I18nNumber {
    /// `None` when constructed from a NaN/infinite value (logged at construction
    /// time); renders as an empty string rather than crashing the game.
    #[reflect(ignore)]
    pub(crate) fixed_decimal: Option<Decimal>,
    /// Locale for this specific translation, `None` to use the global locale
    pub(crate) locale: Option<String>,
}

impl I18nComponent for I18nNumber {
    type Target = Text;

    fn locale<'a>(&'a self, i18n: &'a crate::prelude::I18n) -> &'a str {
        self.locale.as_deref().unwrap_or_else(|| i18n.current())
    }

    fn translate(&self, i18n: &crate::prelude::I18n) -> String {
        let Some(fixed_decimal) = &self.fixed_decimal else {
            return String::new();
        };
        utils::get_formatter(self.locale(i18n), fixed_decimal)
            .map(|f| f.format_to_string(fixed_decimal))
            .unwrap_or_else(|| fixed_decimal.to_string())
    }
}

impl I18nNumber {
    /// Creates a new `I18nNumber` component with the provided number value.
    ///
    /// A NaN/infinite value is a data bug worth an error log, not a crash: it
    /// renders as an empty string instead.
    pub fn new(number: impl Into<f64>) -> Self {
        Self {
            fixed_decimal: utils::try_f64_to_fd(number.into()),
            locale: None,
        }
    }

    /// Set the locale for this specific translation.
    ///
    /// Underscore-separated tags (`en_US`) are normalized to hyphens before
    /// validation. An invalid locale is ignored (logged as an error) and the
    /// global locale is used instead.
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.set_locale(locale);
        self
    }

    /// Replaces the number value in place. A NaN/infinite value is a data bug
    /// worth an error log, not a crash: the previous value is left unchanged.
    pub fn set_number(&mut self, number: impl Into<f64>) {
        if let Some(fd) = utils::try_f64_to_fd(number.into()) {
            self.fixed_decimal = Some(fd);
        }
    }

    /// Set the locale for this specific translation, in place.
    ///
    /// Underscore-separated tags (`en_US`) are normalized to hyphens before
    /// validation. An invalid locale is ignored (logged as an error) and the
    /// previous locale is left unchanged.
    pub fn set_locale(&mut self, locale: impl Into<String>) {
        let raw: String = locale.into();
        let normalized = raw.replace('_', "-");
        match normalized.parse::<icu_locale_core::Locale>() {
            Ok(_) => self.locale = Some(normalized),
            Err(err) => bevy::log::error!("Ignoring invalid locale {raw:?}: {err}"),
        }
    }
}
