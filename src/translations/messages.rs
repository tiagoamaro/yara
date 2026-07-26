//! The English message catalog: one entry per user-facing diagnostic message
//! produced anywhere in the pipeline, keyed by a stable `stage/kebab-name`
//! string and templated with positional `{0}`, `{1}`, ... placeholders.
//!
//! This is the fallback tier of [`super::Vocabulary::msg`]: a translated
//! vocabulary file's `[messages]` section only needs to override the keys a
//! learner will actually see translated; any key it doesn't mention falls
//! back to the English template here. English output is therefore always
//! exactly this table, substituted — which is what keeps `tests/golden/*.stderr`
//! and every `.contains()`/`assert_eq!` message assertion byte-identical
//! across this whole refactor: converting a `format!` call site to
//! `vocab.msg("key", &[...])` must reproduce the old string exactly.
//!
//! Only a representative subset of call sites has been converted to route
//! through this catalog so far (see `src/translations/CLAUDE.md` for the
//! rest of the story and what remains) — every stage has at least one
//! converted message, enough to prove the mechanism end-to-end (including a
//! fully-Portuguese example and error golden), but the full ~138-site sweep
//! described in the project's translation plan is not complete. Adding a new
//! catalog entry never removes a stage's ability to also just `format!` a
//! message directly; both styles coexist during the migration.

/// `(key, English template)` pairs. Keys are namespaced by stage
/// (`lex/`, `parse/`, `resolve/`, `type/`, `runtime/`) so two stages can
/// reuse the same short name without colliding.
pub const MESSAGES: &[(&str, &str)] = &[
    ("lex/unterminated-string", "unterminated string literal"),
    ("parse/expected-found", "expected {0}, found {1}"),
    (
        "parse/expected-identifier-found",
        "expected identifier, found {0}",
    ),
    (
        "parse/unexpected-eof-expected-end",
        "unexpected end of input, expected `end`",
    ),
    ("resolve/import-cycle", "import cycle detected: `{0}`"),
    ("type/undefined-variable", "undefined variable `{0}`"),
    ("type/unknown-type", "unknown type `{0}`"),
    (
        "type/no-method-available",
        "`{0}` has no method `{1}` (available: {2})",
    ),
    ("runtime/undefined-variable", "undefined variable `{0}`"),
    ("runtime/division-by-zero", "division by zero"),
    (
        "runtime/no-method-for-value",
        "no method `{0}` for this value",
    ),
];

/// Substitutes `{0}`, `{1}`, ... in `template` with `args`, in order.
/// Hand-rolled (no format-string crate) to match the project's
/// zero-dependency stance — same rationale as `translations::parse_keyword_file`.
pub fn substitute(template: &str, args: &[&str]) -> String {
    let mut result = template.to_string();
    for (i, arg) in args.iter().enumerate() {
        result = result.replace(&format!("{{{i}}}"), arg);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_positional_placeholders() {
        assert_eq!(
            substitute("expected {0}, found {1}", &["`)`", "`if`"]),
            "expected `)`, found `if`"
        );
    }

    #[test]
    fn leaves_template_without_placeholders_untouched() {
        assert_eq!(substitute("division by zero", &[]), "division by zero");
    }

    #[test]
    fn message_keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (key, _) in MESSAGES {
            assert!(seen.insert(key), "duplicate message key: {key}");
        }
    }
}
