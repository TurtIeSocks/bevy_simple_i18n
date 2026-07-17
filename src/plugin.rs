use std::path::Path;

use bevy::prelude::*;

use crate::{
    assets::{I18nManifest, I18nManifestLoader, TranslationFile, TranslationFileLoader},
    components::{I18nFont, I18nText, I18nText2d},
    parse,
    prelude::I18nComponent,
    resources::{FontFolder, FontManager, I18n},
};

/// Initializes the `bevy_simple_i18n` plugin.
///
/// Locale files are loaded at runtime through the asset server, declared by a RON
/// manifest (see [`I18nManifest`]) that lives at `locales/i18n.ron` by default.
///
/// # Example
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_simple_i18n::prelude::*;
///
/// fn main() {
///     App::new()
///         .add_plugins(DefaultPlugins)
///         .add_plugins(I18nPlugin::default())
///         .run();
/// }
/// ```
pub struct I18nPlugin {
    /// Asset path of the i18n manifest, relative to the asset root.
    pub manifest_path: String,
}

impl Default for I18nPlugin {
    fn default() -> Self {
        Self {
            manifest_path: "locales/i18n.ron".to_string(),
        }
    }
}

impl I18nPlugin {
    /// Uses a manifest at a non-default location.
    pub fn with_manifest(path: impl Into<String>) -> Self {
        Self {
            manifest_path: path.into(),
        }
    }
}

/// Strong handle to the manifest; keeps it (and, through its dependencies, every
/// translation file) alive for the lifetime of the app.
#[derive(Resource)]
struct ManifestHandle(Handle<I18nManifest>);

impl Plugin for I18nPlugin {
    fn build(&self, app: &mut App) {
        let manifest_path = self.manifest_path.clone();
        app.init_resource::<I18n>()
            .init_resource::<FontManager>()
            .register_type::<I18n>()
            .register_type::<I18nText>()
            .register_type::<I18nText2d>()
            .register_type::<I18nFont>()
            .init_asset::<TranslationFile>()
            .init_asset::<I18nManifest>()
            .register_asset_loader(TranslationFileLoader)
            .register_asset_loader(I18nManifestLoader)
            .add_systems(
                PreStartup,
                move |asset_server: Res<AssetServer>, mut commands: Commands| {
                    commands.insert_resource(ManifestHandle(
                        // Clone: `load` wants an `AssetPath<'static>`; this runs once.
                        asset_server.load::<I18nManifest>(manifest_path.clone()),
                    ));
                },
            )
            // PreUpdate so a table rebuild and the resulting re-translation land in
            // the same frame (`update_translations` runs in Update).
            .add_systems(PreUpdate, sync_translations)
            .register_i18n_component::<I18nText>()
            .register_i18n_component::<I18nText2d>();

        #[cfg(feature = "numbers")]
        app.register_type::<crate::components::I18nNumber>()
            .register_i18n_component::<crate::components::I18nNumber>();
    }
}

pub trait I18nComponentRegistration {
    /// Registers an i18n component for automatic translation updates
    fn register_i18n_component<T: I18nComponent>(&mut self) -> &mut Self;
}

impl I18nComponentRegistration for App {
    fn register_i18n_component<T: I18nComponent>(&mut self) -> &mut Self {
        self.add_systems(Update, update_translations::<T>)
    }
}

/// Keeps the translated text (and dynamic font) of every registered [`I18nComponent`] in sync.
///
/// Thanks to change detection this is cheap to run every frame: an entity's target text is only
/// rewritten when the entity's i18n component was just added/changed, or when the [`I18n`]
/// resource changed — a locale switch, or the sync system rebuilding the table after an asset
/// load or hot reload.
#[allow(clippy::type_complexity)]
fn update_translations<T: I18nComponent>(
    i18n: Res<I18n>,
    font_manager: Res<FontManager>,
    mut query: Query<(
        &mut T::Target,
        Option<&mut TextFont>,
        Option<&I18nFont>,
        Ref<T>,
    )>,
) {
    let i18n_changed = i18n.is_changed();
    for (mut target, text_font, dyn_font, key) in query.iter_mut() {
        if !i18n_changed && !key.is_changed() {
            continue;
        }
        bevy::log::debug!("Updating translation for locale {}", key.locale(&i18n));
        **target = key.translate(&i18n);
        if let (Some(mut text_font), Some(dyn_font)) = (text_font, dyn_font) {
            // Bevy 0.19: TextFont::font is a `FontSource` enum; Handle<Font> converts via From.
            text_font.font = font_manager.get(&dyn_font.0, key.locale(&i18n)).into();
        }
    }
}

/// Rebuilds the [`I18n`] translation table (and the dynamic-font registry) whenever the
/// manifest or any translation file is added, modified (hot reload) or finishes loading.
#[allow(clippy::too_many_arguments)]
fn sync_translations(
    mut manifest_events: MessageReader<AssetEvent<I18nManifest>>,
    mut file_events: MessageReader<AssetEvent<TranslationFile>>,
    manifest_handle: Option<Res<ManifestHandle>>,
    manifests: Res<Assets<I18nManifest>>,
    files: Res<Assets<TranslationFile>>,
    asset_server: Res<AssetServer>,
    mut i18n: ResMut<I18n>,
    mut font_manager: ResMut<FontManager>,
) {
    fn is_relevant<A: Asset>(event: &AssetEvent<A>) -> bool {
        matches!(
            event,
            AssetEvent::Added { .. }
                | AssetEvent::Modified { .. }
                | AssetEvent::LoadedWithDependencies { .. }
        )
    }
    // `.count()`-free: both readers must be drained even when the first already matched,
    // otherwise stale events retrigger next frame.
    let manifest_relevant = manifest_events.read().any(is_relevant);
    let files_relevant = file_events.read().any(is_relevant);
    if !manifest_relevant && !files_relevant {
        return;
    }
    let Some(manifest_handle) = manifest_handle else {
        return;
    };
    let Some(manifest) = manifests.get(&manifest_handle.0) else {
        return;
    };

    // Merge in manifest order: later files win on conflicting (locale, key) pairs.
    let mut table = parse::Table::default();
    for handle in &manifest.files {
        if let Some(file) = files.get(handle) {
            parse::merge_into(&mut table, file.table.clone());
        }
    }

    // `ready` latches on: a hot reload never flips a running game back to "loading".
    let ready = i18n.ready() || asset_server.is_loaded_with_dependencies(&manifest_handle.0);
    bevy::log::debug!(
        "Rebuilt i18n table: {} locale(s), ready: {ready}",
        table.len()
    );
    i18n.apply_table(
        table,
        &manifest.default_locale,
        manifest.fallback.clone(),
        ready,
    );

    // Dynamic fonts: `asset_server.load` deduplicates by path, so rebuilding the
    // registry on every sync is cheap.
    for entry in &manifest.fonts {
        let mut folder = FontFolder::default();
        for file in &entry.files {
            let stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file);
            let handle = asset_server.load(Path::new(&entry.dir).join(file));
            if stem == "fallback" {
                folder.fallback = handle;
            } else {
                folder.fonts.insert(stem.to_string(), handle);
            }
        }
        font_manager.insert(entry.family.clone(), folder);
    }
}
