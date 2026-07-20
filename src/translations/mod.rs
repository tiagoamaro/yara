//! Parses keyword-translation files: plain-text `canonical = localized`
//! mappings that let Yara's reserved words (`if`, `while`, `class`, ...) be
//! written in another language. See `examples/translations/CLAUDE.md` and
//! `translations/CLAUDE.md` for the end-to-end story; this module only owns
//! turning a translation file's text into the `HashMap<String, KeywordToken>`
//! that `lexer::Lexer::with_keywords` wants.

use crate::lexer::{default_keywords, KeywordToken};
use std::collections::HashMap;
use std::fmt;

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

/// Parses a translation file's contents into a keyword table ready for
/// `Lexer::with_keywords`.
///
/// Format: one `canonical = localized` mapping per line (`#` starts a
/// line comment, same as Yara itself; blank lines are ignored). `canonical`
/// must be one of the fixed English keyword names (`KeywordToken::canonical_name`,
/// e.g. `if`, `class`, `def`) — an unrecognized name is an error, since a
/// typo here should fail loudly rather than silently doing nothing.
///
/// The returned map starts from `default_keywords()` and only *overrides*
/// entries the file actually mentions: a translation file doesn't need to
/// re-list every keyword, only the ones it wants to change. Concretely, this
/// means:
/// 1. Start with the English map (`"if" -> If`, `"class" -> Class`, ...).
/// 2. For each `canonical = localized` line, remove the *old* spelling
///    (`canonical`'s current key, whatever it was) and insert `localized`
///    pointing at the same `KeywordToken` — so a translated file can't leave
///    both `if` and `se` simultaneously valid, which would be confusing.
/// 3. Reject a `localized` spelling that's already in use by a *different*
///    keyword (e.g. mapping two canonical names to the same translated
///    word) — that would make the two keywords indistinguishable in source
///    text, which is always a mistake, not a valid choice.
pub fn parse_keyword_file(text: &str) -> Result<HashMap<String, KeywordToken>, TranslationError> {
    let mut keywords = default_keywords();
    let mut canonical_to_current_spelling: HashMap<KeywordToken, String> = keywords
        .iter()
        .map(|(spelling, token)| (*token, spelling.clone()))
        .collect();

    for (i, raw_line) in text.lines().enumerate() {
        let line_number = i + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
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

        let Some(token) = KeywordToken::all()
            .into_iter()
            .find(|k| k.canonical_name() == canonical)
        else {
            return Err(TranslationError {
                message: format!("unknown keyword `{canonical}`"),
                line: line_number,
            });
        };

        if let Some(existing_owner) = keywords.get(localized) {
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

        let old_spelling = canonical_to_current_spelling
            .get(&token)
            .cloned()
            .unwrap_or_default();
        keywords.remove(&old_spelling);
        keywords.insert(localized.to_string(), token);
        canonical_to_current_spelling.insert(token, localized.to_string());
    }

    Ok(keywords)
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
            "/translations/pt.keywords"
        ))
        .unwrap();
        let keywords = parse_keyword_file(&text).unwrap();
        assert_eq!(keywords.get("se"), Some(&KeywordToken::If));
        assert_eq!(keywords.get("classe"), Some(&KeywordToken::Class));
        assert_eq!(keywords.get("verdadeiro"), Some(&KeywordToken::True));
    }
}
