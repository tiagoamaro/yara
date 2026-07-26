//! Parses vocabulary-translation files: plain-text, sectioned `canonical =
//! localized` mappings that let a learner write Yara in another language —
//! not just its 15 reserved words, but type names, builtins, primitive
//! methods, and (for a subset of messages so far) error prose too. See
//! `examples/translations/CLAUDE.md` and `translations/CLAUDE.md` for the
//! end-to-end story; this module owns turning a vocabulary file's text into
//! a [`Vocabulary`].

use crate::builtins::BUILTINS;
use crate::lexer::{default_keywords, KeywordToken};
use crate::methods::METHODS;
use crate::typechecker::primitive_type_names;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

pub mod messages;

#[derive(Debug, Clone, PartialEq)]
pub struct TranslationError {
    pub message: String,
    /// 1-indexed line *within the translation file*, not within any `.yara`
    /// source — this error happens before the target program is even read.
    pub line: usize,
}

impl fmt::Display for TranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.line, self.message)
    }
}

impl crate::diagnostics::Diagnostic for TranslationError {
    fn kind(&self) -> &str {
        "keyword translation error"
    }
    fn message(&self) -> &str {
        &self.message
    }
    /// A translation error has a line but no meaningful column (it points at a
    /// whole mapping line in the vocabulary file), so the caret sits at column 1 —
    /// matching how the CLI rendered these before the `Diagnostic` unification.
    fn span(&self) -> crate::diagnostics::Span {
        crate::diagnostics::Span::new(self.line, 1)
    }
}

/// Which `[section]` a vocabulary-file line belongs to. Untagged lines at the
/// top of the file (no `[section]` header yet) default to `Keywords`, which is
/// what keeps a pre-sectioning file like the old `translations/pt.keywords`
/// parsing unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Keywords,
    Types,
    Builtins,
    Methods,
    Messages,
}

impl Section {
    fn from_header(name: &str) -> Option<Section> {
        match name {
            "keywords" => Some(Section::Keywords),
            "types" => Some(Section::Types),
            "builtins" => Some(Section::Builtins),
            "methods" => Some(Section::Methods),
            "messages" => Some(Section::Messages),
            _ => None,
        }
    }
}

/// A fully-resolved translation vocabulary: every localizable name/message in
/// Yara, mapped both ways (localized -> canonical, for reading a translated
/// program, and canonical -> localized, for echoing a localized spelling back
/// inside a rendered diagnostic).
///
/// [`Vocabulary::english()`] builds the untranslated default by deriving every
/// name from the *existing* single sources of truth — [`default_keywords`],
/// [`primitive_type_names`] (plus the `Int`/`Bool`/`Str` aliases, the
/// `IntArray`/`FloatArray`/`BoolArray`/`StringArray` names, and the `Array`/`Ptr`
/// compound-type display words), [`BUILTINS`] (plus `print`), and [`METHODS`]
/// (plus `new`) — rather than restating a second list of these names anywhere
/// that could drift out of sync.
#[derive(Debug)]
pub struct Vocabulary {
    /// localized spelling -> keyword token, exactly what `Lexer::with_keywords`
    /// consumes (unchanged shape from the pre-sectioning `HashMap` this struct
    /// replaces).
    pub keywords: HashMap<String, KeywordToken>,
    types: HashMap<String, String>,
    types_out: HashMap<String, String>,
    builtins: HashMap<String, String>,
    builtins_out: HashMap<String, String>,
    methods: HashMap<String, String>,
    methods_out: HashMap<String, String>,
    /// key -> localized template. Sparse: a key absent here falls back to its
    /// English template in [`messages::MESSAGES`] (see [`Vocabulary::msg`]).
    messages: HashMap<String, String>,
}

/// The compound-type display words (`Array<...>`, `Ptr<...>`) alongside the
/// primitive/array/alias type names — not real syntax on their own (there's no
/// generic `Array<T>` annotation), but they appear inside rendered type names
/// in messages, so they're translatable too.
const COMPOUND_TYPE_WORDS: &[&str] = &["Array", "Ptr"];
const TYPE_ALIASES: &[&str] = &["Int", "Bool", "Str"];
const ARRAY_TYPE_NAMES: &[&str] = &["IntArray", "FloatArray", "BoolArray", "StringArray"];

impl Vocabulary {
    /// Every canonical (English) type name a `[types]` section line's
    /// left-hand side may name: the primitives, their short aliases, the
    /// concrete array-annotation names, and the two compound-type display words.
    fn canonical_type_names() -> Vec<&'static str> {
        let mut names = primitive_type_names();
        names.extend_from_slice(TYPE_ALIASES);
        names.extend_from_slice(ARRAY_TYPE_NAMES);
        names.extend_from_slice(COMPOUND_TYPE_WORDS);
        names
    }

    /// Every canonical builtin name a `[builtins]` section line may name:
    /// every `BUILTINS` registry entry, plus `print` (handled on its own code
    /// path in each stage, so it isn't in the registry itself).
    fn canonical_builtin_names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = BUILTINS.iter().map(|b| b.name).collect();
        names.push("print");
        names
    }

    /// Every canonical method name a `[methods]` section line may name: every
    /// bare (receiver-agnostic) `METHODS` registry name, deduplicated, plus
    /// `new` (the special constructor call, not in the registry).
    fn canonical_method_names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Vec::new();
        for m in METHODS {
            if !names.contains(&m.name) {
                names.push(m.name);
            }
        }
        names.push("new");
        names
    }

    /// The untranslated default: every localized spelling equals its
    /// canonical (English) name, derived from the registries above rather than
    /// a second hardcoded list.
    pub fn english() -> Self {
        let keywords = default_keywords();
        let identity =
            |names: Vec<&'static str>| -> (HashMap<String, String>, HashMap<String, String>) {
                let map: HashMap<String, String> = names
                    .iter()
                    .map(|n| (n.to_string(), n.to_string()))
                    .collect();
                let out = map.clone();
                (map, out)
            };
        let (types, types_out) = identity(Self::canonical_type_names());
        let (builtins, builtins_out) = identity(Self::canonical_builtin_names());
        let (methods, methods_out) = identity(Self::canonical_method_names());
        Vocabulary {
            keywords,
            types,
            types_out,
            builtins,
            builtins_out,
            methods,
            methods_out,
            messages: HashMap::new(),
        }
    }

    /// Normalizes a possibly-localized type name to its canonical English
    /// spelling; passes an unrecognized name through unchanged (a user class
    /// name, or a typo the typechecker will report as "unknown type" either
    /// way). Applied by `Parser::parse_type_annotation` *before*
    /// `types::normalize_type_alias`.
    pub fn canonical_type(&self, name: &str) -> String {
        self.types
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// Normalizes a possibly-localized builtin (or `print`) name to its
    /// canonical English spelling; passes an unrecognized name through
    /// unchanged (a user-defined function name, most likely).
    pub fn canonical_builtin(&self, name: &str) -> String {
        self.builtins
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// Normalizes a possibly-localized primitive-method (or `new`) name to its
    /// canonical English spelling; passes an unrecognized name through
    /// unchanged (an instance method name, most likely).
    pub fn canonical_method(&self, name: &str) -> String {
        self.methods
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// Renders a `Type` the way [`std::fmt::Display`] would, except every
    /// primitive/array/compound-word segment is localized via the
    /// canonical -> localized reverse map — so a message built with this
    /// instead of bare `{ty}` interpolation shows e.g. `Array<Inteiro>` in a
    /// Portuguese-vocabulary program's diagnostics.
    pub fn type_name(&self, ty: &crate::typechecker::Type) -> String {
        use crate::typechecker::Type;
        match ty {
            Type::Array(elem) => format!(
                "{}<{}>",
                self.localized_type_word("Array"),
                self.type_name(elem)
            ),
            Type::Pointer(elem) => format!(
                "{}<{}>",
                self.localized_type_word("Ptr"),
                self.type_name(elem)
            ),
            Type::Instance(name) => name.clone(),
            primitive => self.localized_type_word(&primitive.to_string()),
        }
    }

    fn localized_type_word(&self, canonical: &str) -> String {
        self.types_out
            .get(canonical)
            .cloned()
            .unwrap_or_else(|| canonical.to_string())
    }

    /// Localizes a list of bare method names (e.g. from `methods::names_for`)
    /// for use inside a "no method (available: ...)"-shaped message.
    pub fn localized_method_names(&self, names: &[&str]) -> Vec<String> {
        names
            .iter()
            .map(|n| {
                self.methods_out
                    .get(*n)
                    .cloned()
                    .unwrap_or_else(|| n.to_string())
            })
            .collect()
    }

    /// The localized spelling of the `true`/`false` keyword, read back out of
    /// the keyword table (its localized-spelling -> token direction) rather
    /// than a separate boolean-word table — booleans are keywords, so this is
    /// the same vocabulary the lexer already uses.
    pub fn bool_word(&self, value: bool) -> String {
        let token = if value {
            KeywordToken::True
        } else {
            KeywordToken::False
        };
        self.keywords
            .iter()
            .find(|(_, t)| **t == token)
            .map(|(spelling, _)| spelling.clone())
            .unwrap_or_else(|| value.to_string())
    }

    /// Looks up `key`'s message template — the localized override if the
    /// vocabulary file's `[messages]` section supplied one, else the English
    /// default in [`messages::MESSAGES`] — and substitutes `args` positionally
    /// (`{0}`, `{1}`, ...). Falls back to `key` itself if `key` matches neither
    /// (a programming error: every catalog site should have an English entry).
    pub fn msg(&self, key: &str, args: &[&str]) -> String {
        let template = self.messages.get(key).cloned().or_else(|| {
            messages::MESSAGES
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, t)| t.to_string())
        });
        messages::substitute(&template.unwrap_or_else(|| key.to_string()), args)
    }
}

/// Parses a vocabulary file's contents into a [`Vocabulary`].
///
/// Format: `[section]` headers (`keywords`/`types`/`builtins`/`methods`/
/// `messages`) followed by `canonical = localized` lines, one per line (`#`
/// starts a line comment, same as Yara itself; blank lines ignored). Lines
/// before the first header default to `[keywords]`, which is what keeps a
/// pre-sectioning file (no headers at all) parsing exactly as it always did.
///
/// Each section's `canonical` must be a name that section's registry actually
/// has (a `KeywordToken::canonical_name`, a type/builtin/method name derived in
/// [`Vocabulary::english`], or a [`messages::MESSAGES`] key) — an unrecognized
/// name is a [`TranslationError`], since a typo here should fail loudly rather
/// than silently doing nothing. An empty `localized` (`if =`) is also an error,
/// as is a `localized` spelling already claimed by a *different* canonical
/// name within the same section (would make the two indistinguishable in
/// source text / prose).
pub fn parse_vocabulary(text: &str) -> Result<Vocabulary, TranslationError> {
    let mut vocab = Vocabulary::english();
    let mut section = Section::Keywords;

    // Per-section "current localized spelling" trackers, mirroring the
    // pre-sectioning keyword parser's `canonical_to_current_spelling`: needed
    // so translating a name a second time replaces its old spelling instead
    // of leaving both valid, and so a duplicate-spelling check only compares
    // within the same section.
    let mut keyword_spelling: HashMap<KeywordToken, String> = vocab
        .keywords
        .iter()
        .map(|(spelling, token)| (*token, spelling.clone()))
        .collect();
    let mut type_spelling: HashMap<String, String> = vocab
        .types_out
        .iter()
        .map(|(c, l)| (c.clone(), l.clone()))
        .collect();
    let mut builtin_spelling: HashMap<String, String> = vocab
        .builtins_out
        .iter()
        .map(|(c, l)| (c.clone(), l.clone()))
        .collect();
    let mut method_spelling: HashMap<String, String> = vocab
        .methods_out
        .iter()
        .map(|(c, l)| (c.clone(), l.clone()))
        .collect();

    for (i, raw_line) in text.lines().enumerate() {
        let line_number = i + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if let Some(header) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = Section::from_header(header.trim()).ok_or_else(|| TranslationError {
                message: format!("unknown section `[{header}]`"),
                line: line_number,
            })?;
            continue;
        }

        let Some((canonical, localized)) = line.split_once('=') else {
            return Err(TranslationError {
                message: format!("expected `canonical = localized`, found `{line}`"),
                line: line_number,
            });
        };
        let canonical = canonical.trim();
        let localized = localized.trim();

        if localized.is_empty() {
            return Err(TranslationError {
                message: format!("`{canonical}` has no translated spelling after `=`"),
                line: line_number,
            });
        }

        match section {
            Section::Keywords => {
                let Some(token) = KeywordToken::all()
                    .into_iter()
                    .find(|k| k.canonical_name() == canonical)
                else {
                    return Err(TranslationError {
                        message: format!("unknown keyword `{canonical}`"),
                        line: line_number,
                    });
                };
                if let Some(existing_owner) = vocab.keywords.get(localized) {
                    if *existing_owner != token {
                        return Err(TranslationError {
                            message: format!(
                                "`{localized}` is already used for `{}`, cannot also mean `{canonical}`",
                                existing_owner.canonical_name()
                            ),
                            line: line_number,
                        });
                    }
                }
                let old_spelling = keyword_spelling.get(&token).cloned().unwrap_or_default();
                vocab.keywords.remove(&old_spelling);
                vocab.keywords.insert(localized.to_string(), token);
                keyword_spelling.insert(token, localized.to_string());
            }
            Section::Types => translate_entry(
                canonical,
                localized,
                line_number,
                &Vocabulary::canonical_type_names(),
                &mut vocab.types,
                &mut vocab.types_out,
                &mut type_spelling,
            )?,
            Section::Builtins => translate_entry(
                canonical,
                localized,
                line_number,
                &Vocabulary::canonical_builtin_names(),
                &mut vocab.builtins,
                &mut vocab.builtins_out,
                &mut builtin_spelling,
            )?,
            Section::Methods => translate_entry(
                canonical,
                localized,
                line_number,
                &Vocabulary::canonical_method_names(),
                &mut vocab.methods,
                &mut vocab.methods_out,
                &mut method_spelling,
            )?,
            Section::Messages => {
                if !messages::MESSAGES.iter().any(|(k, _)| *k == canonical) {
                    return Err(TranslationError {
                        message: format!("unknown message key `{canonical}`"),
                        line: line_number,
                    });
                }
                vocab
                    .messages
                    .insert(canonical.to_string(), localized.to_string());
            }
        }
    }

    Ok(vocab)
}

/// Shared per-line logic for the `[types]`/`[builtins]`/`[methods]` sections:
/// validate `canonical` is a real name in that section's registry, reject a
/// `localized` spelling already claimed by a different canonical name, then
/// update the localized->canonical map, its canonical->localized reverse, and
/// the current-spelling tracker (so re-translating the same canonical name
/// replaces its old spelling rather than leaving both valid).
#[allow(clippy::too_many_arguments)]
fn translate_entry(
    canonical: &str,
    localized: &str,
    line_number: usize,
    valid_names: &[&'static str],
    map: &mut HashMap<String, String>,
    map_out: &mut HashMap<String, String>,
    current_spelling: &mut HashMap<String, String>,
) -> Result<(), TranslationError> {
    if !valid_names.contains(&canonical) {
        return Err(TranslationError {
            message: format!("unknown name `{canonical}`"),
            line: line_number,
        });
    }
    if let Some(existing_canonical) = map.get(localized) {
        if existing_canonical != canonical {
            return Err(TranslationError {
                message: format!(
                    "`{localized}` is already used for `{existing_canonical}`, cannot also mean `{canonical}`"
                ),
                line: line_number,
            });
        }
    }
    let old_spelling = current_spelling
        .get(canonical)
        .cloned()
        .unwrap_or_else(|| canonical.to_string());
    map.remove(&old_spelling);
    map.insert(localized.to_string(), canonical.to_string());
    map_out.insert(canonical.to_string(), localized.to_string());
    current_spelling.insert(canonical.to_string(), localized.to_string());
    Ok(())
}

/// Parses a vocabulary file's `[keywords]` section only, returning the
/// `HashMap<String, KeywordToken>` `Lexer::with_keywords` wants directly.
/// Kept for source-compatibility with code written against the pre-`Vocabulary`
/// API; new code should call [`parse_vocabulary`] and read `.keywords` off the
/// result (or thread the whole `Vocabulary` through, via `::with_vocabulary`
/// constructors).
pub fn parse_keyword_file(text: &str) -> Result<HashMap<String, KeywordToken>, TranslationError> {
    parse_vocabulary(text).map(|v| v.keywords)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_a_single_keyword() {
        let keywords = parse_keyword_file("if = se\n").unwrap();
        assert_eq!(keywords.get("se"), Some(&KeywordToken::If));
        assert_eq!(keywords.get("if"), None);
        // Untranslated keywords keep their English spelling.
        assert_eq!(keywords.get("end"), Some(&KeywordToken::End));
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let keywords = parse_keyword_file("# a comment\n\nif = se # trailing comment\n").unwrap();
        assert_eq!(keywords.get("se"), Some(&KeywordToken::If));
    }

    #[test]
    fn unknown_canonical_name_is_an_error() {
        let err = parse_keyword_file("iff = se\n").unwrap_err();
        assert!(err.message.contains("unknown keyword"));
        assert_eq!(err.line, 1);
    }

    #[test]
    fn duplicate_localized_spelling_is_an_error() {
        let err = parse_keyword_file("if = pal\nwhile = pal\n").unwrap_err();
        assert!(err.message.contains("already used"));
        assert_eq!(err.line, 2);
    }

    #[test]
    fn malformed_line_is_an_error() {
        let err = parse_keyword_file("this is not valid\n").unwrap_err();
        assert!(err.message.contains("expected"));
    }

    #[test]
    fn full_bundled_portuguese_file_parses() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/translations/pt.vocab"
        ))
        .unwrap();
        let vocab = parse_vocabulary(&text).unwrap();
        assert_eq!(vocab.keywords.get("se"), Some(&KeywordToken::If));
        assert_eq!(vocab.keywords.get("classe"), Some(&KeywordToken::Class));
        assert_eq!(vocab.keywords.get("verdadeiro"), Some(&KeywordToken::True));
        assert_eq!(vocab.canonical_type("Inteiro"), "Integer");
        assert_eq!(vocab.canonical_builtin("escreva"), "print");
        assert_eq!(vocab.canonical_method("tamanho"), "size");
    }

    #[test]
    fn sectioned_file_parses_types_builtins_and_methods() {
        let text = "[keywords]\nif = se\n[types]\nInteger = Inteiro\n[builtins]\nprint = escreva\n[methods]\nsize = tamanho\n";
        let vocab = parse_vocabulary(text).unwrap();
        assert_eq!(vocab.keywords.get("se"), Some(&KeywordToken::If));
        assert_eq!(vocab.canonical_type("Inteiro"), "Integer");
        assert_eq!(vocab.canonical_builtin("escreva"), "print");
        assert_eq!(vocab.canonical_method("tamanho"), "size");
    }

    #[test]
    fn untagged_lines_default_to_keywords_section() {
        // No `[keywords]` header at all -- matches every pre-sectioning file.
        let vocab = parse_vocabulary("if = se\n").unwrap();
        assert_eq!(vocab.keywords.get("se"), Some(&KeywordToken::If));
    }

    #[test]
    fn unknown_type_name_is_an_error() {
        let err = parse_vocabulary("[types]\nBogus = Fake\n").unwrap_err();
        assert!(err.message.contains("unknown name"));
    }

    #[test]
    fn unknown_builtin_name_is_an_error() {
        let err = parse_vocabulary("[builtins]\nnope = naoexiste\n").unwrap_err();
        assert!(err.message.contains("unknown name"));
    }

    #[test]
    fn unknown_method_name_is_an_error() {
        let err = parse_vocabulary("[methods]\nnope = naoexiste\n").unwrap_err();
        assert!(err.message.contains("unknown name"));
    }

    #[test]
    fn duplicate_localized_type_spelling_is_an_error() {
        let err = parse_vocabulary("[types]\nInteger = X\nFloat = X\n").unwrap_err();
        assert!(err.message.contains("already used"));
    }

    #[test]
    fn unknown_section_header_is_an_error() {
        let err = parse_vocabulary("[bogus]\nif = se\n").unwrap_err();
        assert!(err.message.contains("unknown section"));
    }

    #[test]
    fn unknown_message_key_is_an_error() {
        let err = parse_vocabulary("[messages]\nnope/nope = oops\n").unwrap_err();
        assert!(err.message.contains("unknown message key"));
    }

    #[test]
    fn message_override_takes_precedence_over_english_default() {
        let vocab =
            parse_vocabulary("[messages]\nruntime/division-by-zero = divisao por zero\n").unwrap();
        assert_eq!(
            vocab.msg("runtime/division-by-zero", &[]),
            "divisao por zero"
        );
        // An untranslated key still falls back to English.
        assert_eq!(
            vocab.msg("type/undefined-variable", &["x"]),
            "undefined variable `x`"
        );
    }

    #[test]
    fn english_vocabulary_round_trips_every_registry_name() {
        let vocab = Vocabulary::english();
        for name in Vocabulary::canonical_type_names() {
            assert_eq!(vocab.canonical_type(name), name, "type {name}");
        }
        for name in Vocabulary::canonical_builtin_names() {
            assert_eq!(vocab.canonical_builtin(name), name, "builtin {name}");
        }
        for name in Vocabulary::canonical_method_names() {
            assert_eq!(vocab.canonical_method(name), name, "method {name}");
        }
    }

    /// Every `BUILTINS`/`METHODS`/primitive-type name resolves in
    /// `Vocabulary::english()` -- adding a builtin/method/type without
    /// extending this module's registry-derived lists would fail here.
    #[test]
    fn registry_coverage() {
        let vocab = Vocabulary::english();
        for b in BUILTINS {
            assert_eq!(vocab.canonical_builtin(b.name), b.name);
        }
        for m in METHODS {
            assert_eq!(vocab.canonical_method(m.name), m.name);
        }
        for t in primitive_type_names() {
            assert_eq!(vocab.canonical_type(t), t);
        }
    }

    #[test]
    fn every_message_key_has_an_english_template() {
        // Trivially true by construction (MESSAGES *is* the English catalog),
        // but guards against a future refactor accidentally introducing a
        // key with an empty template.
        for (key, template) in messages::MESSAGES {
            assert!(!template.is_empty(), "empty English template for {key}");
        }
    }
}

/// Convenience re-export so callers can build `Rc<Vocabulary>` without a
/// separate `use std::rc::Rc` if all they import is `translations::*`.
pub type SharedVocabulary = Rc<Vocabulary>;
