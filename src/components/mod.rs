use bevy::ecs::component::Mutable;
use bevy::prelude::Component;

mod i18n_font;
#[cfg(feature = "numbers")]
mod i18n_number;
mod i18n_text;
mod i18n_text_2d;
#[cfg(feature = "rich_text3d")]
mod i18n_text_3d_segment;
mod i18n_text_span;
pub(crate) mod utils;

pub use i18n_font::*;
#[cfg(feature = "numbers")]
pub use i18n_number::*;
pub use i18n_text::*;
pub use i18n_text_2d::*;
#[cfg(feature = "rich_text3d")]
pub use i18n_text_3d_segment::*;
pub use i18n_text_span::*;

/// A text component `bevy_simple_i18n` can write translated strings into.
///
/// Implemented for Bevy's [`Text`](bevy::prelude::Text),
/// [`Text2d`](bevy::prelude::Text2d) and [`TextSpan`](bevy::prelude::TextSpan).
/// Implement it for any third-party component that carries a `String` (e.g.
/// `bevy_rich_text3d`'s `FetchedTextSegment`) to drive it from a translation
/// key with the full pipeline — interpolation, plurals, per-entity locales:
///
/// ```rust,ignore
/// impl I18nTarget for FetchedTextSegment {
///     fn set_text(&mut self, text: String) {
///         self.0 = text;
///     }
/// }
/// ```
///
/// Deliberately NOT blanket-implemented over `DerefMut<Target = String>`:
/// a blanket impl would make downstream `impl I18nTarget for TheirType`
/// a coherence error (E0119) the moment `TheirType` could ever deref to
/// `String`. Explicit impls keep the trait open for the ecosystem.
pub trait I18nTarget: Component<Mutability = Mutable> {
    /// Replaces the component's text with the translated value.
    fn set_text(&mut self, text: String);
}

impl I18nTarget for bevy::prelude::Text {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

impl I18nTarget for bevy::prelude::Text2d {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

impl I18nTarget for bevy::prelude::TextSpan {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

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
    /// The text component this writes its translated value into — anything
    /// implementing [`I18nTarget`]. It is inserted automatically via the
    /// `#[require(..)]` attribute on the implementing component.
    type Target: I18nTarget;

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
                self.set_locale(locale);
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

            /// Replaces the translation key in place.
            pub fn set_key(&mut self, key: impl Into<String>) {
                self.key = key.into();
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

            /// Sets a standard string interpolation argument, in place.
            ///
            /// If an argument with the same `key` already exists, its value is replaced
            /// in place (preserving position — interpolation is first-match-wins);
            /// otherwise the argument is appended.
            pub fn set_arg(&mut self, key: impl Into<String>, value: impl ToString) {
                let key = key.into();
                let value = crate::components::InterpolationType::String(value.to_string());
                match self.args.iter_mut().find(|entry| entry.0 == key) {
                    Some(entry) => entry.1 = value,
                    None => self.args.push((key, value)),
                }
            }

            #[cfg(feature = "numbers")]
            /// Sets a number interpolation argument, in place.
            ///
            /// Upserts like [`Self::set_arg`]. A NaN/infinite value is a data bug worth
            /// an error log, not a crash: the argument is left unchanged (or simply not
            /// added, if it didn't already exist).
            pub fn set_num_arg(&mut self, key: impl Into<String>, value: impl Into<f64>) {
                let Some(fd) = crate::components::utils::try_f64_to_fd(value.into()) else {
                    return;
                };
                let key = key.into();
                let value = crate::components::InterpolationType::Number(fd);
                match self.args.iter_mut().find(|entry| entry.0 == key) {
                    Some(entry) => entry.1 = value,
                    None => self.args.push((key, value)),
                }
            }

            /// Removes all interpolation arguments.
            pub fn clear_args(&mut self) {
                self.args.clear();
            }

            #[cfg(feature = "plurals")]
            /// Sets the plural count in place (see [`Self::with_count`] for resolution
            /// order). A NaN/infinite value is a data bug worth an error log, not a
            /// crash: the count is left unchanged rather than reset to `None`, so an
            /// invalid update never destroys previously valid state.
            pub fn set_count(&mut self, count: impl Into<f64>) {
                if let Some(fd) = crate::components::utils::try_f64_to_fd(count.into()) {
                    self.count = Some(fd);
                }
            }
        }
    };
}
pub(crate) use define_i18n_text_component;
