use super::*;

impl Parser {
    /// Dispatches to the right statement sub-parser purely by inspecting the
    /// current token's kind (one-token lookahead, no backtracking needed at
    /// this level): keywords like `def`/`const`/`class`/`import`/`return`/
    /// `if`/`while`/`for` each map to a dedicated `parse_*` method, a leading
    /// identifier goes to `parse_ident_stmt` (which itself may need to look
    /// further ahead to disambiguate), and anything else falls through to
    /// being parsed as a bare expression statement via `parse_expr`.
    pub(super) fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
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

        // Check for optional parent class: `class Child < Parent`
        let parent = if self.check(&TokenKind::Lt) {
            self.advance();
            let (parent_name, _, _) = self.expect_ident()?;
            Some(parent_name)
        } else {
            None
        };

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
            parent,
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

        let params = self.parse_comma_separated(&TokenKind::RParen, "`)`", Self::parse_param)?;

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

    /// Parses a single `name: Type` parameter declaration (the element parser
    /// for a function's parameter list; see `parse_comma_separated`).
    pub(super) fn parse_param(&mut self) -> Result<Param, ParseError> {
        let (name, line, column) = self.expect_ident()?;
        self.expect(&TokenKind::Colon, "`:`")?;
        let type_ann = self.parse_type_annotation()?;
        Ok(Param {
            name,
            type_ann,
            line,
            column,
        })
    }
}
