//! CLDR plural-key resolution (feature `plurals`).
//!
//! No file-format change: plural forms are ordinary sub-keys of the translation key,
//! flattened like any nested map. For a component with `.with_count(n)`, the key
//! resolves in this order (each candidate goes through the normal locale fallback
//! chain):
//!
//! 1. exact integer sub-key — `cats.0`, `cats.1` (only for integral counts)
//! 2. CLDR category sub-key — `cats.one`, `cats.few`, `cats.many`, …
//! 3. `cats.other`
//! 4. the bare key — `cats` (so non-plural keys keep working with a count)
//!
//! The count is also injected as the `%{count}` interpolation argument (localized
//! number formatting), unless the user supplied their own `count` argument.

use crate::prelude::I18n;

/// Picks the translation key to use for `key` with the given `count`.
pub(crate) fn resolve_plural_key(
    i18n: &I18n,
    locale: &str,
    key: &str,
    count: &fixed_decimal::Decimal,
) -> String {
    // 1. Exact integer sub-key (`cats.0` beats CLDR categories, like bevy-intl).
    let plain = count.to_string();
    if !plain.contains('.') {
        let candidate = format!("{key}.{plain}");
        if i18n.translate(locale, &candidate).is_some() {
            return candidate;
        }
    }

    // 2. CLDR category for the locale (en: 1 -> one; pl: few/many; ar: six forms…).
    let parsed = crate::components::utils::resolve_locale(locale, key);
    if let Ok(rules) = icu_plurals::PluralRules::try_new_cardinal((&parsed).into()) {
        use icu_plurals::PluralCategory as C;
        let name = match rules.category_for(count) {
            C::Zero => "zero",
            C::One => "one",
            C::Two => "two",
            C::Few => "few",
            C::Many => "many",
            C::Other => "other",
        };
        let candidate = format!("{key}.{name}");
        if i18n.translate(locale, &candidate).is_some() {
            return candidate;
        }
    }

    // 3. `other` is the universal CLDR fallback category.
    let candidate = format!("{key}.other");
    if i18n.translate(locale, &candidate).is_some() {
        return candidate;
    }

    // 4. Bare key: non-plural keys keep working with a count, and a full miss goes
    //    through the usual key-echo path.
    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::utils::f64_to_fd;
    use crate::parse::Table;

    /// I18n whose `en` table has the given flat keys.
    fn i18n_with(keys: &[&str]) -> I18n {
        let mut i18n = I18n::default();
        let table: Table = [(
            "en".to_string(),
            keys.iter()
                .map(|k| (k.to_string(), format!("value-of-{k}")))
                .collect(),
        )]
        .into();
        i18n.apply_table(table, "en", Vec::new(), true);
        i18n
    }

    #[test]
    fn integral_count_prefers_the_exact_integer_sub_key() {
        let i18n = i18n_with(&["cats.0", "cats.one", "cats.other"]);
        assert_eq!(
            resolve_plural_key(&i18n, "en", "cats", &f64_to_fd(0.0)),
            "cats.0"
        );
    }

    #[test]
    fn count_resolves_to_the_cldr_category() {
        let i18n = i18n_with(&["cats.one", "cats.other"]);
        // English CLDR: 1 -> one, everything else -> other.
        assert_eq!(
            resolve_plural_key(&i18n, "en", "cats", &f64_to_fd(1.0)),
            "cats.one"
        );
        assert_eq!(
            resolve_plural_key(&i18n, "en", "cats", &f64_to_fd(2.0)),
            "cats.other"
        );
        assert_eq!(
            resolve_plural_key(&i18n, "en", "cats", &f64_to_fd(0.0)),
            "cats.other"
        );
    }

    #[test]
    fn fractional_count_skips_exact_integer_keys() {
        let i18n = i18n_with(&["cats.1", "cats.one", "cats.other"]);
        // 1.5 is not integral, so `cats.1` must not match; English CLDR puts 1.5 in
        // "other".
        assert_eq!(
            resolve_plural_key(&i18n, "en", "cats", &f64_to_fd(1.5)),
            "cats.other"
        );
    }

    #[test]
    fn missing_category_falls_back_to_other_then_bare_key() {
        let only_other = i18n_with(&["cats.other"]);
        assert_eq!(
            resolve_plural_key(&only_other, "en", "cats", &f64_to_fd(1.0)),
            "cats.other"
        );

        let bare = i18n_with(&["cats"]);
        assert_eq!(
            resolve_plural_key(&bare, "en", "cats", &f64_to_fd(1.0)),
            "cats"
        );
    }

    #[test]
    fn unknown_key_resolves_to_the_bare_key_for_the_usual_miss_echo() {
        let i18n = i18n_with(&["unrelated"]);
        assert_eq!(
            resolve_plural_key(&i18n, "en", "cats", &f64_to_fd(3.0)),
            "cats"
        );
    }
}
