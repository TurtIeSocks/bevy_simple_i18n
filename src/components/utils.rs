use rust_i18n::t;

use super::InterpolationType;

#[cfg(feature = "numbers")]
pub(super) fn f64_to_fd(value: f64) -> fixed_decimal::FixedDecimal {
    fixed_decimal::FixedDecimal::try_from_f64(value, fixed_decimal::FloatPrecision::Floating)
        .unwrap_or_else(|err| panic!("Failed to parse FixedDecimal from f64 {value}: {err}"))
}

#[cfg(feature = "numbers")]
pub(super) fn resolve_locale(locale: &str, label: impl std::fmt::Display) -> icu_locid::Locale {
    locale
        .parse()
        .unwrap_or_else(|err| panic!("Invalid locale: {locale} for key: {label}: {err}"))
}

#[cfg(feature = "numbers")]
pub(super) fn get_formatter(
    locale: &str,
    label: impl std::fmt::Display,
) -> icu_decimal::FixedDecimalFormatter {
    let locale = resolve_locale(locale, &label);
    icu_decimal::FixedDecimalFormatter::try_new(&locale.clone().into(), Default::default())
        .unwrap_or_else(|err| {
            panic!("Failed to create FixedDecimalFormatter for {label} with locale {locale}: {err}")
        })
}

pub(super) fn translate_by_key(
    locale: &str,
    key: &str,
    args: &[(String, InterpolationType)],
) -> String {
    #[cfg(feature = "numbers")]
    let fdf = get_formatter(locale, key);

    let (patterns, values): (Vec<&str>, Vec<String>) = args
        .iter()
        .map(|(k, interpolation_type)| {
            let value = match interpolation_type {
                InterpolationType::String(v) => v.clone(),
                #[cfg(feature = "numbers")]
                InterpolationType::Number(v) => fdf.format_to_string(v),
            };
            (k.as_str(), value)
        })
        .unzip();
    let translated = t!(key, locale = locale);

    rust_i18n::replace_patterns(&translated, patterns.as_slice(), values.as_slice())
}
