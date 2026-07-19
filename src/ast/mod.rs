//! AST node definitions for Yara.

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    /// Canonical (alias-normalized) type name, e.g. `Integer`, `Float`, `Boolean`, `String`.
    pub name: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_ann: TypeAnnotation,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit {
        value: i64,
        line: usize,
        column: usize,
    },
    FloatLit {
        value: f64,
        line: usize,
        column: usize,
    },
    StringLit {
        value: String,
        line: usize,
        column: usize,
    },
    BoolLit {
        value: bool,
        line: usize,
        column: usize,
    },
    NilLit {
        line: usize,
        column: usize,
    },
    Ident {
        name: String,
        line: usize,
        column: usize,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        line: usize,
        column: usize,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
        line: usize,
        column: usize,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
        line: usize,
        column: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
}

impl Expr {
    pub fn line(&self) -> usize {
        match self {
            Expr::IntLit { line, .. }
            | Expr::FloatLit { line, .. }
            | Expr::StringLit { line, .. }
            | Expr::BoolLit { line, .. }
            | Expr::NilLit { line, .. }
            | Expr::Ident { line, .. }
            | Expr::Binary { line, .. }
            | Expr::Call { line, .. }
            | Expr::Unary { line, .. } => *line,
        }
    }

    pub fn column(&self) -> usize {
        match self {
            Expr::IntLit { column, .. }
            | Expr::FloatLit { column, .. }
            | Expr::StringLit { column, .. }
            | Expr::BoolLit { column, .. }
            | Expr::NilLit { column, .. }
            | Expr::Ident { column, .. }
            | Expr::Binary { column, .. }
            | Expr::Call { column, .. }
            | Expr::Unary { column, .. } => *column,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VarDecl {
        name: String,
        type_ann: Option<TypeAnnotation>,
        value: Expr,
        line: usize,
        column: usize,
    },
    ConstDecl {
        name: String,
        type_ann: Option<TypeAnnotation>,
        value: Expr,
        line: usize,
        column: usize,
    },
    FunctionDef {
        name: String,
        params: Vec<Param>,
        return_type: Option<TypeAnnotation>,
        body: Vec<Stmt>,
        line: usize,
        column: usize,
    },
    Return {
        value: Option<Expr>,
        line: usize,
        column: usize,
    },
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        elsif_branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
        line: usize,
        column: usize,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        line: usize,
        column: usize,
    },
    For {
        var_name: String,
        range_start: Expr,
        range_end: Expr,
        body: Vec<Stmt>,
        line: usize,
        column: usize,
    },
    ExprStmt(Expr),
}
