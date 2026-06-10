use bevy::prelude::*;
use fixed_decimal::FixedDecimal;

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
    #[reflect(ignore)]
    pub(crate) fixed_decimal: FixedDecimal,
    /// Locale for this specific translation, `None` to use the global locale
    pub(crate) locale: Option<String>,
}

impl I18nComponent for I18nNumber {
    type Target = Text;

    fn locale(&self) -> String {
        self.locale
            .clone()
            .unwrap_or_else(|| rust_i18n::locale().to_string())
    }

    fn translate(&self) -> String {
        utils::get_formatter(&self.locale(), &self.fixed_decimal)
            .format_to_string(&self.fixed_decimal)
    }
}

impl I18nNumber {
    /// Creates a new `I18nNumber` component with the provided number value
    pub fn new(number: impl Into<f64>) -> Self {
        Self {
            fixed_decimal: utils::f64_to_fd(number.into()),
            locale: None,
        }
    }

    /// Set the locale for this specific translation
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }
}
