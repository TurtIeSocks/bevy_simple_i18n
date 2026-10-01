use bevy::{
    ecs::{
        component::{Component, ComponentHooks, StorageType},
        reflect::ReflectComponent,
    },
    log::debug,
    reflect::Reflect,
    ui::widget::Text,
};
use rust_i18n::t;

#[cfg(feature = "numbers")]
use fixed_decimal::FixedDecimal;

/// Component for spawning translatable text entities that are managed by `bevy_simple_i18n`
///
/// It automatically inserts (or replaces) a Bevy `Text` component with the translated text using the provided key
///
/// Updates automatically whenever the locale is changed using the [crate::resources::I18n] resource
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
/// // Basic usage
/// world.spawn(I18nText::new("hello"));
///
/// // With interpolation arguments
/// world.spawn(I18nText::new("greet").with_arg("name", "Bevy User"));
///
/// // With forced locale
/// // overrides the global
/// // does not update when the locale is changed
/// world.spawn(I18nText::new("hello").with_locale("ja"));
/// ```
#[derive(Default, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct I18nText {
    /// Translation key for i18n
    key: String,
    /// Interpolation arguments for the translation key
    args: Vec<(String, InterpolationType)>,
    /// Plural Rules
    #[cfg(feature = "numbers")]
    plural_rules: PluralRule,
    /// Locale for this specific translation, `None` to use the global locale
    pub(crate) locale: Option<String>,
}

impl I18nText {
    /// Creates a new `I18nText` component with the provided translation key
    pub fn new(str: impl Into<String>) -> Self {
        Self {
            key: str.into(),
            args: vec![],
            locale: None,
            #[cfg(feature = "numbers")]
            plural_rules: PluralRule::default(),
        }
    }

    /// Set the locale for this specific translation
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    #[cfg(feature = "numbers")]
    pub fn with_plural_rule(mut self, rules: PluralRule) -> Self {
        self.plural_rules = rules;
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

    /// Internal method that wraps the `rust_i18n::t!` macro
    pub(crate) fn translate(&self) -> String {
        #[cfg(feature = "numbers")]
        let fdf = super::utils::get_formatter(&self.locale, &self.key);

        let (patterns, values): (Vec<&str>, Vec<String>) = self
            .args
            .iter()
            .map(|(k, interpolation_type)| {
                let value = match interpolation_type {
                    InterpolationType::String(v) => v.clone(),
                    #[cfg(feature = "numbers")]
                    InterpolationType::Number(v) => fdf.format_to_string(v),
                };
                (k.as_str(), value)
            })
            .unzip();
        let translated = if let Some(locale) = self.locale.as_ref() {
            t!(&self.key, locale = locale)
        } else {
            t!(&self.key)
        };

        let val = rust_i18n::replace_patterns(&translated, patterns.as_slice(), values.as_slice());
        val
    }
}

impl Component for I18nText {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(_hooks: &mut ComponentHooks) {
        _hooks.on_add(|mut world, entity, _| {
            let val = world.get::<Self>(entity).unwrap().clone();
            debug!("Adding i18n text: {}", val.key);
            if let Some(mut text) = world.get_mut::<Text>(entity) {
                **text = val.translate();
            } else {
                world
                    .commands()
                    .entity(entity)
                    .insert(Text::new(val.translate()));
            }
        });
    }
}

#[derive(Reflect, Debug, Clone)]
enum InterpolationType {
    String(String),
    #[cfg(feature = "numbers")]
    Number(#[reflect(ignore)] FixedDecimal),
}

#[cfg(feature = "numbers")]
#[derive(Reflect, Default, Debug, Clone)]
pub enum PluralRule {
    #[default]
    None,
    Cardinal,
    Ordinal,
}

#[cfg(feature = "numbers")]
impl From<icu_plurals::PluralRuleType> for PluralRule {
    fn from(value: icu_plurals::PluralRuleType) -> Self {
        match value {
            icu_plurals::PluralRuleType::Cardinal => PluralRule::Cardinal,
            icu_plurals::PluralRuleType::Ordinal => PluralRule::Ordinal,
            _ => PluralRule::None,
        }
    }
}
