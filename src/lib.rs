mod assets;
mod components;
mod interpolate;
mod parse;
mod plugin;
#[cfg(feature = "plurals")]
mod plural;
mod resources;

pub mod prelude {
    pub use crate::assets::{
        FontFamilyEntry, I18nManifest, I18nManifestLoader, TranslationFile, TranslationFileLoader,
    };
    pub use crate::components::*;
    pub use crate::plugin::*;
    pub use crate::resources::*;
}
