use std::{
    env,
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

const ASSET_PATH_VAR: &str = "BEVY_ASSET_PATH";
const OUTPUT_FILE_NAME: &str = "bevy_simple_i18n.rs";
const ALLOWED_EXTENSIONS: &[&str] = &["otf", "ttf"];

fn main() {
    cargo_emit::rerun_if_env_changed!(ASSET_PATH_VAR);

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let mut marker_file = File::create(Path::new(&out_dir).join(OUTPUT_FILE_NAME)).unwrap();

    let asset_dir = resolve_asset_dir();

    // Always emit a `rust_i18n::i18n!` invocation, even when no asset folder is found. The `t!`
    // macro used throughout the crate expands to a call into the backend this macro generates, so
    // without it the crate fails to compile (rather than just lacking translations).
    // See https://github.com/TurtIeSocks/bevy_simple_i18n/issues/6.
    let i18n_path = match &asset_dir {
        Some(dir) => dir.to_string_lossy().replace('\\', "/"),
        None => "locales".to_string(),
    };
    marker_file
        .write_all(format!("rust_i18n::i18n!({i18n_path:?});\n\n").as_bytes())
        .unwrap();

    // Collect every font file under the asset folder (if we found one).
    let mut files = Vec::new();
    if let Some(dir) = &asset_dir {
        cargo_emit::rerun_if_changed!(dir.to_string_lossy());
        for full_path in visit_dirs(dir) {
            cargo_emit::rerun_if_changed!(full_path.to_string_lossy());
            let path = full_path.strip_prefix(dir).unwrap();
            // Always store forward-slashed, asset-root-relative paths so the handles built at
            // runtime resolve identically on every platform (including wasm).
            let string_path = path
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");

            let Some(ext) = full_path.extension().and_then(OsStr::to_str) else {
                continue;
            };
            if !ALLOWED_EXTENSIONS.contains(&ext) {
                continue;
            }

            let locale = path.file_stem().unwrap().to_string_lossy().into_owned();
            let family = path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();

            files.push(FontAsset {
                is_fallback: locale == "fallback",
                path: PathBuf::from(string_path),
                family,
                locale,
                ext: ext.to_owned(),
            });
        }
    }

    let mut families: Vec<FontFamily> = Vec::new();
    for asset in files.iter() {
        if let Some(family) = families.iter_mut().find(|f| f.folder == asset.family) {
            family
                .locales
                .push(format!("{}.{}", asset.locale, asset.ext));
        } else {
            families.push(FontFamily {
                path: asset.path.parent().unwrap().to_string_lossy().to_string(),
                folder: asset.family.clone(),
                locales: if asset.is_fallback {
                    vec![]
                } else {
                    vec![format!("{}.{}", asset.locale, asset.ext)]
                },
            });
        }
    }

    marker_file
        .write_all(
            format!(
                r#"#[derive(Debug)]
pub(crate) struct FontFamily {{
    pub path: &'static str,
    pub family: &'static str,
    pub locales: &'static [&'static str],
}}

{}
pub(crate) const FONT_FAMILIES: &[FontFamily] = &[{}];
"#,
                families
                    .iter()
                    .map(|s| s.write())
                    .collect::<Vec<_>>()
                    .join("\n"),
                families
                    .iter()
                    .map(|s| s.push_const())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .as_bytes(),
        )
        .unwrap();
}

/// Locates the asset folder that holds the `locales` and font files.
///
/// Resolution order:
/// 1. The `BEVY_ASSET_PATH` environment variable (recommended for workspace projects, where the
///    heuristic below cannot know which crate's `assets` folder Bevy will load at runtime).
/// 2. The `assets` (or `imported_assets/Default`) folder next to the cargo `target` directory.
///
/// Returns `None` when building on docs.rs or when no folder can be found.
fn resolve_asset_dir() -> Option<PathBuf> {
    // 1. Explicit override.
    if let Ok(value) = env::var(ASSET_PATH_VAR) {
        let path = PathBuf::from(&value);
        if path.exists() {
            return Some(path);
        }
        cargo_emit::warning!(
            "${} points to an unknown folder: {}",
            ASSET_PATH_VAR,
            path.to_string_lossy()
        );
    }

    // 2. Walk up from OUT_DIR to the cargo `target` directory and use the sibling assets folder.
    let candidate = env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .and_then(|out| {
            out.ancestors()
                .find(|ancestor| ancestor.file_name() == Some(OsStr::new("target")))
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        })
        .map(|root| {
            let imported = root.join("imported_assets");
            if imported.exists() {
                imported.join("Default")
            } else {
                root.join("assets")
            }
        });
    if let Some(dir) = candidate {
        if dir.exists() {
            return Some(dir);
        }
    }

    // We're building the docs, so an empty translation set is expected.
    if env::var("DOCS_RS").is_ok() {
        return None;
    }

    cargo_emit::warning!(
        "bevy_simple_i18n could not locate an `assets` folder, so translations and dynamic fonts \
         will be empty. Set the `{}` environment variable to your assets directory. This is \
         required for workspace-structured projects; see the README for details.",
        ASSET_PATH_VAR
    );
    None
}

struct FontAsset {
    path: PathBuf,
    ext: String,
    family: String,
    locale: String,
    is_fallback: bool,
}

struct FontFamily {
    path: String,
    folder: String,
    locales: Vec<String>,
}

impl FontFamily {
    fn write(&self) -> String {
        format!(
            r#"pub(crate) const {}: FontFamily = FontFamily {{
    path: {:?},
    family: "{}",
    locales: &{:?},
}};
"#,
            self.snake_case().to_uppercase(),
            self.path,
            self.folder,
            self.locales
        )
    }

    fn push_const(&self) -> String {
        self.snake_case().to_uppercase()
    }

    fn snake_case(&self) -> String {
        let mut snake_case = String::new();
        let name = self.folder.replace(['/', '\\', '.', '-'], "_");
        let mut prev_char = '\0';
        for (i, ch) in name.chars().enumerate() {
            if ch.is_uppercase() && i > 0 && prev_char != '_' {
                snake_case.push('_');
            }
            snake_case.push(ch.to_ascii_lowercase());
            prev_char = ch;
        }
        snake_case
    }
}

fn visit_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut collected = vec![];
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                collected.append(&mut visit_dirs(&path));
            } else {
                collected.push(path);
            }
        }
    }
    collected
}
