//! Type-name utilities shared across stages, kept out of any single stage so
//! no stage has to reach into another's module for them.
//!
//! Today this is just alias normalization. It lived in `lexer` before the
//! modularization refactor, which forced the *parser* to import from the lexer
//! purely to canonicalize a type annotation — a name-spelling concern, not a
//! lexing one. Moving it here removes that parser→lexer coupling.

/// Normalizes a type-alias identifier to its canonical long form:
/// `Int` -> `Integer`, `Bool` -> `Boolean`, `Str` -> `String`. Every other
/// identifier (including the already-canonical `Integer`/`Float`/… and any
/// class name) passes through unchanged.
///
/// Applied by the parser when it builds a `TypeAnnotation`, so the typechecker
/// and interpreter only ever see canonical names and never have to know aliases
/// exist. Not applied during lexing itself — identifiers stay raw `Ident`s.
pub fn normalize_type_alias(name: &str) -> &str {
    match name {
        "Int" => "Integer",
        "Bool" => "Boolean",
        "Str" => "String",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_map_to_canonical_and_others_pass_through() {
        assert_eq!(normalize_type_alias("Int"), "Integer");
        assert_eq!(normalize_type_alias("Bool"), "Boolean");
        assert_eq!(normalize_type_alias("Str"), "String");
        assert_eq!(normalize_type_alias("Float"), "Float");
        assert_eq!(normalize_type_alias("MyClass"), "MyClass");
    }
}
