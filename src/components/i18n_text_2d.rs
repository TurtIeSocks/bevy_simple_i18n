// Not referenced by code — only brings `Text2d` into scope so the `[`Text2d`]`
// intra-doc link in the struct doc below resolves.
#[allow(unused_imports)]
use bevy::prelude::Text2d;

use super::define_i18n_text_component;

define_i18n_text_component!(
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
    I18nText2d, target: bevy::prelude::Text2d
);
