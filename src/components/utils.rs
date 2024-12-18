#[cfg(feature = "numbers")]
pub(super) fn f64_to_fd(value: f64) -> fixed_decimal::FixedDecimal {
    fixed_decimal::FixedDecimal::try_from_f64(value, fixed_decimal::FloatPrecision::Floating)
        .expect(format!("Failed to parse FixedDecimal from f64: {}", value).as_str())
}

#[cfg(feature = "numbers")]
pub(super) fn resolve_locale(locale: &String, label: impl ToString) -> icu_locid::Locale {
    locale
        .parse()
        .expect(format!("Invalid locale: {} for key: {}", locale, label.to_string()).as_str())
}

#[cfg(feature = "numbers")]
pub(super) fn get_formatter(
    locale: &String,
    label: impl ToString,
) -> icu_decimal::FixedDecimalFormatter {
    let label_string = label.to_string();
    let locale = resolve_locale(locale, label);
    let locale_string = locale.to_string();
    icu_decimal::FixedDecimalFormatter::try_new(&locale.into(), Default::default()).expect(
        format!(
            "Failed to create FixedDecimalFormatter for number: {} with locale: {}",
            label_string, locale_string,
        )
        .as_str(),
    )
}
