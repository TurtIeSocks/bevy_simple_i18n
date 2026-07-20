use bevy_rich_text3d::FetchedTextSegment;

use super::{define_i18n_text_component, I18nTarget};

// This impl lives HERE (not in user code) because of the orphan rule:
// `I18nTarget` is this crate's trait, so this crate may implement it for the
// foreign `FetchedTextSegment` — no user crate can.
impl I18nTarget for FetchedTextSegment {
    fn set_text(&mut self, text: String) {
        self.0 = text;
    }
}

define_i18n_text_component!(
    /// Component for translatable `bevy_rich_text3d` text segments (feature
    /// `rich_text3d`).
    ///
    /// A [`FetchedTextSegment`] is inserted automatically (via `#[require(..)]`).
    /// Reference this entity from a [`Text3d`](bevy_rich_text3d::Text3d) via
    /// `Text3dSegment::Extract(entity)` and the segment re-translates on locale
    /// change like any other i18n component — interpolation, plurals and the
    /// in-place setters all work.
    I18nText3dSegment, target: bevy_rich_text3d::FetchedTextSegment
);
