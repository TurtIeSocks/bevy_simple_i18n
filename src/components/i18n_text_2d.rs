use bevy::prelude::*;

use super::{utils::translate_by_key, I18nComponent, InterpolationType};

/// Component for spawning translatable text 2d entities that are managed by `bevy_simple_i18n`
///
/// A Bevy [`Text2d`] component is inserted automatically (via `#[require(Text2d)]`) and kept in
/// sync with the translated value for the provided key.
///
/// Updates automatically whenever the locale is changed using the [`crate::resources::I18n`] resource
///
/// # Example
///
/// ```json
/// // en.json
/// {
///     "hello": "Hello, World!",
///     "greet": "Hello, %{name}!"
/// }
/// ```
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_simple_i18n::prelude::*;
/// # fn system(mut commands: Commands) {
/// // Basic usage
/// commands.spawn(I18nText2d::new("hello"));
///
/// // With interpolation arguments
/// commands.spawn(I18nText2d::new("greet").with_arg("name", "Bevy User"));
///
/// // With forced locale
/// // overrides the global
/// // does not update when the locale is changed
/// commands.spawn(I18nText2d::new("hello").with_locale("ja"));
/// # }
/// ```
#[derive(Component, Default, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(Text2d)]
pub struct I18nText2d {
    /// Translation key for i18n
    key: String,
    /// Interpolation arguments for the translation key
    args: Vec<(String, InterpolationType)>,
    /// Locale for this specific translation, `None` to use the global locale
    pub(crate) locale: Option<String>,
}

impl I18nComponent for I18nText2d {
    type Target = Text2d;

    fn locale(&self) -> String {
        self.locale
            .clone()
            .unwrap_or_else(|| rust_i18n::locale().to_string())
    }

    fn translate(&self) -> String {
        translate_by_key(&self.locale(), &self.key, &self.args)
    }
}

impl I18nText2d {
    /// Creates a new [`I18nText2d`] component with the provided translation key
    pub fn new(str: impl Into<String>) -> Self {
        Self {
            key: str.into(),
            args: vec![],
            locale: None,
        }
    }

    /// Set the locale for this specific translation
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// Add a standard string interpolation argument to the translation key
    ///
    /// This method can be called as many times as needed
    pub fn with_arg(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.args
            .push((key.into(), InterpolationType::String(value.to_string())));
        self
    }

    #[cfg(feature = "numbers")]
    /// Add a number interpolation argument to the translation key
    ///
    /// This method can be called as many times as needed
    pub fn with_num_arg(mut self, key: impl Into<String>, value: impl Into<f64>) -> Self {
        self.args.push((
            key.into(),
            InterpolationType::Number(super::utils::f64_to_fd(value.into())),
        ));
        self
    }
}
