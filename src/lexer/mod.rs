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
    LParen,
    RParen,
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

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

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

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

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
            "nil" => TokenKind::Nil,
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            _ => TokenKind::Ident(text),
        }
    }

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
                    return Err(LexError {
                        message: "unexpected character `.`".to_string(),
                        line,
                        column,
                    });
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
}
