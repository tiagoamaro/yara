//! Recursive-descent parser: tokens -> AST.

use crate::ast::{BinOp, Expr, FieldDecl, Param, Stmt, TypeAnnotation, UnOp};
use crate::lexer::{normalize_type_alias, Token, TokenKind};
use std::fmt;

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

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

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

    fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ParseError> {
        let (name, line, column) = self.expect_ident()?;
        Ok(TypeAnnotation {
            name: normalize_type_alias(&name).to_string(),
            line,
            column,
        })
    }

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

    /// Disambiguates `x = expr` / `x: Type = expr` var decls from bare expression statements.
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

    /// `class Name ... end`, restricted body grammar: only `const` decls,
    /// bare `name: Type` field decls, and `def` methods are allowed.
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

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_comparison()
    }

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

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_primary_base()?;
        self.parse_postfix(expr)
    }

    /// Wraps `expr` in `Expr::Index` for each trailing `[expr]` (chained
    /// indexing, e.g. a future 2D-array `grid[i][j]`) and in `Expr::FieldAccess`
    /// / `Expr::MethodCall` for each trailing `.name` / `.name(args)`.
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
