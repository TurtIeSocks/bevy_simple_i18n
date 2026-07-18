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
///
/// Resolution walks the locale chain OUTER (requested locale, truncation parents,
/// explicit fallbacks) and the plural forms INNER (exact integer, CLDR category,
/// `other`, bare key), like i18next: the plural CATEGORY is computed with the rules
/// of the locale whose table supplies the string — an `en` fallback string for a
/// `ja` request pluralizes with English rules, since it renders English words. And a
/// translation in the requested language (even a bare, non-plural one) beats a
/// better plural form in a fallback language.
pub(crate) fn resolve_plural_key(
    i18n: &I18n,
    locale: &str,
    key: &str,
    count: &fixed_decimal::Decimal,
) -> String {
    // Exact integer sub-keys (`cats.0` beats CLDR categories, like bevy-intl).
    // `-0` normalizes to `0` for matching.
    let plain = count.to_string();
    let exact = (!plain.contains('.')).then(|| {
        let digits = if plain == "-0" { "0" } else { plain.as_str() };
        format!("{key}.{digits}")
    });

    for candidate_locale in i18n.candidate_locales(locale) {
        if let Some(exact) = &exact {
            if i18n.lookup_exact(candidate_locale, exact).is_some() {
                return exact.clone();
            }
        }

        // CLDR category with THIS locale's rules (en: 1 -> one; pl: few/many; …).
        let parsed = crate::components::utils::resolve_locale(candidate_locale, key);
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
            if i18n.lookup_exact(candidate_locale, &candidate).is_some() {
                return candidate;
            }
        }

        // `other` is the universal CLDR fallback category.
        let candidate = format!("{key}.other");
        if i18n.lookup_exact(candidate_locale, &candidate).is_some() {
            return candidate;
        }

        // Bare key in this locale: locale priority beats plural-form priority.
        if i18n.lookup_exact(candidate_locale, key).is_some() {
            return key.to_string();
        }
    }

    // Full miss: bare key goes through the usual key-echo path.
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
    fn fallback_locale_plurals_use_the_fallback_locales_categories() {
        // ja current locale, manifest fallback ["en"], ja ships NO cats keys, en has
        // cats.one/cats.other. The string that will actually render comes from the
        // EN table, so EN's plural rules must pick the form: 1 -> "cats.one"
        // ("You have 1 cat"), not ja's category (ja has only "other", which would
        // produce "You have 1 cats").
        let mut i18n = I18n::default();
        let table: Table = [(
            "en".to_string(),
            [
                ("cats.one".to_string(), "You have %{count} cat".to_string()),
                (
                    "cats.other".to_string(),
                    "You have %{count} cats".to_string(),
                ),
            ]
            .into(),
        )]
        .into();
        i18n.apply_table(table, "en", vec!["en".to_string()], true);

        assert_eq!(
            resolve_plural_key(&i18n, "ja", "cats", &f64_to_fd(1.0)),
            "cats.one"
        );
    }

    #[test]
    fn locale_priority_beats_plural_form_priority() {
        // ja ships a bare (non-plural) "cats"; en fallback ships plural forms. The
        // ja string wins: a translation in the requested language beats a better
        // plural form in a fallback language.
        let mut i18n = I18n::default();
        let table: Table = [
            (
                "ja".to_string(),
                [("cats".to_string(), "猫が%{count}匹います".to_string())].into(),
            ),
            (
                "en".to_string(),
                [("cats.one".to_string(), "You have %{count} cat".to_string())].into(),
            ),
        ]
        .into();
        i18n.apply_table(table, "en", vec!["en".to_string()], true);

        assert_eq!(
            resolve_plural_key(&i18n, "ja", "cats", &f64_to_fd(1.0)),
            "cats"
        );
    }

    #[test]
    fn negative_zero_matches_the_exact_zero_key() {
        let i18n = i18n_with(&["cats.0", "cats.other"]);
        assert_eq!(
            resolve_plural_key(&i18n, "en", "cats", &f64_to_fd(-0.0)),
            "cats.0"
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
