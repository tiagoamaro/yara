//! Tokenizer for Yara source.

use std::fmt;

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

/// Normalizes type-alias identifiers to their canonical long form.
/// `Int` -> `Integer`, `Bool` -> `Boolean`, `Str` -> `String`. All other
/// identifiers pass through unchanged.
pub fn normalize_type_alias(name: &str) -> &str {
    match name {
        "Int" => "Integer",
        "Bool" => "Boolean",
        "Str" => "String",
        other => other,
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
}

impl Lexer {
    /// Creates a lexer positioned at the very start of `source` (line 1,
    /// column 1).
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
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
                message: format!("invalid float literal `{text}`"),
                line,
                column,
            })?;
            Ok(TokenKind::Float(value))
        } else {
            let value = text.parse::<i64>().map_err(|_| LexError {
                message: format!("invalid integer literal `{text}`"),
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
                        message: "unterminated string literal".to_string(),
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
                                message: format!("invalid escape sequence `\\{other}`"),
                                line,
                                column,
                            });
                        }
                        None => {
                            return Err(LexError {
                                message: "unterminated string literal".to_string(),
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
    fn read_ident_or_keyword(&mut self) -> TokenKind {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.advance();
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        match text.as_str() {
            "def" => TokenKind::Def,
            "end" => TokenKind::End,
            "if" => TokenKind::If,
            "elsif" => TokenKind::Elsif,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "const" => TokenKind::Const,
            "return" => TokenKind::Return,
            "import" => TokenKind::Import,
            "class" => TokenKind::Class,
            "nil" => TokenKind::Nil,
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            _ => TokenKind::Ident(text),
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
                    message: format!("unexpected character `{other}`"),
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
    fn type_alias_normalization() {
        assert_eq!(normalize_type_alias("Int"), "Integer");
        assert_eq!(normalize_type_alias("Bool"), "Boolean");
        assert_eq!(normalize_type_alias("Str"), "String");
        assert_eq!(normalize_type_alias("Float"), "Float");
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
}
