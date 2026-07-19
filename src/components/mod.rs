use std::ops::DerefMut;

use bevy::ecs::component::Mutable;
use bevy::prelude::Component;

mod i18n_font;
#[cfg(feature = "numbers")]
mod i18n_number;
mod i18n_text;
mod i18n_text_2d;
pub(crate) mod utils;

pub use i18n_font::*;
#[cfg(feature = "numbers")]
pub use i18n_number::*;
pub use i18n_text::*;
pub use i18n_text_2d::*;

/// Trait implemented by every component that `bevy_simple_i18n` keeps translated.
///
/// Implementing this and registering it with
/// [`register_i18n_component`](crate::prelude::I18nComponentRegistration::register_i18n_component)
/// lets you drive any custom text component from a translation key. The built-in
/// [`I18nText`], [`I18nText2d`] and [`I18nNumber`] components all implement it.
///
/// Both methods receive the [`I18n`](crate::prelude::I18n) resource — all locale
/// state lives in ECS, there are no global statics.
pub trait I18nComponent: Component {
    /// The Bevy text component this writes its translated value into.
    ///
    /// It must dereference to a [`String`] — as Bevy's [`Text`](bevy::prelude::Text) and
    /// [`Text2d`](bevy::prelude::Text2d) both do — and is inserted automatically via the
    /// `#[require(..)]` attribute on the implementing component.
    type Target: Component<Mutability = Mutable> + DerefMut<Target = String>;

    /// Returns this component's locale: its per-entity override if one was set,
    /// otherwise the current locale of the [`I18n`](crate::prelude::I18n) resource.
    fn locale<'a>(&'a self, i18n: &'a crate::prelude::I18n) -> &'a str;

    /// Produces the translated / localized string for the resolved [`locale`](Self::locale).
    fn translate(&self, i18n: &crate::prelude::I18n) -> String;
}

#[derive(bevy::prelude::Reflect, Debug, Clone)]
pub(crate) enum InterpolationType {
    String(String),
    #[cfg(feature = "numbers")]
    Number(#[reflect(ignore)] fixed_decimal::Decimal),
}

/// Generates an `I18nText`-shaped component: same fields, same [`I18nComponent`] impl
/// shape, same builder methods — only the type name, the `#[require(..)]`ed target,
/// and the struct-level rustdoc (passed in at the invocation site) differ.
///
/// Paths inside this macro's body are fully qualified (`crate::...` / `bevy::...`)
/// rather than relying on the invoking module's imports, since `macro_rules!` path
/// resolution follows the definition site, not the invocation site.
macro_rules! define_i18n_text_component {
    (
        $(#[$struct_doc:meta])*
        $name:ident, target: $target:ty
    ) => {
        // `#[reflect(Component)]` below expands (via the `Reflect` derive) to code that
        // references `ReflectComponent` unqualified at this expansion site — bring it
        // into scope here rather than requiring every invocation site to import it.
        use bevy::prelude::ReflectComponent;

        $(#[$struct_doc])*
        #[derive(bevy::prelude::Component, Default, bevy::prelude::Reflect, Debug, Clone)]
        #[reflect(Component)]
        #[require($target)]
        pub struct $name {
            /// Translation key for i18n
            key: String,
            /// Interpolation arguments for the translation key
            args: Vec<(String, crate::components::InterpolationType)>,
            /// Locale for this specific translation, `None` to use the global locale
            pub(crate) locale: Option<String>,
            /// Plural count: resolves the key to its CLDR plural form and injects `%{count}`
            #[cfg(feature = "plurals")]
            #[reflect(ignore)]
            count: Option<fixed_decimal::Decimal>,
        }

        impl crate::components::I18nComponent for $name {
            type Target = $target;

            fn locale<'a>(&'a self, i18n: &'a crate::prelude::I18n) -> &'a str {
                self.locale.as_deref().unwrap_or_else(|| i18n.current())
            }

            fn translate(&self, i18n: &crate::prelude::I18n) -> String {
                #[cfg(feature = "plurals")]
                if let Some(count) = &self.count {
                    return crate::components::utils::translate_plural(
                        i18n,
                        self.locale(i18n),
                        &self.key,
                        &self.args,
                        count,
                    );
                }
                crate::components::utils::translate_by_key(i18n, self.locale(i18n), &self.key, &self.args)
            }
        }

        impl $name {
            /// Creates a new component with the provided translation key
            pub fn new(str: impl Into<String>) -> Self {
                Self {
                    key: str.into(),
                    args: vec![],
                    locale: None,
                    #[cfg(feature = "plurals")]
                    count: None,
                }
            }

            /// Sets the plural count: the key resolves to its plural sub-key (exact integer
            /// `key.0`, CLDR category `key.one`/`key.few`/…, then `key.other`, then the bare
            /// key), and `%{count}` becomes available as a localized interpolation argument.
            ///
            /// ```json
            /// // en.json
            /// {
            ///     "cats.0": "You have no cats",
            ///     "cats.one": "You have %{count} cat",
            ///     "cats.other": "You have %{count} cats"
            /// }
            /// ```
            #[cfg(feature = "plurals")]
            pub fn with_count(mut self, count: impl Into<f64>) -> Self {
                self.count = crate::components::utils::try_f64_to_fd(count.into());
                self
            }

            /// Set the locale for this specific translation.
            ///
            /// Underscore-separated tags (`en_US`) are normalized to hyphens before
            /// validation. An invalid locale is ignored (logged as an error) and the
            /// global locale is used instead.
            pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
                let raw: String = locale.into();
                let normalized = raw.replace('_', "-");
                match normalized.parse::<icu_locale_core::Locale>() {
                    Ok(_) => self.locale = Some(normalized),
                    Err(err) => bevy::log::error!("Ignoring invalid locale {raw:?}: {err}"),
                }
                self
            }

            /// Add a standard string interpolation argument to the translation key
            ///
            /// This method can be called as many times as needed
            pub fn with_arg(mut self, key: impl Into<String>, value: impl ToString) -> Self {
                self.args.push((
                    key.into(),
                    crate::components::InterpolationType::String(value.to_string()),
                ));
                self
            }

            #[cfg(feature = "numbers")]
            /// Add a number interpolation argument to the translation key.
            ///
            /// This method can be called as many times as needed. A NaN/infinite value is a
            /// data bug worth an error log, not a crash: the argument is skipped and the
            /// `%{name}` pattern stays verbatim in the rendered text.
            pub fn with_num_arg(mut self, key: impl Into<String>, value: impl Into<f64>) -> Self {
                if let Some(fd) = crate::components::utils::try_f64_to_fd(value.into()) {
                    self.args.push((
                        key.into(),
                        crate::components::InterpolationType::Number(fd),
                    ));
                }
                self
            }
        }
    };
}
pub(crate) use define_i18n_text_component;
