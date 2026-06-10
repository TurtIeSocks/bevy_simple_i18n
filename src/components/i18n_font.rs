use bevy::prelude::*;

/// Component for spawning dynamic font entities that are managed by `bevy_simple_i18n`
///
/// A Bevy [`TextFont`] component is inserted automatically (via `#[require(TextFont)]`) and its
/// font handle is updated based on the locale of the sibling i18n component
/// ([`I18nText`](crate::prelude::I18nText), [`I18nNumber`](crate::prelude::I18nNumber) or
/// [`I18nText2d`](crate::prelude::I18nText2d)) on the same entity.
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_simple_i18n::prelude::*;
/// # fn system(mut commands: Commands) {
/// commands.spawn((I18nText::new("hello"), I18nFont::new("NotoSans")));
/// # }
/// ```
#[derive(Component, Default, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(TextFont)]
pub struct I18nFont(pub(crate) String);

impl I18nFont {
    /// Creates a new `I18nFont` component from the provided font family
    pub fn new(family: impl Into<String>) -> Self {
        Self(family.into())
    }
}
