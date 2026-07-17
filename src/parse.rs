//! Parsing of locale files into per-locale flat `key -> text` tables.
//!
//! Behavior-ports the rust-i18n v3 file semantics (see
//! `docs/systematic-refactor/research/rust-i18n-replication-spec.md`):
//!
//! - **v1**: the whole file holds translations for ONE locale, derived from the file
//!   stem (`en.json` -> `en`, `app.en.yml` -> `en`). Nested maps flatten to dot keys.
//! - **v2** (`_version: 2` at the root): each entry maps a key to `locale -> text`
//!   pairs; nested maps recurse with dot-joined key prefixes.
//!
//! Conscious divergences from rust-i18n (documented in docs/systematic-refactor/map.md):
//! `_version` is stripped from v1 tables instead of leaking as a translation key.

use std::collections::HashMap;

/// `locale -> (flat key -> text)`
pub(crate) type Table = HashMap<String, HashMap<String, String>>;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unsupported locale file extension: {0}")]
    UnsupportedExtension(String),
    #[error("failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(feature = "yaml")]
    #[error("failed to parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[cfg(feature = "toml")]
    #[error("failed to parse TOML: {0}")]
    Toml(#[from] serde_toml::de::Error),
    #[error("invalid v2 locale file: no `key: {{locale: text}}` entries found")]
    EmptyV2,
    #[error("locale file is not valid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// Parses one locale file (raw bytes + its file stem and extension) into a [`Table`].
pub(crate) fn parse_file(stem: &str, ext: &str, bytes: &[u8]) -> Result<Table, ParseError> {
    let value: serde_json::Value = match ext {
        "json" => serde_json::from_slice(bytes)?,
        #[cfg(feature = "yaml")]
        "yml" | "yaml" => serde_yaml::from_slice(bytes)?,
        #[cfg(feature = "toml")]
        "toml" => serde_toml::from_str(std::str::from_utf8(bytes)?)?,
        other => return Err(ParseError::UnsupportedExtension(other.to_string())),
    };

    // rust-i18n parity: v2 only when `_version` is the NUMBER 2 (`as_u64() == 2`).
    if value.get("_version").and_then(serde_json::Value::as_u64) == Some(2) {
        parse_v2(&value)
    } else {
        Ok(parse_v1(stem, &value))
    }
}

/// Merges `src` into `dst`; on conflicting `(locale, key)` the `src` value wins.
pub(crate) fn merge_into(dst: &mut Table, src: Table) {
    for (locale, keys) in src {
        dst.entry(locale).or_default().extend(keys);
    }
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// v1: the file stem's last dot segment is the locale; the body flattens to dot keys.
fn parse_v1(stem: &str, value: &serde_json::Value) -> Table {
    let locale = stem.split('.').next_back().unwrap_or(stem);
    let mut keys = HashMap::new();
    if let Some(map) = value.as_object() {
        for (k, v) in map {
            if k == "_version" {
                continue; // divergence: don't leak the format marker as a translation
            }
            flatten_v1(&join("", k), v, &mut keys);
        }
    }
    Table::from([(locale.to_string(), keys)])
}

fn flatten_v1(key: &str, value: &serde_json::Value, out: &mut HashMap<String, String>) {
    use serde_json::Value::*;
    match value {
        Object(map) => {
            for (k, v) in map {
                flatten_v1(&join(key, k), v, out);
            }
        }
        // Leaf coercion, rust-i18n parity: Null/Array -> "", Bool/Number -> Display.
        String(s) => drop(out.insert(key.to_string(), s.clone())),
        Null | Array(_) => drop(out.insert(key.to_string(), std::string::String::new())),
        Bool(b) => drop(out.insert(key.to_string(), b.to_string())),
        Number(n) => drop(out.insert(key.to_string(), n.to_string())),
    }
}

/// v2: walk the tree; a STRING leaf means "path so far = key, last segment = locale".
/// Non-string, non-map leaves are ignored (rust-i18n only consumes strings here).
fn parse_v2(value: &serde_json::Value) -> Result<Table, ParseError> {
    fn walk(prefix: &str, value: &serde_json::Value, out: &mut Table) {
        if let Some(map) = value.as_object() {
            for (k, v) in map {
                if prefix.is_empty() && k == "_version" {
                    continue;
                }
                match v {
                    serde_json::Value::String(text) if !prefix.is_empty() => {
                        out.entry(k.clone())
                            .or_default()
                            .insert(prefix.to_string(), text.clone());
                    }
                    serde_json::Value::Object(_) => walk(&join(prefix, k), v, out),
                    _ => {}
                }
            }
        }
    }

    let mut table = Table::new();
    walk("", value, &mut table);
    if table.is_empty() {
        return Err(ParseError::EmptyV2);
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(stem: &str, s: &str) -> Table {
        parse_file(stem, "json", s.as_bytes()).unwrap()
    }

    #[test]
    fn v1_uses_file_stem_as_locale() {
        let t = json("en", r#"{"hello": "Hello World"}"#);
        assert_eq!(t["en"]["hello"], "Hello World");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn v1_locale_is_last_dot_segment_of_stem() {
        let t = json("app.en", r#"{"hello": "x"}"#);
        assert!(t.contains_key("en"));
        assert!(!t.contains_key("app.en"));
    }

    #[test]
    fn v1_flattens_nested_maps_to_dot_keys() {
        let t = json("en", r#"{"a": {"b": {"c": "deep"}}, "top": "flat"}"#);
        assert_eq!(t["en"]["a.b.c"], "deep");
        assert_eq!(t["en"]["top"], "flat");
    }

    #[test]
    fn v1_literal_dot_keys_and_nested_maps_are_equivalent() {
        let t = json("en", r#"{"messages.hello": "Hi"}"#);
        assert_eq!(t["en"]["messages.hello"], "Hi");
    }

    #[test]
    fn v1_coerces_scalar_leaves_like_rust_i18n() {
        let t = json(
            "en",
            r#"{"n": null, "b": true, "f": false, "i": 1, "fl": 1.5, "arr": [1, 2]}"#,
        );
        assert_eq!(t["en"]["n"], "");
        assert_eq!(t["en"]["b"], "true");
        assert_eq!(t["en"]["f"], "false");
        assert_eq!(t["en"]["i"], "1");
        assert_eq!(t["en"]["fl"], "1.5");
        assert_eq!(t["en"]["arr"], "");
    }

    #[test]
    fn v1_strips_the_version_marker_key() {
        // Divergence from rust-i18n, which leaks `_version -> "1"` as a translation.
        let t = json("en", r#"{"_version": 1, "hello": "x"}"#);
        assert!(!t["en"].contains_key("_version"));
        assert_eq!(t["en"]["hello"], "x");
    }

    #[test]
    fn v2_detected_by_numeric_version_2() {
        let t = json(
            "anything",
            r#"{"_version": 2, "hello": {"en": "Hello", "ja": "こんにちは"}}"#,
        );
        assert_eq!(t["en"]["hello"], "Hello");
        assert_eq!(t["ja"]["hello"], "こんにちは");
        assert!(!t.contains_key("anything"), "v2 must ignore the file stem");
    }

    #[test]
    fn v2_nested_maps_become_dot_joined_keys() {
        let t = json(
            "x",
            r#"{"_version": 2, "messages": {"hello": {"en": "Hi", "de": "Hallo"}}}"#,
        );
        assert_eq!(t["en"]["messages.hello"], "Hi");
        assert_eq!(t["de"]["messages.hello"], "Hallo");
    }

    #[test]
    fn v2_with_no_entries_is_an_error() {
        let err = parse_file("x", "json", br#"{"_version": 2}"#).unwrap_err();
        assert!(matches!(err, ParseError::EmptyV2));
    }

    #[test]
    fn string_version_2_is_not_v2() {
        // rust-i18n detects v2 via `as_u64() == 2`; the STRING "2" stays v1.
        let t = json("en", r#"{"_version": "2", "hello": {"ja": "x"}}"#);
        assert_eq!(t["en"]["hello.ja"], "x");
    }

    #[test]
    fn parse_error_reported_not_panicking() {
        assert!(parse_file("en", "json", b"{not json").is_err());
        assert!(matches!(
            parse_file("en", "wgsl", b"x").unwrap_err(),
            ParseError::UnsupportedExtension(_)
        ));
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn yaml_v2_parses() {
        let src = "_version: 2\nhello:\n  en: Hello world\n  zh-TW: 你好世界\n";
        let t = parse_file("v2_example", "yml", src.as_bytes()).unwrap();
        assert_eq!(t["en"]["hello"], "Hello world");
        assert_eq!(t["zh-TW"]["hello"], "你好世界");
    }

    #[cfg(feature = "toml")]
    #[test]
    fn toml_v1_parses() {
        let src = "hello = \"Hallo Welt\"\n[messages]\nbye = \"Tschüss\"\n";
        let t = parse_file("de", "toml", src.as_bytes()).unwrap();
        assert_eq!(t["de"]["hello"], "Hallo Welt");
        assert_eq!(t["de"]["messages.bye"], "Tschüss");
    }

    #[test]
    fn merge_later_wins_on_scalar_conflicts_and_unions_the_rest() {
        let mut dst = json("en", r#"{"hello": "Hello World", "only_v1": "keep"}"#);
        let src = json(
            "x",
            r#"{"_version": 2, "hello": {"en": "Hello world", "ja": "こんにちは世界"}}"#,
        );
        merge_into(&mut dst, src);
        assert_eq!(dst["en"]["hello"], "Hello world"); // later file wins
        assert_eq!(dst["en"]["only_v1"], "keep"); // untouched keys survive
        assert_eq!(dst["ja"]["hello"], "こんにちは世界"); // new locales appended
    }
}
