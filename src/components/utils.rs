use super::InterpolationType;
use crate::prelude::I18n;

/// Non-panicking f64 -> Decimal conversion: a NaN/infinite value is a data bug worth
/// an error log, not a crash. Used by `with_count`, `with_num_arg`, and
/// `I18nNumber::new`. `plurals` already implies `numbers` (see `Cargo.toml`), so
/// gating on `numbers` alone covers both.
#[cfg(feature = "numbers")]
pub(crate) fn try_f64_to_fd(value: f64) -> Option<fixed_decimal::Decimal> {
    fixed_decimal::Decimal::try_from_f64(value, fixed_decimal::FloatPrecision::RoundTrip)
        .map_err(|err| bevy::log::error!("Ignoring non-finite number {value}: {err}"))
        .ok()
}

/// Non-panicking locale parse: an unparseable tag (manifest `fallback` data, or a
/// mistyped `.with_locale(...)`) is a data bug worth an error log, not a crash.
#[cfg(feature = "numbers")]
pub(crate) fn resolve_locale(
    locale: &str,
    label: impl std::fmt::Display,
) -> Option<icu_locale_core::Locale> {
    locale
        .parse()
        .map_err(|err| bevy::log::error!("Invalid locale: {locale} for key: {label}: {err}"))
        .ok()
}

#[cfg(feature = "numbers")]
pub(super) fn get_formatter(
    locale: &str,
    label: impl std::fmt::Display,
) -> Option<icu_decimal::DecimalFormatter> {
    let locale = resolve_locale(locale, &label)?;
    icu_decimal::DecimalFormatter::try_new((&locale).into(), Default::default())
        .map_err(|err| {
            bevy::log::error!(
                "Failed to create DecimalFormatter for {label} with locale {locale}: {err}"
            )
        })
        .ok()
}

pub(super) fn translate_by_key(
    i18n: &I18n,
    locale: &str,
    key: &str,
    args: &[(String, InterpolationType)],
) -> String {
    let (patterns, values) = build_args(locale, key, args);
    translate_resolved(i18n, locale, key, patterns, values)
}

/// Like [`translate_by_key`], but first resolves `key` to its plural form for
/// `count`, and injects `%{count}` (localized) unless the caller supplied one —
/// user-provided args come first, and interpolation is first-match-wins.
#[cfg(feature = "plurals")]
pub(super) fn translate_plural(
    i18n: &I18n,
    locale: &str,
    key: &str,
    args: &[(String, InterpolationType)],
    count: &fixed_decimal::Decimal,
) -> String {
    let resolved = crate::plural::resolve_plural_key(i18n, locale, key, count);
    let (mut patterns, mut values) = build_args(locale, key, args);
    patterns.push("count");
    values.push(
        get_formatter(locale, key)
            .map(|f| f.format_to_string(count))
            .unwrap_or_else(|| count.to_string()),
    );
    translate_resolved(i18n, locale, &resolved, patterns, values)
}

fn build_args<'a>(
    locale: &str,
    key: &str,
    args: &'a [(String, InterpolationType)],
) -> (Vec<&'a str>, Vec<String>) {
    #[cfg(not(feature = "numbers"))]
    let _ = (locale, key);
    // Built lazily, at most once: zero formatter construction when there are no
    // number args at all (the common case), and a memoized `None` (rather than
    // retrying) if construction failed once for this locale.
    #[cfg(feature = "numbers")]
    let mut fdf: Option<Option<icu_decimal::DecimalFormatter>> = None;

    args.iter()
        .map(|(k, interpolation_type)| {
            let value = match interpolation_type {
                InterpolationType::String(v) => v.clone(),
                #[cfg(feature = "numbers")]
                InterpolationType::Number(v) => fdf
                    .get_or_insert_with(|| get_formatter(locale, key))
                    .as_ref()
                    .map(|f| f.format_to_string(v))
                    .unwrap_or_else(|| v.to_string()),
            };
            (k.as_str(), value)
        })
        .unzip()
}

fn translate_resolved(
    i18n: &I18n,
    locale: &str,
    key: &str,
    patterns: Vec<&str>,
    values: Vec<String>,
) -> String {
    // rust-i18n parity: a complete miss renders the key verbatim, and interpolation
    // still applies to it.
    let translated = match i18n.translate(locale, key) {
        Some(text) => text,
        None => {
            if i18n.ready() {
                bevy::log::warn!("Missing translation for key `{key}` (locale `{locale}`)");
            } else {
                bevy::log::debug!(
                    "Translation for key `{key}` requested before locale assets loaded"
                );
            }
            key
        }
    };

    crate::interpolate::interpolate(translated, &patterns, &values)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the `Display` fallback used when a `DecimalFormatter` can't be built:
    /// it must render plain digits, not scientific notation.
    #[cfg(feature = "numbers")]
    #[test]
    fn decimal_display_fallback_is_plain_notation() {
        let d = try_f64_to_fd(2503.1).expect("2503.1 is finite");
        assert_eq!(d.to_string(), "2503.1");
    }
}
