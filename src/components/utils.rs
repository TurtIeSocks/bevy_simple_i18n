use super::InterpolationType;
use crate::prelude::I18n;

#[cfg(feature = "numbers")]
pub(crate) fn f64_to_fd(value: f64) -> fixed_decimal::Decimal {
    fixed_decimal::Decimal::try_from_f64(value, fixed_decimal::FloatPrecision::RoundTrip)
        .unwrap_or_else(|err| panic!("Failed to parse Decimal from f64 {value}: {err}"))
}

#[cfg(feature = "numbers")]
pub(crate) fn resolve_locale(
    locale: &str,
    label: impl std::fmt::Display,
) -> icu_locale_core::Locale {
    locale
        .parse()
        .unwrap_or_else(|err| panic!("Invalid locale: {locale} for key: {label}: {err}"))
}

#[cfg(feature = "numbers")]
pub(super) fn get_formatter(
    locale: &str,
    label: impl std::fmt::Display,
) -> icu_decimal::DecimalFormatter {
    let locale = resolve_locale(locale, &label);
    icu_decimal::DecimalFormatter::try_new((&locale).into(), Default::default()).unwrap_or_else(
        |err| panic!("Failed to create DecimalFormatter for {label} with locale {locale}: {err}"),
    )
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
    values.push(get_formatter(locale, key).format_to_string(count));
    translate_resolved(i18n, locale, &resolved, patterns, values)
}

fn build_args<'a>(
    locale: &str,
    key: &str,
    args: &'a [(String, InterpolationType)],
) -> (Vec<&'a str>, Vec<String>) {
    #[cfg(not(feature = "numbers"))]
    let _ = (locale, key);
    #[cfg(feature = "numbers")]
    let fdf = get_formatter(locale, key);

    args.iter()
        .map(|(k, interpolation_type)| {
            let value = match interpolation_type {
                InterpolationType::String(v) => v.clone(),
                #[cfg(feature = "numbers")]
                InterpolationType::Number(v) => fdf.format_to_string(v),
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
