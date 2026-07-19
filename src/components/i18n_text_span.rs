use super::define_i18n_text_component;

define_i18n_text_component!(
    /// Component for translatable rich-text spans managed by `bevy_simple_i18n`.
    ///
    /// A Bevy [`TextSpan`](bevy::prelude::TextSpan) is inserted automatically (via
    /// `#[require(TextSpan)]`) and kept in sync with the translated value. Spawn it
    /// as a CHILD of an entity with [`Text`](bevy::prelude::Text) (or
    /// [`Text2d`](bevy::prelude::Text2d)) to compose one paragraph out of several
    /// independently styled, independently translated pieces:
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_simple_i18n::prelude::*;
    /// # fn system(mut commands: Commands) {
    /// commands.spawn(I18nText::new("greeting")).with_child((
    ///     I18nTextSpan::new("player_name_label").with_arg("name", "Alex"),
    ///     TextColor(Color::srgb(1.0, 0.8, 0.2)),
    /// ));
    /// # }
    /// ```
    I18nTextSpan, target: bevy::prelude::TextSpan
);
