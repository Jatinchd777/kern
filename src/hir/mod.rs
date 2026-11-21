use crate::diagnostics::Span;
use crate::parser::ast::{BinaryOp, UnaryOp};
use crate::semantic::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StructId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnumId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone)]
pub struct Program {
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub id: StructId,
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub id: EnumId,
    pub name: String,
    pub variants: Vec<Variant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub tag: u32,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub id: FunctionId,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Block,
    /// Indexed by `LocalId`; every local in the function has an entry.
    pub local_types: Vec<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub local: LocalId,
    pub name: String,
    pub ty: Type,
    pub span: Span,
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
        local: LocalId,
        ty: Type,
        mutable: bool,
        value: Option<Expr>,
    },
    Assign {
        local: LocalId,
        value: Expr,
    },
    Expr(Expr),
    Print(Expr),
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
pub enum ElseBranch {
    Block(Block),
    If(Box<Stmt>),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MatchPattern {
    Wildcard,
    Variant { enum_id: EnumId, variant: usize },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(u128),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
    Local(LocalId),
    EnumVariant {
        enum_id: EnumId,
        variant: usize,
    },
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
        function: FunctionId,
        args: Vec<Expr>,
    },
    Field {
        base: Box<Expr>,
        field: usize,
    },
    StructLiteral {
        struct_id: StructId,
        fields: Vec<(usize, Expr)>,
    },
}
