//! Recursive-descent parser: tokens -> AST.

use crate::ast::{BinOp, Expr, FieldDecl, Param, Stmt, TypeAnnotation, UnOp};
use crate::lexer::{Token, TokenKind};
use crate::types::normalize_type_alias;
use std::fmt;

/// A parse-time failure. Mirrors `lexer::LexError`'s shape (a `message` plus
/// `line`/`column` of the offending token) so both stages of the pipeline
/// report errors the same way and callers can format them uniformly.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl crate::diagnostics::Diagnostic for ParseError {
    fn kind(&self) -> &str {
        "parse error"
    }
    fn message(&self) -> &str {
        &self.message
    }
    fn span(&self) -> crate::diagnostics::Span {
        crate::diagnostics::Span::new(self.line, self.column)
    }
}

/// Holds the full token stream produced by the lexer plus a cursor (`pos`)
/// into it. There is no separate "current token" field: every method reads
/// through `peek`/`advance`, so `pos` is the single source of truth for where
/// parsing is. Because it's just an integer, it can be saved and restored
/// (see `parse_ident_stmt`'s checkpoint/rewind) to support limited backtracking.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    /// Entry point: repeatedly parses top-level statements via `parse_stmt`
    /// until the `Eof` token is reached. There's no enclosing block here (no
    /// terminator other than `Eof`), since the whole file is one implicit block.
    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    /// Returns the token at the current cursor position without consuming it.
    /// Every dispatch decision in the parser (which statement/expression
    /// variant to build) is made by inspecting this token first.
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    /// Reports whether the current token has the same *kind* as `kind`,
    /// ignoring any payload the variant carries (e.g. the wrapped `String` in
    /// `Ident`/`Str`, or the number in `Int`/`Float`). It compares
    /// `std::mem::discriminant`s rather than the values themselves, so a
    /// placeholder like `TokenKind::Ident(String::new())` can be used purely
    /// to mean "any identifier" when building a terminator list, without the
    /// placeholder's payload ever being compared.
    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    /// Consumes and returns the current token, moving `pos` one step forward.
    /// Clamps at the last index instead of running past the end of the
    /// vector, relying on the lexer's invariant that it always appends a
    /// trailing `Eof` token — so once positioned on `Eof`, further calls just
    /// keep returning it rather than panicking on out-of-bounds access.
    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    /// Consumes the current token if it matches `kind`, otherwise produces a
    /// `ParseError` naming what was expected (the `what` string, e.g. "`)`"
    /// or "`=`") versus what was actually found. This is the parser's main
    /// building block for "this token must be here or the input is invalid"
    /// checks (closing parens/brackets, `end`, operators, etc.).
    fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            Err(ParseError {
                message: format!("expected {what}, found {:?}", tok.kind),
                line: tok.line,
                column: tok.column,
            })
        }
    }

    /// Like `expect`, specialized for `TokenKind::Ident`: consumes the
    /// current token only if it's an identifier, unwraps its `String` payload,
    /// and returns it along with the token's source position (needed by
    /// callers to stamp AST nodes with a location). Used everywhere a name is
    /// required — variable/function/class/param names, field/method names.
    fn expect_ident(&mut self) -> Result<(String, usize, usize), ParseError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident(name) => {
                self.advance();
                Ok((name, tok.line, tok.column))
            }
            _ => Err(ParseError {
                message: format!("expected identifier, found {:?}", tok.kind),
                line: tok.line,
                column: tok.column,
            }),
        }
    }

    /// Parses a type name (after a `:` in a param/field/var/const/return-type
    /// position) as a bare identifier, then normalizes short aliases (`Int`,
    /// `Str`, `Bool`, ...) to their canonical names (`Integer`, `String`,
    /// `Boolean`, ...) via `types::normalize_type_alias`. Alias resolution
    /// happens here in the parser rather than the lexer so the lexer only
    /// ever emits raw identifier tokens.
    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ParseError> {
        let (name, line, column) = self.expect_ident()?;
        Ok(TypeAnnotation {
            name: normalize_type_alias(&name).to_string(),
            line,
            column,
        })
    }

    /// Parses statements (via `parse_stmt`) into a `Vec` until the current
    /// token matches one of the given `terminators` (e.g. `[End]` for a
    /// `while`/`def` body, or `[Elsif, Else, End]` for an `if`-branch body).
    /// The terminator token itself is left unconsumed — the caller (e.g.
    /// `parse_if`, `parse_while`) is responsible for advancing past it, since
    /// which terminator matched affects what happens next (another `elsif`
    /// branch vs. the final `end`). Hitting `Eof` before any terminator is an
    /// error, since it means the block was never closed.
    fn parse_block(&mut self, terminators: &[TokenKind]) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !terminators.iter().any(|t| self.check(t)) {
            if self.check(&TokenKind::Eof) {
                let tok = self.peek();
                return Err(ParseError {
                    message: "unexpected end of input, expected `end`".to_string(),
                    line: tok.line,
                    column: tok.column,
                });
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    /// Dispatches to the right statement sub-parser purely by inspecting the
    /// current token's kind (one-token lookahead, no backtracking needed at
    /// this level): keywords like `def`/`const`/`class`/`import`/`return`/
    /// `if`/`while`/`for` each map to a dedicated `parse_*` method, a leading
    /// identifier goes to `parse_ident_stmt` (which itself may need to look
    /// further ahead to disambiguate), and anything else falls through to
    /// being parsed as a bare expression statement via `parse_expr`.
    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Def => self.parse_function_def(),
            TokenKind::Const => self.parse_const_decl(),
            TokenKind::Class => self.parse_class(),
            TokenKind::Import => self.parse_import(),
            TokenKind::Return => self.parse_return(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Ident(_) => self.parse_ident_stmt(),
            _ => {
                let expr = self.parse_expr()?;
                Ok(Stmt::ExprStmt(expr))
            }
        }
    }

    /// Disambiguates the several statement shapes that can start with an
    /// identifier: `x: Type = expr` (typed var decl), `x = expr` (inferred
    /// var decl), `obj.field = value` (field assignment), or a bare
    /// expression statement (e.g. a call `foo()` or `obj.method()`).
    ///
    /// Uses a checkpoint/rewind trick because the grammar can't be
    /// disambiguated by the identifier alone: first it consumes the
    /// identifier and checks for an immediately-following `:`, which
    /// unambiguously signals a typed decl. If there's no `:`, the parser
    /// can't yet tell whether this is `x = value` or just an expression like
    /// `x.foo()` or `x + 1` — so it rewinds `self.pos` back to the saved
    /// `checkpoint` and re-parses from scratch as a full expression via
    /// `parse_expr` (which itself handles idents, calls, field access,
    /// method calls, ...). Once that expression is in hand, it switches on
    /// whether an `=` follows and, if so, on the *shape* of the parsed
    /// expression: a plain `Expr::Ident` becomes an inferred `Stmt::VarDecl`,
    /// an `Expr::FieldAccess` becomes a `Stmt::FieldAssign`, and anything
    /// else followed by `=` (e.g. `f() = 2`) is rejected as an invalid
    /// assignment target. No `=` at all means it's just an expression
    /// statement.
    fn parse_ident_stmt(&mut self) -> Result<Stmt, ParseError> {
        let checkpoint = self.pos;
        let (name, line, column) = self.expect_ident()?;

        if self.check(&TokenKind::Colon) {
            self.advance();
            let type_ann = self.parse_type_annotation()?;
            self.expect(&TokenKind::Eq, "`=`")?;
            let value = self.parse_expr()?;
            return Ok(Stmt::VarDecl {
                name,
                type_ann: Some(type_ann),
                value,
                line,
                column,
            });
        }

        // Not a typed decl: rewind and parse a full expression, which also
        // handles `obj.field`, `obj.method(args)`, plain idents, and calls.
        self.pos = checkpoint;
        let expr = self.parse_expr()?;

        if self.check(&TokenKind::Eq) {
            self.advance();
            let value = self.parse_expr()?;
            return match expr {
                Expr::Ident { name, line, column } => Ok(Stmt::VarDecl {
                    name,
                    type_ann: None,
                    value,
                    line,
                    column,
                }),
                Expr::FieldAccess {
                    object,
                    field,
                    line,
                    column,
                } => Ok(Stmt::FieldAssign {
                    object: *object,
                    field,
                    value,
                    line,
                    column,
                }),
                other => Err(ParseError {
                    message: "invalid assignment target".to_string(),
                    line: other.line(),
                    column: other.column(),
                }),
            };
        }

        Ok(Stmt::ExprStmt(expr))
    }

    /// Parses `import "path"`: the keyword, then a required string-literal
    /// token holding the module path. Unlike most other statements, an
    /// import has no `end` to close — it's a single-token-payload statement.
    fn parse_import(&mut self) -> Result<Stmt, ParseError> {
        let import_tok = self.advance();
        let tok = self.peek().clone();
        let path = match tok.kind {
            TokenKind::Str(value) => {
                self.advance();
                value
            }
            _ => {
                return Err(ParseError {
                    message: format!(
                        "expected string literal after `import`, found {:?}",
                        tok.kind
                    ),
                    line: tok.line,
                    column: tok.column,
                });
            }
        };
        Ok(Stmt::Import {
            path,
            line: import_tok.line,
            column: import_tok.column,
        })
    }

    /// Parses `const NAME[: Type] = expr`. Structurally identical to the
    /// inferred/typed `VarDecl` path in `parse_ident_stmt`, except the `const`
    /// keyword makes the shape unambiguous up front, so there's no need for
    /// the checkpoint/rewind trick used there: the optional type annotation
    /// and the value expression are just read straight through.
    fn parse_const_decl(&mut self) -> Result<Stmt, ParseError> {
        let const_tok = self.advance();
        let (name, _, _) = self.expect_ident()?;
        let type_ann = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_annotation()?)
        } else {
            None
        };
        self.expect(&TokenKind::Eq, "`=`")?;
        let value = self.parse_expr()?;
        Ok(Stmt::ConstDecl {
            name,
            type_ann,
            value,
            line: const_tok.line,
            column: const_tok.column,
        })
    }

    /// Parses `class Name ... end` using its own statement loop rather than
    /// delegating to `parse_stmt`/`parse_block`, because a class body's
    /// grammar is deliberately more restricted than a normal block's: only
    /// three shapes are legal inside it, dispatched on the current token just
    /// like `parse_stmt` does, but with everything else rejected outright
    /// instead of falling through to an expression statement:
    /// - `const NAME: Type = expr` -> delegates to `parse_const_decl`.
    /// - `def name(...) ... end` -> delegates to `parse_function_def`
    ///   (becomes a method).
    /// - a bare `name: Type` (no `=`, no keyword) -> parsed inline here as a
    ///   `FieldDecl`, since field declarations don't correspond to any
    ///   `Stmt` variant used outside a class.
    /// Anything else, or hitting `Eof` before the closing `end`, is a parse
    /// error naming what was expected.
    fn parse_class(&mut self) -> Result<Stmt, ParseError> {
        let class_tok = self.advance();
        let (name, _, _) = self.expect_ident()?;

        let mut consts = Vec::new();
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::End) {
            let tok = self.peek().clone();
            match tok.kind {
                TokenKind::Eof => {
                    return Err(ParseError {
                        message: "unexpected end of input, expected `end`".to_string(),
                        line: tok.line,
                        column: tok.column,
                    });
                }
                TokenKind::Const => consts.push(self.parse_const_decl()?),
                TokenKind::Def => methods.push(self.parse_function_def()?),
                TokenKind::Ident(_) => {
                    let (fname, fline, fcolumn) = self.expect_ident()?;
                    self.expect(&TokenKind::Colon, "`:`")?;
                    let type_ann = self.parse_type_annotation()?;
                    fields.push(FieldDecl {
                        name: fname,
                        type_ann,
                        line: fline,
                        column: fcolumn,
                    });
                }
                _ => {
                    return Err(ParseError {
                        message: format!(
                            "expected a const, field, or method declaration inside `class`, found {:?}",
                            tok.kind
                        ),
                        line: tok.line,
                        column: tok.column,
                    });
                }
            }
        }
        self.expect(&TokenKind::End, "`end`")?;

        Ok(Stmt::ClassDef {
            name,
            consts,
            fields,
            methods,
            line: class_tok.line,
            column: class_tok.column,
        })
    }

    /// Parses `def name(param: Type, ...): ReturnType ... end` (both a
    /// top-level function and, when found inside `parse_class`, a method —
    /// the AST doesn't distinguish the two, since that's a semantic
    /// distinction made later by the typechecker/interpreter based on where
    /// the `FunctionDef` sits). Reads a comma-separated parameter list (each
    /// requiring an explicit `name: Type`), an optional `: ReturnType`, then
    /// delegates the function body to `parse_block(&[End])` so it can contain
    /// arbitrary statements (unlike the restricted `parse_class` body).
    fn parse_function_def(&mut self) -> Result<Stmt, ParseError> {
        let def_tok = self.advance();
        let (name, _, _) = self.expect_ident()?;
        self.expect(&TokenKind::LParen, "`(`")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let (pname, pline, pcolumn) = self.expect_ident()?;
                self.expect(&TokenKind::Colon, "`:`")?;
                let type_ann = self.parse_type_annotation()?;
                params.push(Param {
                    name: pname,
                    type_ann,
                    line: pline,
                    column: pcolumn,
                });
                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "`)`")?;

        let return_type = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_block(&[TokenKind::End])?;
        self.expect(&TokenKind::End, "`end`")?;

        Ok(Stmt::FunctionDef {
            name,
            params,
            return_type,
            body,
            line: def_tok.line,
            column: def_tok.column,
        })
    }

    /// Parses `return` with an optional trailing expression. The value is
    /// omitted (`None`) when the very next token is `end` or `Eof`, i.e.
    /// `return` appears on its own with nothing after it on the same
    /// statement; otherwise whatever follows is parsed as the return value
    /// expression.
    fn parse_return(&mut self) -> Result<Stmt, ParseError> {
        let tok = self.advance();
        let value = if self.check(&TokenKind::End) || self.check(&TokenKind::Eof) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        Ok(Stmt::Return {
            value,
            line: tok.line,
            column: tok.column,
        })
    }

    /// Parses `if cond ... [elsif cond ...]* [else ...] end`. Each branch
    /// body is parsed with `parse_block` given the set of tokens that could
    /// legally follow it: the `then` body and each `elsif` body stop at the
    /// next `elsif`, an `else`, or the final `end` (since any of those could
    /// come next), while the `else` body (if present) only stops at `end`.
    /// The trailing `end` is consumed once at the very end, after the
    /// optional `else` has already been handled, closing the whole
    /// if-elsif-else chain.
    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        let if_tok = self.advance();
        let condition = self.parse_expr()?;
        let then_body = self.parse_block(&[TokenKind::Elsif, TokenKind::Else, TokenKind::End])?;

        let mut elsif_branches = Vec::new();
        while self.check(&TokenKind::Elsif) {
            self.advance();
            let cond = self.parse_expr()?;
            let body = self.parse_block(&[TokenKind::Elsif, TokenKind::Else, TokenKind::End])?;
            elsif_branches.push((cond, body));
        }

        let else_body = if self.check(&TokenKind::Else) {
            self.advance();
            Some(self.parse_block(&[TokenKind::End])?)
        } else {
            None
        };

        self.expect(&TokenKind::End, "`end`")?;

        Ok(Stmt::If {
            condition,
            then_body,
            elsif_branches,
            else_body,
            line: if_tok.line,
            column: if_tok.column,
        })
    }

    /// Parses `while cond ... end`: a condition expression followed by a
    /// body block terminated by `end`.
    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        let while_tok = self.advance();
        let condition = self.parse_expr()?;
        let body = self.parse_block(&[TokenKind::End])?;
        self.expect(&TokenKind::End, "`end`")?;
        Ok(Stmt::While {
            condition,
            body,
            line: while_tok.line,
            column: while_tok.column,
        })
    }

    /// Parses `for var in start..end ... end`. Notably, the range bounds are
    /// parsed with `parse_additive` rather than the full `parse_expr`
    /// (comparison-level) — since `..` sits between additive and comparison
    /// precedence in this grammar, using the full expression parser would let
    /// a stray comparison operator swallow one side of the range in a
    /// confusing way, so bounds are restricted to additive-or-tighter
    /// expressions (literals, arithmetic, calls, etc.).
    fn parse_for(&mut self) -> Result<Stmt, ParseError> {
        let for_tok = self.advance();
        let (var_name, _, _) = self.expect_ident()?;
        self.expect(&TokenKind::In, "`in`")?;
        let range_start = self.parse_additive()?;
        self.expect(&TokenKind::DotDot, "`..`")?;
        let range_end = self.parse_additive()?;
        let body = self.parse_block(&[TokenKind::End])?;
        self.expect(&TokenKind::End, "`end`")?;
        Ok(Stmt::For {
            var_name,
            range_start,
            range_end,
            body,
            line: for_tok.line,
            column: for_tok.column,
        })
    }

    /// Entry point into expression parsing — currently just forwards to
    /// `parse_comparison`, the loosest-binding level. This is precedence
    /// climbing / operator-precedence parsing via a chain of methods, one
    /// per precedence level, from loosest to tightest binding:
    /// `parse_expr` -> `parse_comparison` -> `parse_additive` ->
    /// `parse_multiplicative` -> `parse_unary` -> `parse_primary` (which is
    /// `parse_primary_base` + `parse_postfix`). Each level's method always
    /// calls the *next tighter* level first to get its left operand (and,
    /// after seeing an operator, its right operand too), then loops to
    /// consume zero or more operators *at its own precedence* — never a
    /// looser one, since those belong to a level further up the chain, and
    /// never a tighter one, since the recursive call into the next level
    /// already consumed those. This is what encodes precedence without
    /// needing an explicit precedence table: `1 + 2 * 3` is parsed by
    /// `parse_additive` calling `parse_multiplicative` for its left operand,
    /// which itself consumes `2 * 3` entirely before `parse_additive` ever
    /// sees the `+`, naturally producing `1 + (2 * 3)`. Each loop builds a
    /// left-associative tree by re-binding `left` to a new `Expr::Binary`
    /// node on every iteration, so `1 - 2 - 3` parses as `(1 - 2) - 3` rather
    /// than the (wrong, right-associative) alternative.
    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_comparison()
    }

    /// Loosest-binding level: equality/relational operators (`==`, `!=`,
    /// `<`, `>`, `<=`, `>=`). Gets its operands from `parse_additive` (the
    /// next tighter level) and loops, left-associatively, over any chain of
    /// comparison operators — so `a < b == c` parses as `(a < b) == c`.
    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::NotEq => BinOp::NotEq,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::LtEq => BinOp::LtEq,
                TokenKind::GtEq => BinOp::GtEq,
                _ => break,
            };
            let tok = self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                column: tok.column,
            };
        }
        Ok(left)
    }

    /// Middle precedence level: binary `+`/`-`. Gets operands from
    /// `parse_multiplicative` (tighter-binding) and loops over any chain of
    /// same-level operators, so `1 + 2 - 3` parses left-associatively as
    /// `(1 + 2) - 3`.
    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            let tok = self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                column: tok.column,
            };
        }
        Ok(left)
    }

    /// Binds tighter than `+`/`-`, looser than unary `-`: `*`/`/`. Gets
    /// operands from `parse_unary` and loops over a chain of same-level
    /// operators left-associatively, so `1 * 2 / 3` parses as `(1 * 2) / 3`.
    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => break,
            };
            let tok = self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                column: tok.column,
            };
        }
        Ok(left)
    }

    /// Handles prefix `-` (negation). Unlike the binary levels above, this
    /// isn't a left-associative loop — it's a right-recursive call to itself
    /// (`self.parse_unary()`, not `parse_primary()`), so a run of leading
    /// minuses like `--x` parses as nested `Unary(Neg, Unary(Neg, x))`
    /// rather than being rejected or flattened (the typechecker/interpreter
    /// are left to just evaluate the double negation). Once there's no more
    /// leading `-`, it falls through to `parse_primary`, the tightest-binding
    /// level (literals, identifiers, calls, parens, array literals, and
    /// their postfix operators).
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.check(&TokenKind::Minus) {
            let tok = self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnOp::Neg,
                expr: Box::new(expr),
                line: tok.line,
                column: tok.column,
            });
        }
        self.parse_primary()
    }

    /// Tightest-binding level of the precedence chain: builds the base
    /// expression via `parse_primary_base` (literals, identifiers, calls,
    /// parenthesized sub-expressions, array literals), then immediately
    /// hands it to `parse_postfix` to consume any trailing indexing/field-
    /// access/method-call operators. Postfix operators bind tighter than
    /// anything else in the grammar — even tighter than unary `-` — which is
    /// why `parse_unary` recurses into `parse_primary` rather than the other
    /// way around: `-a[0]` must parse as `-(a[0])`, not `(-a)[0]`.
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_primary_base()?;
        self.parse_postfix(expr)
    }

    /// Given an already-parsed base expression, greedily consumes a chain of
    /// postfix operators and folds them onto it, left to right, so they
    /// associate the way source order suggests (`a.b.c[0].d(1)` reads as
    /// "get `.b` off `a`, then `.c` off that, then index `[0]`, then call
    /// `.d(1)` on the result"): each loop iteration wraps `expr` in one more
    /// layer —
    /// - a trailing `[expr]` wraps it in `Expr::Index`, supporting chained
    ///   indexing like a future 2D-array `grid[i][j]`;
    /// - a trailing `.name` wraps it in `Expr::FieldAccess`, unless that name
    ///   is immediately followed by `(`, in which case the parenthesized,
    ///   comma-separated argument list is consumed too and it becomes an
    ///   `Expr::MethodCall` instead.
    /// The loop stops (and returns `expr` as-is) as soon as neither `[` nor
    /// `.` follows, which is what lets a plain identifier or literal pass
    /// through unchanged when it has no postfix operators at all.
    fn parse_postfix(&mut self, mut expr: Expr) -> Result<Expr, ParseError> {
        loop {
            if self.check(&TokenKind::LBracket) {
                let bracket_tok = self.advance();
                let index = self.parse_expr()?;
                self.expect(&TokenKind::RBracket, "`]`")?;
                expr = Expr::Index {
                    array: Box::new(expr),
                    index: Box::new(index),
                    line: bracket_tok.line,
                    column: bracket_tok.column,
                };
            } else if self.check(&TokenKind::Dot) {
                let dot_tok = self.advance();
                let (name, _, _) = self.expect_ident()?;
                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.check(&TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen, "`)`")?;
                    expr = Expr::MethodCall {
                        object: Box::new(expr),
                        method: name,
                        args,
                        line: dot_tok.line,
                        column: dot_tok.column,
                    };
                } else {
                    expr = Expr::FieldAccess {
                        object: Box::new(expr),
                        field: name,
                        line: dot_tok.line,
                        column: dot_tok.column,
                    };
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }

    /// Parses a single base expression with no postfix operators attached
    /// yet (that's `parse_postfix`'s job, applied by the caller
    /// `parse_primary`): dispatches purely on the current token's kind to
    /// build a literal (`Int`/`Float`/`Str`/`Bool`/`Nil`), a bare identifier
    /// or, if `(` follows the identifier, a function `Call` with a
    /// comma-separated argument list, a parenthesized sub-expression
    /// (`( expr )`, which re-enters `parse_expr` at full precedence and just
    /// returns the inner expression unwrapped — parens have no AST node of
    /// their own, they just override precedence), or an array literal
    /// (`[ expr, expr, ... ]`). Any other token is a parse error, since
    /// nothing else can start an expression.
    fn parse_primary_base(&mut self) -> Result<Expr, ParseError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Int(value) => {
                self.advance();
                Ok(Expr::IntLit {
                    value,
                    line: tok.line,
                    column: tok.column,
                })
            }
            TokenKind::Float(value) => {
                self.advance();
                Ok(Expr::FloatLit {
                    value,
                    line: tok.line,
                    column: tok.column,
                })
            }
            TokenKind::Str(ref value) => {
                let value = value.clone();
                self.advance();
                Ok(Expr::StringLit {
                    value,
                    line: tok.line,
                    column: tok.column,
                })
            }
            TokenKind::Bool(value) => {
                self.advance();
                Ok(Expr::BoolLit {
                    value,
                    line: tok.line,
                    column: tok.column,
                })
            }
            TokenKind::Nil => {
                self.advance();
                Ok(Expr::NilLit {
                    line: tok.line,
                    column: tok.column,
                })
            }
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.advance();
                if self.check(&TokenKind::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.check(&TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen, "`)`")?;
                    Ok(Expr::Call {
                        callee: name,
                        args,
                        line: tok.line,
                        column: tok.column,
                    })
                } else {
                    Ok(Expr::Ident {
                        name,
                        line: tok.line,
                        column: tok.column,
                    })
                }
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "`)`")?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    loop {
                        elements.push(self.parse_expr()?);
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBracket, "`]`")?;
                Ok(Expr::ArrayLit {
                    elements,
                    line: tok.line,
                    column: tok.column,
                })
            }
            _ => Err(ParseError {
                message: format!("unexpected token {:?}", tok.kind),
                line: tok.line,
                column: tok.column,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> Vec<Stmt> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap()
    }

    #[test]
    fn parses_function_def() {
        let stmts = parse("def add(a: Int, b: Int): Int\n  a + b\nend");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::FunctionDef {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].type_ann.name, "Integer");
                assert_eq!(return_type.as_ref().unwrap().name, "Integer");
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected FunctionDef, got {other:?}"),
        }
    }

    #[test]
    fn parses_var_decl_inferred_and_explicit() {
        let stmts = parse("x = 5\ny: Float = 5.0");
        match &stmts[0] {
            Stmt::VarDecl {
                name,
                type_ann: None,
                ..
            } => assert_eq!(name, "x"),
            other => panic!("expected VarDecl, got {other:?}"),
        }
        match &stmts[1] {
            Stmt::VarDecl {
                name,
                type_ann: Some(t),
                ..
            } => {
                assert_eq!(name, "y");
                assert_eq!(t.name, "Float");
            }
            other => panic!("expected VarDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_type_alias_in_var_decl() {
        let stmts = parse("x: Int = 5\ny: Bool = true\nz: Str = \"hi\"");
        for (stmt, expected) in stmts.iter().zip(["Integer", "Boolean", "String"]) {
            match stmt {
                Stmt::VarDecl {
                    type_ann: Some(t), ..
                } => assert_eq!(t.name, expected),
                other => panic!("expected VarDecl, got {other:?}"),
            }
        }
    }

    #[test]
    fn parses_if_elsif_else() {
        let stmts = parse("if x > 0\n  1\nelsif x < 0\n  2\nelse\n  3\nend");
        match &stmts[0] {
            Stmt::If {
                elsif_branches,
                else_body,
                ..
            } => {
                assert_eq!(elsif_branches.len(), 1);
                assert!(else_body.is_some());
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn parses_while() {
        let stmts = parse("while x > 0\n  x = x - 1\nend");
        assert!(matches!(stmts[0], Stmt::While { .. }));
    }

    #[test]
    fn parses_for_range() {
        let stmts = parse("for i in 0..10\n  print(i)\nend");
        match &stmts[0] {
            Stmt::For { var_name, .. } => assert_eq!(var_name, "i"),
            other => panic!("expected For, got {other:?}"),
        }
    }

    #[test]
    fn parses_call_expr_stmt() {
        let stmts = parse("print(\"hi\")");
        match &stmts[0] {
            Stmt::ExprStmt(Expr::Call { callee, args, .. }) => {
                assert_eq!(callee, "print");
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected call ExprStmt, got {other:?}"),
        }
    }

    #[test]
    fn operator_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let stmts = parse("1 + 2 * 3");
        match &stmts[0] {
            Stmt::ExprStmt(Expr::Binary {
                op, left, right, ..
            }) => {
                assert_eq!(*op, BinOp::Add);
                assert!(matches!(**left, Expr::IntLit { value: 1, .. }));
                assert!(matches!(**right, Expr::Binary { op: BinOp::Mul, .. }));
            }
            other => panic!("expected Binary, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_reports_position() {
        let tokens = Lexer::new("def foo(\n  1\nend").tokenize().unwrap();
        let err = Parser::new(tokens).parse_program().unwrap_err();
        assert_eq!(err.line, 2);
    }

    #[test]
    fn const_decl() {
        let stmts = parse("const PI: Float = 3.14");
        match &stmts[0] {
            Stmt::ConstDecl { name, type_ann, .. } => {
                assert_eq!(name, "PI");
                assert_eq!(type_ann.as_ref().unwrap().name, "Float");
            }
            other => panic!("expected ConstDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_unary_negation() {
        let stmts = parse("x = -5");
        match &stmts[0] {
            Stmt::VarDecl { value, .. } => {
                assert!(matches!(
                    value,
                    Expr::Unary {
                        op: crate::ast::UnOp::Neg,
                        ..
                    }
                ));
            }
            other => panic!("expected VarDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_array_literal_and_index() {
        let stmts = parse("xs = [1, 2, 3]\ny = xs[0]");
        match &stmts[0] {
            Stmt::VarDecl {
                value: Expr::ArrayLit { elements, .. },
                ..
            } => assert_eq!(elements.len(), 3),
            other => panic!("expected ArrayLit VarDecl, got {other:?}"),
        }
        match &stmts[1] {
            Stmt::VarDecl {
                value: Expr::Index { .. },
                ..
            } => {}
            other => panic!("expected Index VarDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_import() {
        let stmts = parse("import \"helper\"");
        match &stmts[0] {
            Stmt::Import { path, .. } => assert_eq!(path, "helper"),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn parses_class_with_const_field_and_method() {
        let stmts = parse(
            "class Hello\n  const PI: Float = 3.14159\n  count: Integer\n\n  def initializer(number: Int)\n    count = number\n  end\nend",
        );
        match &stmts[0] {
            Stmt::ClassDef {
                name,
                consts,
                fields,
                methods,
                ..
            } => {
                assert_eq!(name, "Hello");
                assert_eq!(consts.len(), 1);
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "count");
                assert_eq!(fields[0].type_ann.name, "Integer");
                assert_eq!(methods.len(), 1);
                match &methods[0] {
                    Stmt::FunctionDef {
                        name: mname,
                        params,
                        ..
                    } => {
                        assert_eq!(mname, "initializer");
                        assert_eq!(params.len(), 1);
                    }
                    other => panic!("expected FunctionDef, got {other:?}"),
                }
            }
            other => panic!("expected ClassDef, got {other:?}"),
        }
    }

    #[test]
    fn parses_new_field_access_and_method_call() {
        let stmts = parse("h = Hello.new(5)\nx = h.count\nh.greet(\"hi\")\nh.count = 9");
        match &stmts[0] {
            Stmt::VarDecl {
                value:
                    Expr::MethodCall {
                        method,
                        args,
                        object,
                        ..
                    },
                ..
            } => {
                assert_eq!(method, "new");
                assert_eq!(args.len(), 1);
                assert!(matches!(**object, Expr::Ident { .. }));
            }
            other => panic!("expected MethodCall VarDecl, got {other:?}"),
        }
        match &stmts[1] {
            Stmt::VarDecl {
                value: Expr::FieldAccess { field, .. },
                ..
            } => assert_eq!(field, "count"),
            other => panic!("expected FieldAccess VarDecl, got {other:?}"),
        }
        match &stmts[2] {
            Stmt::ExprStmt(Expr::MethodCall { method, args, .. }) => {
                assert_eq!(method, "greet");
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected MethodCall ExprStmt, got {other:?}"),
        }
        match &stmts[3] {
            Stmt::FieldAssign { field, .. } => assert_eq!(field, "count"),
            other => panic!("expected FieldAssign, got {other:?}"),
        }
    }
}
