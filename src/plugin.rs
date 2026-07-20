use std::path::Path;

use bevy::asset::{AssetLoadFailedEvent, AssetPath};
use bevy::prelude::*;

use bevy::ecs::component::Mutable;

use crate::{
    assets::{I18nManifest, I18nManifestLoader, TranslationFile, TranslationFileLoader},
    components::{I18nFont, I18nKey, I18nTarget, I18nText, I18nText2d, I18nTextSpan},
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
            .register_type::<I18nTextSpan>()
            .register_type::<I18nFont>()
            .register_type::<I18nKey>()
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
            .register_i18n_component::<I18nText2d>()
            .register_i18n_component::<I18nTextSpan>();

        #[cfg(feature = "detect")]
        app.add_systems(PreStartup, detect_system_locale);

        #[cfg(feature = "numbers")]
        app.register_type::<crate::components::I18nNumber>()
            .register_i18n_component::<crate::components::I18nNumber>();

        #[cfg(feature = "rich_text3d")]
        app.register_type::<crate::components::I18nText3dSegment>()
            .register_i18n_component::<crate::components::I18nText3dSegment>();

        #[cfg(feature = "fontmesh")]
        app.register_type::<crate::components::I18nTextMesh>()
            .register_i18n_component::<crate::components::I18nTextMesh>();
    }
}

pub trait I18nComponentRegistration {
    /// Registers an i18n component for automatic translation updates
    fn register_i18n_component<T: I18nComponent>(&mut self) -> &mut Self;

    /// Registers a closure that writes translated text (driven by an [`I18nKey`]
    /// on the same entity) into `Target` — for third-party components this crate
    /// has no [`I18nTarget`](crate::prelude::I18nTarget) impl for. The closure
    /// runs whenever the locale, translation table, or the entity's `I18nKey`
    /// changes.
    fn register_i18n_writer<Target, F>(&mut self, writer: F) -> &mut Self
    where
        Target: Component<Mutability = Mutable>,
        F: Fn(&mut Target, String) + Send + Sync + 'static;
}

impl I18nComponentRegistration for App {
    fn register_i18n_component<T: I18nComponent>(&mut self) -> &mut Self {
        self.add_systems(Update, update_translations::<T>)
    }

    fn register_i18n_writer<Target, F>(&mut self, writer: F) -> &mut Self
    where
        Target: Component<Mutability = Mutable>,
        F: Fn(&mut Target, String) + Send + Sync + 'static,
    {
        self.add_systems(
            Update,
            move |i18n: Res<I18n>, mut query: Query<(&mut Target, Ref<I18nKey>)>| {
                let i18n_changed = i18n.is_changed();
                for (mut target, key) in query.iter_mut() {
                    if !i18n_changed && !key.is_changed() {
                        continue;
                    }
                    writer(&mut target, key.translate(&i18n));
                }
            },
        )
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
        Option<Ref<I18nFont>>,
        Ref<T>,
    )>,
) {
    let i18n_changed = i18n.is_changed();
    for (mut target, text_font, dyn_font, key) in query.iter_mut() {
        // Also react to a just-inserted/changed I18nFont, so adding a dynamic font to
        // an existing entity applies it without waiting for the next locale change.
        let font_changed = dyn_font.as_ref().is_some_and(|font| font.is_changed());
        if !i18n_changed && !key.is_changed() && !font_changed {
            continue;
        }
        bevy::log::debug!("Updating translation for locale {}", key.locale(&i18n));
        target.set_text(key.translate(&i18n));
        if let (Some(mut text_font), Some(dyn_font)) = (text_font, dyn_font) {
            // Bevy 0.19: TextFont::font is a `FontSource` enum; Handle<Font> converts via From.
            text_font.font = font_manager.get(&dyn_font.0, key.locale(&i18n)).into();
        }
    }
}

/// Detects the system/device locale once at startup and stages it on [`I18n`].
///
/// It is applied when the translation table loads — and only if the game ships that
/// locale (or a parent of it); otherwise the manifest's `default_locale` wins. A later
/// explicit [`I18n::set_locale`] always overrides it.
#[cfg(feature = "detect")]
fn detect_system_locale(mut i18n: ResMut<I18n>) {
    let Some(raw) = bevy_device_lang::get_lang() else {
        bevy::log::debug!("No system locale detected");
        return;
    };
    // Normalize platform quirks (`en_US`) and validate: an unparseable tag must not
    // reach the icu number formatter, which panics on invalid locales.
    match raw.replace('_', "-").parse::<icu_locale_core::Locale>() {
        Ok(locale) => {
            bevy::log::debug!("Detected system locale: {locale}");
            i18n.set_detected(locale.to_string());
        }
        Err(err) => {
            bevy::log::warn!("Ignoring unparseable system locale {raw:?}: {err}");
        }
    }
}

/// Rebuilds the [`I18n`] translation table (and the dynamic-font registry) whenever the
/// manifest or any translation file is added, modified (hot reload) or finishes loading,
/// and surfaces load failures with actionable logs.
#[allow(clippy::too_many_arguments)]
fn sync_translations(
    mut manifest_events: MessageReader<AssetEvent<I18nManifest>>,
    mut file_events: MessageReader<AssetEvent<TranslationFile>>,
    mut manifest_failures: MessageReader<AssetLoadFailedEvent<I18nManifest>>,
    mut file_failures: MessageReader<AssetLoadFailedEvent<TranslationFile>>,
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
    // Fold instead of `.any()`: readers must be FULLY drained even after a match,
    // otherwise leftover same-frame events retrigger a redundant rebuild next frame.
    let mut relevant = manifest_events
        .read()
        .fold(false, |hit, e| hit | is_relevant(e));
    relevant = file_events
        .read()
        .fold(relevant, |hit, e| hit | is_relevant(e));

    // A failed dependency is terminal in bevy_asset (no retry), so treat it as
    // completion: log something actionable and let `ready()` latch rather than
    // leaving loading screens gated on `I18n::ready()` hanging forever.
    for failure in manifest_failures.read() {
        bevy::log::error!(
            "bevy_simple_i18n: failed to load the i18n manifest `{}`: {}. Create it, or \
             point `I18nPlugin::with_manifest` at the right path (relative to the asset \
             root, e.g. \"locales/i18n.ron\"). Continuing without translations.",
            failure.path,
            failure.error
        );
        i18n.mark_ready();
    }
    for failure in file_failures.read() {
        bevy::log::error!(
            "bevy_simple_i18n: failed to load translation file `{}` (listed in the i18n \
             manifest): {}. Its translations are skipped.",
            failure.path,
            failure.error
        );
        relevant = true;
    }

    if !relevant {
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

    // `ready` latches on at any TERMINAL load state — fully loaded, or failed
    // dependencies (degraded but done). A hot reload never flips a running game
    // back to "loading".
    let ready = i18n.ready()
        || asset_server
            .get_recursive_dependency_load_state(&manifest_handle.0)
            .is_some_and(|state| state.is_loaded() || state.is_failed());
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

    // Dynamic fonts: rebuild the registry from scratch so families removed from a
    // hot-reloaded manifest don't linger. `asset_server.load` deduplicates by path,
    // so re-requesting surviving fonts is cheap.
    font_manager.fonts.clear();
    for entry in &manifest.fonts {
        let mut folder = FontFolder::default();
        for file in &entry.files {
            let stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file);
            // Keep the manifest's asset source (e.g. `embedded://`): `dir` is relative
            // to the asset root of whichever source the manifest came from.
            let path = AssetPath::from(Path::new(&entry.dir).join(file))
                .with_source(manifest.source.clone());
            let handle = asset_server.load(path);
            if stem == "fallback" {
                folder.fallback = handle;
            } else {
                folder.fonts.insert(stem.to_string(), handle);
            }
        }
        font_manager.insert(entry.family.clone(), folder);
    }
}
