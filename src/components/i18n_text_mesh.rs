use super::{define_i18n_text_component, I18nTarget};

// This impl lives HERE (not in user code) because of the orphan rule:
// `I18nTarget` is this crate's trait, so this crate may implement it for the
// foreign `bevy_fontmesh::TextMesh` — no user crate can.
impl I18nTarget for bevy_fontmesh::TextMesh {
    fn set_text(&mut self, text: String) {
        self.text = text;
    }
}

define_i18n_text_component!(
    /// Component for translatable `bevy_fontmesh` 3D mesh text (feature `fontmesh`).
    ///
    /// A [`TextMesh`](bevy_fontmesh::TextMesh) is inserted automatically (via
    /// `#[require(..)]`) and its `text` field is kept in sync — the mesh
    /// regenerates on change. Set the font by inserting your own `TextMesh`
    /// alongside (the `#[require]` only fills in a default when missing).
    I18nTextMesh, target: bevy_fontmesh::TextMesh
);
