# Bevy 0.19 Asset-System Facts — Runtime i18n Design Sheet

Ground truth: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_asset-0.19.0/`, `bevy_ecs-0.19.0/`, `bevy_app-0.19.0/`, `bevy_state-0.19.0/`, `bevy_common_assets-0.17.0/`, `bevy_asset_loader-0.27.0/`. Repo confirmed on `bevy = "0.19"` w/ `bevy_asset` feature.

## 1. AssetLoader trait + registration + extension ambiguity

- Trait (`bevy_asset-0.19.0/src/loader.rs:32-52`):
```rust
pub trait AssetLoader: TypePath + Send + Sync + 'static {
    type Asset: Asset;
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;
    type Error: Into<BevyError>;   // NOTE: BevyError, not std Error bound
    fn load(&self, reader: &mut dyn Reader, settings: &Self::Settings,
        load_context: &mut LoadContext)
        -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>>;
    fn extensions(&self) -> &[&str] { &[] }   // no leading dot
}
```
- Registration via `AssetApp` trait on `App` (`lib.rs:556-693`): `register_asset_loader<L>(loader)`, `init_asset_loader::<L: FromWorld>()`, `init_asset::<A>()` (registers `Assets<A>` + `add_message::<AssetEvent<A>>` + `AssetLoadFailedEvent<A>`, `lib.rs:658-659`), `preregister_asset_loader::<L>(extensions)` (blocks matching loads until real loader arrives).
- Extension matching is loader-driven (`extensions()`), NOT on the `Asset` derive.
- **Two loaders claiming ".json"** — resolution in `AssetLoaders::find(asset_type_id, asset_path)` (`src/server/loaders.rs:155-238`):
  - `AssetServer::load::<A>(path)` passes `TypeId::of::<A>()` → candidates filtered to loaders producing `A`. If exactly 1 loader for `A`, it wins regardless of extension. If multiple loaders for `A`, extension filter picks last-registered match; last-registered wins ties.
  - **Different asset types sharing ".json" is FINE for typed loads** — our `.json → LocaleAsset` loader coexists with someone else's `.json → TheirAsset`; the typed handle disambiguates. Untyped loads (`load_untyped`, `load_folder`) use `get_by_extension` = last-registered wins (`loaders.rs:248-252`) — that's the ambiguity trap.
  - Duplicate warn only fires when SAME asset type registers same ext twice: "Duplicate AssetLoader registered… Loader must be specified in a .meta file" (`loaders.rs:66`).
  - **Explicit loader pick:** `.meta` file next to the asset, `AssetAction::Load { loader: String /* loader TypePath */, settings }` (`src/meta.rs:58-62`) → resolved via `get_by_name` (loader full type path). No runtime `load_with::<Loader>` API.
- `load_with_settings` **deprecated in 0.19** → builder API: `asset_server.load_builder().with_settings(|s: &mut S| …).load::<A>(path)` (`server/mod.rs:371,449-457,1873+`). Settings only tweak the chosen loader's `Settings`, they don't choose the loader.
- Practical disambiguation lever: multi-dot extensions. `AssetPath::get_full_extension` returns everything after FIRST dot of filename ("my_asset.config.ron" → "config.ron", `src/path.rs:502-518`); matcher tries full ext then secondary suffixes (`loaders.rs` via `iter_secondary_extensions`). So registering ext `"locale.json"` beats generic `"json"` loaders for `en.locale.json`.

## 2. load_folder / LoadedFolder — platform support

- `AssetServer::load_folder(path) -> Handle<LoadedFolder>` (`server/mod.rs:1115`); `LoadedFolder { handles: Vec<UntypedHandle> }` (`src/folder.rs`). Folder handles reload on file add/remove/move when `file_watcher` on (`server/mod.rs:1110-1113`).
- **wasm: BROKEN.** `HttpWasmAssetReader::read_directory` returns `EmptyPathStream` + `error!("Reading directories is not supported with the HttpWasmAssetReader")`; `is_directory` always `Ok(false)` (`src/io/wasm.rs:127-140`). load_folder on wasm = empty folder + error log, no panic.
- Android: works via NDK `AssetManager::open_dir` (`src/io/android.rs:46-73`) — but NDK open_dir lists files only (subdir entries not returned as dirs by AAssetManager; bevy maps entries flat) — treat as fragile.
- What crates do instead: `bevy_asset_loader-0.27.0/src/standard_dynamic_asset.rs:38` docs its `Folder` variant: "This is not supported for web builds! …consider using `StandardDynamicAsset::Files`" — i.e. explicit file lists in a RON manifest. Other ecosystem answer: embed (bevy_embedded_assets / `embedded_asset!`).

## 3. Hot reload + event naming (0.19)

- Feature `file_watcher` = `notify-debouncer-full`, dep is `cfg(not(target_arch = "wasm32"))` only (`bevy_asset-0.19.0/Cargo.toml:39-41,202`) → **no hot reload on wasm**. `embedded_watcher` implies `file_watcher`. `AssetPlugin.watch_for_changes_override: Option<bool>` (`lib.rs:244`).
- **0.19 naming: buffered events are `Message`.** `EventReader` does NOT exist anywhere in bevy_ecs 0.19 (grep = zero hits). `Event`/`EntityEvent` are observer-only (`bevy_ecs/src/event/mod.rs:88,327`, observers take `On<E>`). Buffered: `trait Message`, `MessageReader<'w,'s,M>`, `MessageWriter`, `app.add_message::<M>()` (`bevy_ecs/src/message/*`).
- `AssetEvent<A>` is `#[derive(Message, Reflect)]` (`src/event.rs:47-62`), variants: `Added{id}`, `Modified{id}`, `Removed{id}`, `Unused{id}`, `LoadedWithDependencies{id}`. Helpers `is_modified(id)`, `is_loaded_with_dependencies(id)`, etc. Also `AssetLoadFailedEvent<A>{id,path,error}` + untyped version.
- Listening: `fn sys(mut reader: MessageReader<AssetEvent<LocaleAsset>>) { for ev in reader.read() {…} }` (usage confirmed in bevy_asset's own test, `lib.rs:988`). Events flushed in `AssetEventSystems` set (`lib.rs:705-709`).
- On reload: re-load re-`insert`s into `Assets<A>` → existing id ⇒ `Modified` (`src/assets.rs:375-391`); `LoadedWithDependencies` also re-fires via internal events (`server/info.rs:492,592`). Listen for `Modified` (and `LoadedWithDependencies` for first ready). `Assets::get_mut` also emits `Modified`; `get_mut_untracked` doesn't (`assets.rs:440-460`). `AssetServer::reload(path)` for manual reload (`server/mod.rs:952`).

## 4. Embedded default locale data — YES

- `embedded_asset!(app, "rock.wgsl")` / `embedded_asset!(app, "/src/", path)` — `include_bytes!` into `EmbeddedAssetRegistry`, registered under the `embedded` AssetSource (`src/io/embedded/mod.rs:343-357`, `EMBEDDED = "embedded"` at line 24). Load path: `asset_server.load::<A>("embedded://crate_name/path/after/src.ext")` (`src` stripped; doc at `mod.rs:290-335`), or `load_embedded_asset!(&asset_server, "file.json")` same-module macro (supports settings arg). Goes through normal loaders ⇒ our JSON/RON locale loader works on embedded bytes. `embedded_watcher` feature hot-reloads embedded files in dev. Fully wasm-compatible (bytes in binary). → Ship default/fallback locale inside crate via `embedded_asset!`, user assets override at runtime.

## 5. Forcing Resource change detection

- `DetectChangesMut` trait (`bevy_ecs-0.19.0/src/change_detection/traits.rs:121-165`): `fn set_changed(&mut self)` — sets changed tick to `this_run` (impl at `traits.rs:431-434`, `params.rs:1365-1368`); also `set_added()`, `bypass_change_detection() -> &mut T` (mutate WITHOUT flagging). Implemented for `ResMut<T>`/`Mut<T>`. So: `fn touch(mut r: ResMut<I18n>) { r.set_changed(); }` makes `Res<I18n>.is_changed()` true next run without field mutation. Import: `bevy::ecs::change_detection::DetectChangesMut` (in prelude).
- Run conditions available: `resource_changed::<T>`, `resource_exists_and_changed::<T>`, `resource_changed_or_removed::<T>` (`bevy_ecs/src/schedule/condition.rs:909,963,1031`).

## 6. Manifest/dynamic-asset patterns

- Bevy itself: **no built-in manifest/collection mechanism** in 0.19. Closest primitives: `load_folder` (native-only), `.meta` files, `LoadContext` dependency loading.
- `bevy_asset_loader` 0.27 dynamic assets: `.assets.ron` = `DynamicAssets` map keyed by string → `StandardDynamicAsset` enum (`standard_dynamic_asset.rs:29-54`): `File{path}`, `Folder{path}` (not web), `Files{paths: Vec<String>}`, `Image{path,…}`, etc. Rough manifest shape:
```ron
({
    "locales.all": Files(paths: ["locales/en.json", "locales/ja.json"]),
})
```
- `bevy_common_assets` 0.17 (supports **bevy 0.19**, Cargo.toml dep `bevy_asset = "0.19.0"`) — canonical generic serde loader pattern: `JsonAssetPlugin::<A>::new(&["ext"])` → `init_asset::<A>() + register_asset_loader(JsonAssetLoader::<A>{extensions})`; loader = read_to_end + `serde_json::from_slice` (`src/json.rs`). Copy this shape for our locale loader (or depend on the crate).
- **Manifest-as-asset pattern (recommended):** manifest loader lists locale files; inside `load()` either (a) `load_context.load::<LocaleAsset>(path)` — deferred dep handle, tracked, parent gets `LoadedWithDependencies` when all locales done (`loader_builders.rs:91`); (b) `load_context.load_builder().load_value::<A>(path).await` — immediate, returns `LoadedAsset<A>` data (`loader_builders.rs:142`); (c) `load_context.read_asset_bytes(path).await -> Vec<u8>` raw sibling read (`loader.rs:569`); (d) `add_labeled_asset(label, asset)` to expose sub-assets `manifest.ron#en` (`loader.rs:479`). All wasm-safe (per-file fetch, no dir listing).

## 7. 0.17–0.19 changes relevant to design

- Event split (0.17): buffered `Message`/`MessageReader`/`MessageWriter`/`add_message`; observers `Event` + `On<E>` + `app.add_observer(…)` (`bevy_app-0.19.0/src/app.rs:1482`). **No observers on Resources in 0.19** (component lifecycle only: `Add`/`Insert`/… in `bevy_ecs/src/lifecycle.rs`); for locale-change reaction use `resource_changed::<T>` run condition or a custom `Message`/`Event`.
- `AssetChanged<C>` query filter (`bevy_asset/src/asset_changed.rs`): like `Changed` but fires when the component's referenced ASSET changes; requires component impl `AsAssetId { type Asset; fn as_asset_id(&self) -> AssetId<Self::Asset> }` (`lib.rs:455-461`). Ideal for "re-render text nodes whose locale asset hot-reloaded": `Query<&mut Text, AssetChanged<I18nTextHandle>>`.
- Readiness: `AssetServer::get_load_state(id) -> Option<LoadState>` (`server/mod.rs:1243`), `LoadState { NotLoaded, Loading, Loaded, Failed(Arc<AssetLoadError>) }` (derives `Component`!, `server/mod.rs:2246-2262`); `is_loaded`, `is_loaded_with_dependencies(id)` (`:1317,1331`); `RecursiveDependencyLoadState` for whole trees.
- States for load-gating: `bevy_state-0.19.0` present — `trait States`, `app.init_state::<S>()/insert_state` (`src/app.rs:40,54`), `OnEnter/OnExit`, needs `StatesPlugin` (in DefaultPlugins).
- `WebAssetPlugin` (`src/io/web.rs`, features `http`/`https`): `asset_server.load("https://…/en.json")` via fetch on wasm / ureq native — optional remote-locale story.
- `UnapprovedPathMode` guards paths outside asset root; builder `.override_unapproved()`.
- `register_asset_source(id, AssetSourceBuilder)` must run BEFORE `AssetPlugin` is added (`lib.rs:560-567`) — custom `i18n://` source possible but ordering-hostile for a library plugin; avoid.
- `Res<Assets<T>>` change-detection gotcha: `Assets<T>` is a normal Resource — ANY asset add/remove/mutate of type T marks whole resource changed; per-asset granularity requires `AssetEvent<T>` messages or `AssetChanged` filter, not `res.is_changed()`.

## Constraints for our design

1. **No folder enumeration on wasm** (silent empty result). Locale discovery must be: (a) explicit list in plugin config, (b) RON/JSON manifest asset listing files (each then loaded as dependency — works everywhere), or (c) embedded defaults. Pick manifest-as-asset + `load_context.load` deps → single `LoadedWithDependencies` readiness signal; optionally allow `load_folder` fast-path behind `#[cfg(not(target_arch="wasm32"))]`.
2. **No hot reload on wasm** (notify dep excluded); design must degrade gracefully — reload path driven purely by `AssetEvent::Modified` messages, which simply never fire there.
3. Use dedicated multi-dot extensions (e.g. `en.locale.json` or `.i18n.ron`) or rely on typed `load::<LocaleAsset>` to dodge `.json` loader collisions; never rely on untyped extension dispatch.
4. Locale-switch reactivity: `Res<I18n>` resource + `resource_changed::<I18n>` run condition; re-translate on `AssetEvent::Modified` too; `set_changed()` (DetectChangesMut) available to force refresh after asset reload without touching fields. Consider `AsAssetId` wrapper component + `AssetChanged` filter for per-entity precision.
5. Ship fallback locale via `embedded_asset!` (`embedded://bevy_simple_i18n/...`) — wasm-safe, zero-config, still goes through same loader; user `assets/locales/` overrides.
6. `MessageReader` not `EventReader`; `AssetEvent` variants Added/Modified/Removed/Unused/LoadedWithDependencies; register asset types with `init_asset::<A>()` + `register_asset_loader`.
7. `AssetLoader::load` is async, `Settings` must be serde+Default (use `()` if none), `Error: Into<BevyError>` (thiserror enum fine, pattern per bevy_common_assets 0.17 which already supports bevy 0.19 — candidate dependency instead of hand-rolling JSON/RON loaders).