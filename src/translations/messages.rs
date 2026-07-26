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
//! Every `format!`-built error message in `lexer/`, `parser/`, `resolver/`,
//! and `interpreter/` has been converted to a catalog entry (see
//! `src/translations/CLAUDE.md`); `typechecker/` and `diagnostics/` are
//! covered separately. A handful of non-error-message `format!` call sites
//! remain untouched on purpose (stack-frame labels like `{class}.new`,
//! compound type-name building like `Ptr<{inner}>`, test-only scratch
//! strings) — those aren't user-facing diagnostic prose. Adding a new
//! catalog entry never removes a stage's ability to also just `format!` a
//! message directly; both styles coexist where conversion hasn't reached yet.

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
    (
        "type/method-arity-mismatch",
        "`{0}#{1}` expects {2} argument(s), found {3}",
    ),
    (
        "type/array-size-expects-array",
        "`Array#size` expects an array, found `{0}`",
    ),
    (
        "type/array-push-mismatch",
        "`Array<{0}>#push` expects `{0}`, found `{1}`",
    ),
    (
        "type/array-push-expects-array",
        "`Array#push` expects an array, found `{0}`",
    ),
    (
        "type/array-get-index-not-integer",
        "`Array#get` index must be `Integer`, found `{0}`",
    ),
    (
        "type/array-get-expects-array",
        "`Array#get` expects an array, found `{0}`",
    ),
    (
        "type/array-set-index-not-integer",
        "`Array#set` index must be `Integer`, found `{0}`",
    ),
    (
        "type/array-set-mismatch",
        "`Array<{0}>#set` expects `{0}`, found `{1}`",
    ),
    (
        "type/array-set-expects-array",
        "`Array#set` expects an array, found `{0}`",
    ),
    (
        "type/array-pop-expects-array",
        "`Array#pop` expects an array, found `{0}`",
    ),
    (
        "type/array-is-empty-expects-array",
        "`Array#is_empty` expects an array, found `{0}`",
    ),
    (
        "type/string-size-expects-string",
        "`String#size` expects a string, found `{0}`",
    ),
    (
        "type/string-upper-expects-string",
        "`String#upper` expects a string, found `{0}`",
    ),
    (
        "type/string-lower-expects-string",
        "`String#lower` expects a string, found `{0}`",
    ),
    (
        "type/string-trim-expects-string",
        "`String#trim` expects a string, found `{0}`",
    ),
    (
        "type/string-is-empty-expects-string",
        "`String#is_empty` expects a string, found `{0}`",
    ),
    (
        "type/string-to-i-expects-string",
        "`String#to_i` expects a string, found `{0}`",
    ),
    (
        "type/string-to-f-expects-string",
        "`String#to_f` expects a string, found `{0}`",
    ),
    (
        "type/string-to-s-expects-string",
        "`String#to_s` expects a string, found `{0}`",
    ),
    (
        "type/int-to-s-expects-int",
        "`Integer#to_s` expects an integer, found `{0}`",
    ),
    (
        "type/int-to-f-expects-int",
        "`Integer#to_f` expects an integer, found `{0}`",
    ),
    (
        "type/int-abs-expects-int",
        "`Integer#abs` expects an integer, found `{0}`",
    ),
    (
        "type/float-to-s-expects-float",
        "`Float#to_s` expects a float, found `{0}`",
    ),
    (
        "type/float-to-i-expects-float",
        "`Float#to_i` expects a float, found `{0}`",
    ),
    (
        "type/float-abs-expects-float",
        "`Float#abs` expects a float, found `{0}`",
    ),
    (
        "type/bool-to-s-expects-bool",
        "`Boolean#to_s` expects a boolean, found `{0}`",
    ),
    (
        "type/ptr-deref-expects-ptr",
        "`Ptr#deref` expects a pointer, found `{0}`",
    ),
    (
        "type/ptr-set-deref-mismatch",
        "`Ptr<{0}>#set_deref` expects `{0}`, found `{1}`",
    ),
    (
        "type/ptr-set-deref-expects-ptr",
        "`Ptr#set_deref` expects a pointer, found `{0}`",
    ),
    (
        "type/ptr-free-expects-ptr",
        "`Ptr#free` expects a pointer, found `{0}`",
    ),
    ("type/undefined-function", "undefined function `{0}`"),
    (
        "type/function-arity-mismatch",
        "function `{0}` expects {1} argument(s), found {2}",
    ),
    (
        "type/argument-type-mismatch",
        "argument to `{0}` expects `{1}`, found `{2}`",
    ),
    (
        "type/call-arity-mismatch",
        "`{0}` expects {1} argument(s), found {2}",
    ),
    ("type/len-expects-array", "`len` expects an array, found `{0}`"),
    (
        "type/push-onto-mismatch",
        "`push` onto `Array<{0}>` expects `{0}`, found `{1}`",
    ),
    (
        "type/push-expects-array",
        "`push` expects an array, found `{0}`",
    ),
    (
        "type/get-index-not-integer",
        "`get` index must be Integer, found `{0}`",
    ),
    ("type/get-expects-array", "`get` expects an array, found `{0}`"),
    (
        "type/set-index-not-integer",
        "`set` index must be Integer, found `{0}`",
    ),
    (
        "type/set-onto-mismatch",
        "`set` onto `Array<{0}>` expects `{0}`, found `{1}`",
    ),
    (
        "type/set-expects-array",
        "`set` expects an array, found `{0}`",
    ),
    ("type/pop-expects-array", "`pop` expects an array, found `{0}`"),
    (
        "type/deref-expects-pointer",
        "`deref` expects a pointer, found `{0}`",
    ),
    (
        "type/set-deref-into-mismatch",
        "`set_deref` into `Ptr<{0}>` expects `{0}`, found `{1}`",
    ),
    (
        "type/set-deref-expects-pointer",
        "`set_deref` expects a pointer, found `{0}`",
    ),
    (
        "type/free-expects-pointer",
        "`free` expects a pointer, found `{0}`",
    ),
    (
        "type/builtin-arity-mismatch",
        "`{0}` expects {1} argument(s), found {2}",
    ),
    (
        "type/class-const-requires-annotation",
        "class constants require an explicit type annotation",
    ),
    (
        "type/unknown-parent-class",
        "class `{0}` inherits from unknown class `{1}`",
    ),
    (
        "type/inheritance-cycle",
        "inheritance cycle: class `{0}` has a circular parent chain",
    ),
    (
        "type/cannot-access-field",
        "cannot access field `{0}` on `{1}`",
    ),
    ("type/class-has-no-field", "class `{0}` has no field `{1}`"),
    (
        "type/cannot-call-method",
        "cannot call method `{0}` on `{1}`",
    ),
    ("type/class-has-no-method", "class `{0}` has no method `{1}`"),
    (
        "type/class-has-no-static-method",
        "class `{0}` has no static method `{1}`",
    ),
    (
        "type/no-initializer-takes-no-args",
        "class `{0}` has no initializer, so `.new` takes no arguments",
    ),
    (
        "type/method-return-type-mismatch",
        "method `{0}#{1}` declared to return `{2}`, but returns `{3}`",
    ),
    (
        "type/field-never-assigned",
        "field `{0}` of class `{1}` is never assigned in `initializer` (it would be `Nil` at runtime, not `{2}`)",
    ),
    (
        "type/var-decl-type-mismatch",
        "type mismatch for `{0}`: declared `{1}`, found `{2}`",
    ),
    (
        "type/function-return-type-mismatch",
        "function `{0}` declared to return `{1}`, but returns `{2}`",
    ),
    (
        "type/if-condition-must-be-boolean",
        "`if` condition must be Boolean, found `{0}`",
    ),
    (
        "type/elsif-condition-must-be-boolean",
        "`elsif` condition must be Boolean, found `{0}`",
    ),
    (
        "type/while-condition-must-be-boolean",
        "`while` condition must be Boolean, found `{0}`",
    ),
    (
        "type/cannot-assign-field",
        "cannot assign `{0}` to field `{1}` of type `{2}`",
    ),
    (
        "type/branches-return-different-types",
        "branches of `if` return different types: `{0}` vs `{1}`",
    ),
    ("type/cannot-negate", "cannot negate `{0}`"),
    (
        "type/array-elements-must-share-type",
        "array elements must share one type: found `{0}` and `{1}`",
    ),
    (
        "type/array-index-must-be-integer",
        "array index must be Integer, found `{0}`",
    ),
    ("type/cannot-index-into", "cannot index into `{0}`"),
    (
        "type/cannot-apply-binop",
        "cannot apply `{0}` to `{1}` and `{2}`",
    ),
    ("diag/lex-error", "lex error"),
    ("diag/parse-error", "parse error"),
    ("diag/type-error", "type error"),
    ("diag/runtime-error", "runtime error"),
    ("diag/import-error", "import error"),
    (
        "diag/keyword-translation-error",
        "keyword translation error",
    ),
    ("diag/frame-in", "in"),
    ("diag/frame-at", "at"),
    (
        "runtime/expected-boolean-condition",
        "expected Boolean condition, found `{0}`",
    ),
    ("runtime/expected-integer", "expected Integer, found `{0}`"),
    ("runtime/cannot-negate", "cannot negate `{0}`"),
    (
        "runtime/field-on-non-object",
        "cannot access field `{0}` on a non-object value",
    ),
    (
        "runtime/class-has-no-field",
        "class `{0}` has no field `{1}`",
    ),
    (
        "runtime/array-index-out-of-bounds",
        "array index {0} out of bounds (length {1})",
    ),
    ("runtime/cannot-index-into", "cannot index into `{0}`"),
    ("runtime/cannot-add", "cannot add `{0}` and `{1}`"),
    (
        "runtime/cannot-subtract",
        "cannot subtract `{0}` from `{1}`",
    ),
    ("runtime/cannot-multiply", "cannot multiply `{0}` and `{1}`"),
    ("runtime/cannot-divide", "cannot divide `{0}` by `{1}`"),
    ("runtime/cannot-compare", "cannot compare `{0}` and `{1}`"),
    (
        "runtime/deref-expects-pointer",
        "`deref` expects a pointer, found `{0}`",
    ),
    (
        "runtime/use-after-free",
        "use after free: pointer ptr#{0} was already freed",
    ),
    ("runtime/invalid-pointer", "invalid pointer ptr#{0}"),
    (
        "runtime/set-deref-expects-pointer",
        "`set_deref` expects a pointer, found `{0}`",
    ),
    (
        "runtime/free-expects-pointer",
        "`free` expects a pointer, found `{0}`",
    ),
    (
        "runtime/double-free",
        "double free: pointer ptr#{0} was already freed",
    ),
    (
        "runtime/len-expects-array",
        "`len` expects an array, found `{0}`",
    ),
    (
        "runtime/push-expects-array",
        "`push` expects an array, found `{0}`",
    ),
    (
        "runtime/set-expects-array",
        "`set` expects an array, found `{0}`",
    ),
    (
        "runtime/pop-expects-array",
        "`pop` expects an array, found `{0}`",
    ),
    ("runtime/undefined-function", "undefined function `{0}`"),
    (
        "runtime/method-on-non-object",
        "cannot call method `{0}` on a non-object value",
    ),
    (
        "runtime/class-has-no-method",
        "class `{0}` has no method `{1}`",
    ),
    (
        "runtime/field-assign-on-non-object",
        "cannot assign field `{0}` on a non-object value",
    ),
    (
        "runtime/cannot-parse-as-integer",
        "cannot parse `{0}` as an Integer",
    ),
    (
        "runtime/cannot-parse-as-float",
        "cannot parse `{0}` as a Float",
    ),
    ("lex/invalid-float-literal", "invalid float literal `{0}`"),
    (
        "lex/invalid-integer-literal",
        "invalid integer literal `{0}`",
    ),
    (
        "lex/invalid-escape-sequence",
        "invalid escape sequence `\\{0}`",
    ),
    ("lex/unexpected-character", "unexpected character `{0}`"),
    ("parse/unexpected-token", "unexpected token {0}"),
    (
        "resolve/cannot-resolve-import",
        "cannot resolve import `{0}`: {1}",
    ),
    ("resolve/import-cycle-at", "import cycle detected at `{0}`"),
    (
        "resolve/cannot-read-imported-file",
        "cannot read imported file `{0}`: {1}",
    ),
    ("resolve/lex-error-in", "lex error in `{0}`: {1}"),
    ("resolve/parse-error-in", "parse error in `{0}`: {1}"),
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
