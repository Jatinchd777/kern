//! Abstract syntax tree. Spans and helper accessors are kept for diagnostics
//! and for future passes that need source locations.
#![allow(dead_code)]

use crate::diagnostics::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
    Struct(StructDecl),
    Enum(EnumDecl),
    Use(UseDecl),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Function(item) => item.span,
            Item::Struct(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Use(item) => item.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariantDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariantDecl {
    pub name: String,
    pub payload: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExprKind {
    Unit,
    Named(String),
    Generic { name: String, args: Vec<TypeExpr> },
    Array { element: Box<TypeExpr>, len: usize },
    Tuple(Vec<TypeExpr>),
    Ref { mutable: bool, inner: Box<TypeExpr> },
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Let {
        name: String,
        mutable: bool,
        ty: Option<TypeExpr>,
        value: Option<Expr>,
    },
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },
    Expr(Expr),
    Print {
        args: Vec<PrintArg>,
        newline: bool,
    },
    Cin(Vec<Expr>),
    If {
        condition: Expr,
        then_block: Block,
        else_branch: Option<ElseBranch>,
    },
    While {
        condition: Expr,
        body: Block,
    },
    Loop {
        body: Block,
    },
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Box<Stmt>>,
        body: Block,
    },
    Break,
    Continue,
    Return(Option<Expr>),
    Block(Block),
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone)]
pub enum PrintArg {
    Expr(Expr),
    Template(Vec<TemplatePart>),
}

#[derive(Debug, Clone)]
pub enum TemplatePart {
    Text(String),
    Interpolation(Expr),
    Placeholder { format: FormatSpec, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FormatSpec {
    pub radix: Option<Radix>,
    pub precision: Option<usize>,
    pub width: Option<usize>,
    pub zero_pad: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radix {
    HexLower,
    HexUpper,
    Binary,
    Octal,
}

#[derive(Debug, Clone)]
pub enum ElseBranch {
    Block(Block),
    If(Box<Stmt>),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    EnumVariant { path: Vec<String>, span: Span },
    Wildcard { span: Span },
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::EnumVariant { span, .. } | Pattern::Wildcard { span } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int {
        value: u128,
        suffix: Option<String>,
    },
    Float {
        value: f64,
        suffix: Option<String>,
    },
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
    Var(String),
    Path(Vec<String>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    Field {
        base: Box<Expr>,
        name: String,
    },
    StructLiteral {
        name: String,
        fields: Vec<FieldInit>,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}
