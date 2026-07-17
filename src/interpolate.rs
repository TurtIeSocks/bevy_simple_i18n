//! `%{name}` interpolation, replacing `rust_i18n::replace_patterns`.
//!
//! Parity semantics (replication spec §3): first match wins on duplicate pattern
//! names, an unmatched `%{name}` stays verbatim (including the `%`), there is no
//! escaping mechanism, and an unclosed `%{name` leaves the input untouched.
//!
//! Conscious divergence: rust-i18n's byte state machine lets a `%` ANYWHERE before a
//! `{...}` trigger a replacement (`"% foo {a}"` interpolates). We only recognize the
//! adjacent form `%{name}` — every real locale file uses that shape.

/// Replaces each `%{name}` in `input` where `name` matches an entry in `patterns`,
/// with the value at the same index in `values`.
pub(crate) fn interpolate(input: &str, patterns: &[&str], values: &[String]) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("%{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            break; // unclosed pattern: leave the remainder untouched
        };
        let name = &after[..end];
        out.push_str(&rest[..start]);
        // First index whose pattern matches wins; extra patterns without values are
        // ignored (rust-i18n zips and truncates).
        match patterns
            .iter()
            .position(|p| *p == name)
            .and_then(|i| values.get(i))
        {
            Some(value) => out.push_str(value),
            None => {
                out.push_str("%{");
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str, args: &[(&str, &str)]) -> String {
        let patterns: Vec<&str> = args.iter().map(|(k, _)| *k).collect();
        let values: Vec<String> = args.iter().map(|(_, v)| v.to_string()).collect();
        interpolate(input, &patterns, &values)
    }

    #[test]
    fn replaces_adjacent_pattern() {
        assert_eq!(
            run("Hello, %{name}!", &[("name", "world")]),
            "Hello, world!"
        );
    }

    #[test]
    fn replaces_multiple_distinct_patterns() {
        assert_eq!(
            run("%{a} + %{b} = %{c}", &[("a", "1"), ("b", "2"), ("c", "3")]),
            "1 + 2 = 3"
        );
    }

    #[test]
    fn duplicate_pattern_names_first_match_wins() {
        // Spec vector: replace_patterns("Hi %{a} %{a}", ["a","a"], ["1","2"]) == "Hi 1 1"
        assert_eq!(run("Hi %{a} %{a}", &[("a", "1"), ("a", "2")]), "Hi 1 1");
    }

    #[test]
    fn unmatched_pattern_stays_verbatim_including_percent() {
        // Spec vector: replace_patterns("Hi %{b}", ["a"], ["1"]) == "Hi %{b}"
        assert_eq!(run("Hi %{b}", &[("a", "1")]), "Hi %{b}");
        assert_eq!(run("Hi %{name}", &[]), "Hi %{name}");
    }

    #[test]
    fn unclosed_pattern_leaves_input_untouched() {
        assert_eq!(run("Hi %{name", &[("name", "x")]), "Hi %{name");
    }

    #[test]
    fn braces_without_percent_are_untouched() {
        assert_eq!(run("Hi {name}", &[("name", "x")]), "Hi {name}");
    }

    #[test]
    fn divergence_percent_must_be_adjacent() {
        // rust-i18n would produce "% foo1 bar"; we deliberately do not.
        assert_eq!(run("% foo {a} bar", &[("a", "1")]), "% foo {a} bar");
    }

    #[test]
    fn utf8_input_and_values_are_safe() {
        assert_eq!(
            run("%{name}さん、こんにちは", &[("name", "ベビー")]),
            "ベビーさん、こんにちは"
        );
        assert_eq!(run("你有%{count}隻貓", &[("count", "2")]), "你有2隻貓");
    }

    #[test]
    fn value_containing_pattern_syntax_is_not_reinterpolated() {
        assert_eq!(run("%{a}", &[("a", "%{b}"), ("b", "x")]), "%{b}");
    }
}
