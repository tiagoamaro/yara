//! Tokenizer for Yara source.

use crate::translations::Vocabulary;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // literals
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Ident(String),

    // keywords
    Def,
    End,
    If,
    Elsif,
    Else,
    While,
    For,
    In,
    Const,
    Return,
    Nil,
    Import,
    Class,

    // operators / punctuation
    Plus,
    Minus,
    Star,
    Slash,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Eq,
    Colon,
    ColonEq,
    DotDot,
    Dot,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,

    Eof,
}

/// Renders a `TokenKind` for error-message interpolation (e.g. "expected
/// `)`, found ..."). Byte-identical to what `#[derive(Debug)]` would print
/// (tuple variants as `Ident("foo")`/`Int(5)`, unit variants as their bare
/// name like `Plus`) — kept as a hand-written `Display` impl rather than
/// reusing `{:?}` directly so call sites can go through `vocab.msg`, which
/// only accepts `&str` arguments built via `Display`/`to_string`. Reproduces
/// the derive exactly by delegating each field to `{:?}` itself (so a
/// `Str`/`Ident` payload gets the same quote-and-escape treatment a derived
/// `Debug` would give it).
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Int(v) => write!(f, "Int({v:?})"),
            TokenKind::Float(v) => write!(f, "Float({v:?})"),
            TokenKind::Str(v) => write!(f, "Str({v:?})"),
            TokenKind::Bool(v) => write!(f, "Bool({v:?})"),
            TokenKind::Ident(v) => write!(f, "Ident({v:?})"),
            TokenKind::Def => write!(f, "Def"),
            TokenKind::End => write!(f, "End"),
            TokenKind::If => write!(f, "If"),
            TokenKind::Elsif => write!(f, "Elsif"),
            TokenKind::Else => write!(f, "Else"),
            TokenKind::While => write!(f, "While"),
            TokenKind::For => write!(f, "For"),
            TokenKind::In => write!(f, "In"),
            TokenKind::Const => write!(f, "Const"),
            TokenKind::Return => write!(f, "Return"),
            TokenKind::Nil => write!(f, "Nil"),
            TokenKind::Import => write!(f, "Import"),
            TokenKind::Class => write!(f, "Class"),
            TokenKind::Plus => write!(f, "Plus"),
            TokenKind::Minus => write!(f, "Minus"),
            TokenKind::Star => write!(f, "Star"),
            TokenKind::Slash => write!(f, "Slash"),
            TokenKind::EqEq => write!(f, "EqEq"),
            TokenKind::NotEq => write!(f, "NotEq"),
            TokenKind::Lt => write!(f, "Lt"),
            TokenKind::Gt => write!(f, "Gt"),
            TokenKind::LtEq => write!(f, "LtEq"),
            TokenKind::GtEq => write!(f, "GtEq"),
            TokenKind::Eq => write!(f, "Eq"),
            TokenKind::Colon => write!(f, "Colon"),
            TokenKind::ColonEq => write!(f, "ColonEq"),
            TokenKind::DotDot => write!(f, "DotDot"),
            TokenKind::Dot => write!(f, "Dot"),
            TokenKind::LParen => write!(f, "LParen"),
            TokenKind::RParen => write!(f, "RParen"),
            TokenKind::LBracket => write!(f, "LBracket"),
            TokenKind::RBracket => write!(f, "RBracket"),
            TokenKind::Comma => write!(f, "Comma"),
            TokenKind::Eof => write!(f, "Eof"),
        }
    }
}

/// The fixed set of reserved words/literal-keywords Yara recognizes,
/// independent of what text spells them in a given source file. This
/// indirection — text maps to `KeywordToken`, `KeywordToken` maps to
/// `TokenKind` — is what makes keyword translation possible: the lexer's
/// keyword table (`self.keywords: HashMap<String, KeywordToken>`) is data,
/// not a hardcoded string match, so a different set of source spellings
/// (see `translations::parse_keyword_file`) can point at the same
/// `KeywordToken`s and therefore produce exactly the same `TokenKind`s the
/// parser/typechecker/interpreter already know how to handle. `True`/`False`
/// are split out from `TokenKind::Bool(bool)` here since a keyword *token*
/// (as opposed to a lexed *token*) carries no payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeywordToken {
    Def,
    End,
    If,
    Elsif,
    Else,
    While,
    For,
    In,
    Const,
    Return,
    Nil,
    Import,
    Class,
    True,
    False,
}

/// The single source of truth for Yara's keyword vocabulary: each keyword's
/// canonical (English, load-bearing) spelling paired with its `KeywordToken`.
/// Everything keyword-related derives from this one table — `canonical_name`,
/// `all`, and `default_keywords` — so adding or renaming a keyword is a
/// one-line edit here, with no parallel list to keep in sync.
const KEYWORDS: &[(&str, KeywordToken)] = &[
    ("def", KeywordToken::Def),
    ("end", KeywordToken::End),
    ("if", KeywordToken::If),
    ("elsif", KeywordToken::Elsif),
    ("else", KeywordToken::Else),
    ("while", KeywordToken::While),
    ("for", KeywordToken::For),
    ("in", KeywordToken::In),
    ("const", KeywordToken::Const),
    ("return", KeywordToken::Return),
    ("nil", KeywordToken::Nil),
    ("import", KeywordToken::Import),
    ("class", KeywordToken::Class),
    ("true", KeywordToken::True),
    ("false", KeywordToken::False),
];

impl KeywordToken {
    /// The canonical (English, load-bearing) name used as the left-hand side
    /// in a translation file (`if = se`) and as the lookup key in
    /// `default_keywords()`. Derived from [`KEYWORDS`] — the reverse of the
    /// spelling→token direction that table is read in elsewhere. Panics only if
    /// a variant were ever missing from `KEYWORDS` (a programming error).
    pub fn canonical_name(self) -> &'static str {
        KEYWORDS
            .iter()
            .find(|(_, token)| *token == self)
            .map(|(name, _)| *name)
            .expect("every KeywordToken variant must be listed in KEYWORDS")
    }

    /// All keyword tokens, in `KEYWORDS` order — used by `translations` to look
    /// a token up by its canonical name. Derived from [`KEYWORDS`] so the
    /// variant list is never repeated.
    pub fn all() -> Vec<KeywordToken> {
        KEYWORDS.iter().map(|(_, token)| *token).collect()
    }
}

/// The default English keyword table: every keyword keyed by its canonical
/// spelling. This is what `Lexer::new` uses, and what `Lexer::with_keywords`
/// starts from before a translation file overrides some subset of entries (see
/// `translations::parse_keyword_file`) — a translation file only needs to list
/// the words it actually wants to change, so untranslated keywords silently
/// keep their English spelling. Built straight from [`KEYWORDS`].
pub fn default_keywords() -> HashMap<String, KeywordToken> {
    KEYWORDS
        .iter()
        .map(|(name, token)| (name.to_string(), *token))
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl crate::diagnostics::Diagnostic for LexError {
    fn kind(&self) -> &str {
        "lex error"
    }
    fn message(&self) -> &str {
        &self.message
    }
    fn span(&self) -> crate::diagnostics::Span {
        crate::diagnostics::Span::new(self.line, self.column)
    }
}

/// Walks a Yara source file one `char` at a time and turns it into a flat
/// list of tokens (see `tokenize`). This is the very first stage of the
/// pipeline: `Parser` (in `parser::`) only ever sees `Token`s, never raw
/// text, so every other stage is shielded from things like whitespace,
/// comments, or the exact spelling of a `0x`-vs-decimal number literal.
///
/// ```mermaid
/// flowchart LR
///     A[source: &str] --> B["Lexer::new"]
///     B --> C["Lexer::tokenize (consumes self)"]
///     C --> D["Vec&lt;Token&gt;"]
/// ```
pub struct Lexer {
    /// The whole source file, pre-split into `char`s. A `Vec<char>` (rather
    /// than indexing the original `&str` by byte) means `pos` always lines
    /// up with a whole character, so multi-byte UTF-8 can't be sliced in
    /// half — at the cost of an upfront O(n) allocation.
    chars: Vec<char>,
    /// Index into `chars` of the next character to read.
    pos: usize,
    /// 1-indexed line of `chars[pos]`, tracked incrementally in `advance`.
    line: usize,
    /// 1-indexed column of `chars[pos]`, tracked incrementally in `advance`.
    column: usize,
    /// Maps recognized keyword spellings (source text) to their
    /// `KeywordToken`. Defaults to `default_keywords()` (English); a
    /// translated program builds this from `translations::parse_keyword_file`
    /// instead via `Lexer::with_keywords`. Any identifier text not found here
    /// is just `TokenKind::Ident`, never an error — unknown-word handling is
    /// the parser's job, not the lexer's.
    keywords: HashMap<String, KeywordToken>,
    /// Governs error-message localization (`LexError.message` built via
    /// `vocab.msg`). Defaults to `Vocabulary::english()` for `new`/
    /// `with_keywords`; `with_vocabulary` is the entry point that also
    /// localizes lexer error prose, not just keyword spellings.
    vocab: Rc<Vocabulary>,
}

impl Lexer {
    /// Creates a lexer positioned at the very start of `source` (line 1,
    /// column 1), recognizing the default English keyword spellings.
    pub fn new(source: &str) -> Self {
        Self::with_keywords(source, default_keywords())
    }

    /// Like `new`, but recognizing `keywords` (source spelling -> reserved
    /// word) instead of the English defaults — the entry point for running
    /// a program with translated keywords (see `translations::parse_keyword_file`).
    /// Kept for source-compatibility; error messages stay English (no full
    /// `Vocabulary` available here) — use `with_vocabulary` to localize those too.
    pub fn with_keywords(source: &str, keywords: HashMap<String, KeywordToken>) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            keywords,
            vocab: Rc::new(Vocabulary::english()),
        }
    }

    /// Like `with_keywords`, but threading the full `Vocabulary` through so
    /// lexer error messages (`LexError.message`) are localized via
    /// `vocab.msg`, not just keyword spellings.
    pub fn with_vocabulary(source: &str, vocab: Rc<Vocabulary>) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            keywords: vocab.keywords.clone(),
            vocab,
        }
    }

    /// Runs the lexer to completion, producing every token in the source in
    /// order, terminated by a trailing `TokenKind::Eof`.
    ///
    /// The main loop is a dispatch on the next character's *category*:
    /// 1. Skip any run of whitespace/comments (`skip_whitespace_and_comments`).
    /// 2. If there's no character left, emit `Eof` at the current position and stop.
    /// 3. Otherwise, record the token's starting `(line, column)`, then hand off
    ///    to the sub-lexer for that character's category — digit -> `read_number`,
    ///    `"` -> `read_string`, letter/`_` -> `read_ident_or_keyword`, anything
    ///    else -> `read_operator` (which also reports "unrecognized character"
    ///    errors, since by elimination nothing else matched).
    ///
    /// Consumes `self` (rather than borrowing) because a `Lexer` is a
    /// one-shot pass — there's no reason to keep it around afterward.
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let (line, column) = (self.line, self.column);
            let Some(c) = self.peek() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    line,
                    column,
                });
                break;
            };

            let kind = if c.is_ascii_digit() {
                self.read_number()?
            } else if c == '"' {
                self.read_string()?
            } else if c.is_alphabetic() || c == '_' {
                self.read_ident_or_keyword()
            } else {
                self.read_operator()?
            };

            tokens.push(Token { kind, line, column });
        }
        Ok(tokens)
    }

    /// Returns the character at the current position without consuming it,
    /// or `None` at end of input. Every sub-lexer uses this (rather than
    /// indexing `chars` directly) to decide what to do next without
    /// accidentally moving forward.
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Like `peek`, but one character further ahead. Needed only where a
    /// single character isn't enough to decide what's being read — e.g. a
    /// `.` after digits is only the start of a float if the character after
    /// *that* is also a digit (`5.5` vs. `5..10`, a range).
    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    /// Consumes and returns the current character, moving `pos` forward by
    /// one and updating `line`/`column` to match: a newline resets the
    /// column to 1 and bumps the line, anything else just moves the column
    /// forward. This is the *only* place position bookkeeping happens, so
    /// every token's reported `line`/`column` is only ever as accurate as
    /// this one function.
    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    /// Consumes whitespace and `#`-to-end-of-line comments in a loop until
    /// neither is next, so the main `tokenize` loop always lands on the
    /// start of a real token. Whitespace and comments never become tokens
    /// themselves — they're fully invisible to every later compiler stage.
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                Some('#') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    /// Reads an integer or float literal starting at the current position.
    /// Consumes a run of digits, then checks for a `.` followed by *another*
    /// digit (so `5..10` — a range — isn't misread as `5.` followed by
    /// `.10`) to decide whether to also consume a fractional part. Parses
    /// the collected text with `str::parse`, turning any failure (e.g. an
    /// integer literal too large for `i64`) into a `LexError` at the
    /// literal's starting position.
    fn read_number(&mut self) -> Result<TokenKind, LexError> {
        let (line, column) = (self.line, self.column);
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }
        let mut is_float = false;
        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            self.advance();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if is_float {
            let value = text.parse::<f64>().map_err(|_| LexError {
                message: self.vocab.msg("lex/invalid-float-literal", &[&text]),
                line,
                column,
            })?;
            Ok(TokenKind::Float(value))
        } else {
            let value = text.parse::<i64>().map_err(|_| LexError {
                message: self.vocab.msg("lex/invalid-integer-literal", &[&text]),
                line,
                column,
            })?;
            Ok(TokenKind::Int(value))
        }
    }

    /// Reads a `"..."` string literal starting at the opening quote.
    /// Consumes characters until the matching closing `"`, translating
    /// `\n`/`\t`/`\"`/`\\` escape sequences along the way (any other escape,
    /// like `\q`, is a `LexError`). Hitting end-of-input or a newline before
    /// the closing quote is also an error ("unterminated string literal") —
    /// Yara has no multi-line string literal syntax.
    fn read_string(&mut self) -> Result<TokenKind, LexError> {
        let (line, column) = (self.line, self.column);
        self.advance(); // opening quote
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(LexError {
                        message: self.vocab.msg("lex/unterminated-string", &[]),
                        line,
                        column,
                    });
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.advance() {
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some(other) => {
                            return Err(LexError {
                                message: self
                                    .vocab
                                    .msg("lex/invalid-escape-sequence", &[&other.to_string()]),
                                line,
                                column,
                            });
                        }
                        None => {
                            return Err(LexError {
                                message: self.vocab.msg("lex/unterminated-string", &[]),
                                line,
                                column,
                            });
                        }
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.advance();
                }
            }
        }
        Ok(TokenKind::Str(value))
    }

    /// Reads a run of alphanumeric/`_` characters and classifies it: if the
    /// text matches one of Yara's reserved words (`def`, `if`, `class`, ...)
    /// or the boolean literals `true`/`false`, returns the matching keyword
    /// `TokenKind`; otherwise it's a plain identifier (`TokenKind::Ident`).
    /// Keyword lookup is a `match` on the collected `&str` rather than a
    /// hash-map — with this few keywords the compiler-generated jump table
    /// is both simpler to read and at least as fast.
    /// Reads a run of alphanumeric/`_` characters and classifies it by
    /// looking it up in `self.keywords`: a hit maps its `KeywordToken` to the
    /// matching `TokenKind` (splitting `True`/`False` back out into
    /// `TokenKind::Bool(true/false)`, since only `TokenKind` carries that
    /// payload); a miss is a plain identifier. Looking the text up in a map
    /// (built once, at `Lexer` construction) rather than matching on literal
    /// strings is what lets a translated keyword set (see
    /// `translations::parse_keyword_file`) recognize different source
    /// spellings while still producing the exact same `TokenKind`s every
    /// later stage already understands.
    fn read_ident_or_keyword(&mut self) -> TokenKind {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.advance();
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        match self.keywords.get(text.as_str()) {
            Some(KeywordToken::Def) => TokenKind::Def,
            Some(KeywordToken::End) => TokenKind::End,
            Some(KeywordToken::If) => TokenKind::If,
            Some(KeywordToken::Elsif) => TokenKind::Elsif,
            Some(KeywordToken::Else) => TokenKind::Else,
            Some(KeywordToken::While) => TokenKind::While,
            Some(KeywordToken::For) => TokenKind::For,
            Some(KeywordToken::In) => TokenKind::In,
            Some(KeywordToken::Const) => TokenKind::Const,
            Some(KeywordToken::Return) => TokenKind::Return,
            Some(KeywordToken::Import) => TokenKind::Import,
            Some(KeywordToken::Class) => TokenKind::Class,
            Some(KeywordToken::Nil) => TokenKind::Nil,
            Some(KeywordToken::True) => TokenKind::Bool(true),
            Some(KeywordToken::False) => TokenKind::Bool(false),
            None => TokenKind::Ident(text),
        }
    }

    /// Reads a single operator/punctuation token. Consumes one character and
    /// switches on it; several (`=`, `!`, `<`, `>`, `:`, `.`) peek one
    /// character further to decide between a one- and two-character token
    /// (`=` vs `==`, `:` vs `:=`, `.` vs `..`), following the general lexer
    /// pattern of "maximal munch" — always prefer the longest valid token.
    /// `!` with no following `=` and any character matching nothing above
    /// are both `LexError`s, since Yara has no unary `!`/`not` and no other
    /// single-character punctuation.
    fn read_operator(&mut self) -> Result<TokenKind, LexError> {
        let (line, column) = (self.line, self.column);
        let c = self.advance().unwrap();
        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::NotEq
                } else {
                    return Err(LexError {
                        message: "unexpected character `!`".to_string(),
                        line,
                        column,
                    });
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            ':' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::ColonEq
                } else {
                    TokenKind::Colon
                }
            }
            '.' => {
                if self.peek() == Some('.') {
                    self.advance();
                    TokenKind::DotDot
                } else {
                    TokenKind::Dot
                }
            }
            other => {
                return Err(LexError {
                    message: self
                        .vocab
                        .msg("lex/unexpected-character", &[&other.to_string()]),
                    line,
                    column,
                });
            }
        };
        Ok(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        Lexer::new(src)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    /// `TokenKind`'s hand-written `Display` must byte-match what
    /// `#[derive(Debug)]` would print (tuple variants quote/escape their
    /// payload, unit variants print their bare name) -- this is what lets
    /// `parser::mod.rs`/`expressions.rs` route `{:?}`-shaped error messages
    /// through `vocab.msg` without changing their English wording.
    #[test]
    fn token_kind_display_matches_derived_debug() {
        let cases = [
            TokenKind::Int(5),
            TokenKind::Float(1.5),
            TokenKind::Str("foo".to_string()),
            TokenKind::Bool(true),
            TokenKind::Ident("bar".to_string()),
            TokenKind::Plus,
            TokenKind::EqEq,
            TokenKind::Eof,
        ];
        for kind in cases {
            assert_eq!(kind.to_string(), format!("{kind:?}"));
        }
    }

    /// A localized vocabulary's `[messages]` override reaches lexer errors
    /// too, once `Lexer::with_vocabulary` (rather than `with_keywords`) is
    /// used -- proves `lex/unterminated-string` is actually read from
    /// `self.vocab` and not hardcoded.
    #[test]
    fn localized_vocabulary_translates_lexer_errors() {
        use crate::translations::parse_vocabulary;
        let vocab = std::rc::Rc::new(
            parse_vocabulary("[messages]\nlex/unterminated-string = string nao terminada\n")
                .unwrap(),
        );
        let err = Lexer::with_vocabulary("\"abc", vocab)
            .tokenize()
            .unwrap_err();
        assert_eq!(err.message, "string nao terminada");
    }

    /// The `KEYWORDS` table must be internally consistent: no spelling and no
    /// token appears twice, and `default_keywords()` reflects every entry.
    /// Guards the single-source-of-truth table against copy-paste slips now
    /// that `canonical_name`/`all`/`default_keywords` all derive from it.
    #[test]
    fn keyword_table_is_consistent() {
        let defaults = default_keywords();
        assert_eq!(
            defaults.len(),
            KEYWORDS.len(),
            "duplicate spelling in KEYWORDS"
        );
        let mut tokens = std::collections::HashSet::new();
        for (spelling, token) in KEYWORDS {
            assert!(
                tokens.insert(*token),
                "duplicate token in KEYWORDS: {token:?}"
            );
            assert_eq!(defaults.get(*spelling), Some(token));
        }
    }

    /// `canonical_name` must round-trip every token back to its `KEYWORDS`
    /// spelling — the reverse-lookup direction `translations` relies on.
    #[test]
    fn canonical_name_round_trips_every_keyword() {
        for token in KeywordToken::all() {
            let name = token.canonical_name();
            assert_eq!(default_keywords().get(name), Some(&token));
        }
    }

    #[test]
    fn tokenizes_function_def() {
        let src = "def add(a: Int, b: Int): Int\n  a + b\nend";
        let kinds = kinds(src);
        assert_eq!(
            kinds,
            vec![
                TokenKind::Def,
                TokenKind::Ident("add".into()),
                TokenKind::LParen,
                TokenKind::Ident("a".into()),
                TokenKind::Colon,
                TokenKind::Ident("Int".into()),
                TokenKind::Comma,
                TokenKind::Ident("b".into()),
                TokenKind::Colon,
                TokenKind::Ident("Int".into()),
                TokenKind::RParen,
                TokenKind::Colon,
                TokenKind::Ident("Int".into()),
                TokenKind::Ident("a".into()),
                TokenKind::Plus,
                TokenKind::Ident("b".into()),
                TokenKind::End,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tracks_line_and_column() {
        let tokens = Lexer::new("x\ny").tokenize().unwrap();
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[0].column, 1);
        assert_eq!(tokens[1].line, 2);
        assert_eq!(tokens[1].column, 1);
    }

    #[test]
    fn reads_literals() {
        assert_eq!(kinds("5")[0], TokenKind::Int(5));
        assert_eq!(kinds("5.5")[0], TokenKind::Float(5.5));
        assert_eq!(kinds("true")[0], TokenKind::Bool(true));
        assert_eq!(kinds("\"hi\"")[0], TokenKind::Str("hi".into()));
    }

    #[test]
    fn skips_comments() {
        let kinds = kinds("x # a comment\ny");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("x".into()),
                TokenKind::Ident("y".into()),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn range_operator() {
        assert_eq!(
            kinds("0..10"),
            vec![
                TokenKind::Int(0),
                TokenKind::DotDot,
                TokenKind::Int(10),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn unterminated_string_reports_position() {
        let err = Lexer::new("\"abc").tokenize().unwrap_err();
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 1);
    }

    #[test]
    fn brackets() {
        assert_eq!(
            kinds("[1, 2]"),
            vec![
                TokenKind::LBracket,
                TokenKind::Int(1),
                TokenKind::Comma,
                TokenKind::Int(2),
                TokenKind::RBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn class_keyword_and_dot() {
        assert_eq!(
            kinds("class Foo\nend\nx.field"),
            vec![
                TokenKind::Class,
                TokenKind::Ident("Foo".into()),
                TokenKind::End,
                TokenKind::Ident("x".into()),
                TokenKind::Dot,
                TokenKind::Ident("field".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn with_keywords_recognizes_translated_spellings() {
        let mut keywords = default_keywords();
        keywords.remove("if");
        keywords.insert("se".to_string(), KeywordToken::If);
        let tokens = Lexer::with_keywords("se x", keywords).tokenize().unwrap();
        assert_eq!(
            tokens.into_iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![TokenKind::If, TokenKind::Ident("x".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn with_keywords_untranslated_word_is_plain_ident() {
        // "if" was removed from the map in favor of "se" above; standalone
        // Lexer::new (default English map) still recognizes "if" as a keyword
        // and is unaffected by any other Lexer's custom map.
        assert_eq!(kinds("if")[0], TokenKind::If);
    }
}
