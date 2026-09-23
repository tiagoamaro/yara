use super::*;

impl Parser {
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
    pub(super) fn parse_expr(&mut self) -> Result<Expr, ParseError> {
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
    pub(super) fn parse_additive(&mut self) -> Result<Expr, ParseError> {
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
                    let args =
                        self.parse_comma_separated(&TokenKind::RParen, "`)`", Self::parse_expr)?;
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
                    let args =
                        self.parse_comma_separated(&TokenKind::RParen, "`)`", Self::parse_expr)?;
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
                let elements =
                    self.parse_comma_separated(&TokenKind::RBracket, "`]`", Self::parse_expr)?;
                Ok(Expr::ArrayLit {
                    elements,
                    line: tok.line,
                    column: tok.column,
                })
            }
            _ => Err(ParseError {
                message: self
                    .vocab
                    .msg("parse/unexpected-token", &[&tok.kind.to_string()]),
                line: tok.line,
                column: tok.column,
            }),
        }
    }
}
