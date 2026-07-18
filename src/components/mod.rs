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
