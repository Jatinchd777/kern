use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals and identifiers.
    Int { value: u128, suffix: Option<String> },
    Float { value: f64, suffix: Option<String> },
    Str { value: String, raw: String },
    Char(char),
    Ident(String),

    // Keywords.
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Loop,
    For,
    Break,
    Continue,
    Return,
    Struct,
    Enum,
    Match,
    Use,
    True,
    False,
    Print,
    Println,
    Cin,

    // Punctuation and operators.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Amp,
    Pipe,
    Caret,
    Shl,
    Shr,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    Arrow,
    FatArrow,
    ColonColon,
    Dot,
    Comma,
    Semi,
    Colon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    Eof,
}

impl TokenKind {
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Int { .. } => "integer literal".to_string(),
            TokenKind::Float { .. } => "floating-point literal".to_string(),
            TokenKind::Str { .. } => "string literal".to_string(),
            TokenKind::Char(_) => "character literal".to_string(),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::Eof => "end of file".to_string(),
            other => format!("`{}`", other.symbol()),
        }
    }

    /// The source spelling for fixed tokens, used in parser diagnostics.
    pub fn symbol(&self) -> &'static str {
        match self {
            TokenKind::Fn => "fn",
            TokenKind::Let => "let",
            TokenKind::Mut => "mut",
            TokenKind::If => "if",
            TokenKind::Else => "else",
            TokenKind::While => "while",
            TokenKind::Loop => "loop",
            TokenKind::For => "for",
            TokenKind::Break => "break",
            TokenKind::Continue => "continue",
            TokenKind::Return => "return",
            TokenKind::Struct => "struct",
            TokenKind::Enum => "enum",
            TokenKind::Match => "match",
            TokenKind::Use => "use",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::Print => "print",
            TokenKind::Println => "println",
            TokenKind::Cin => "cin",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Eq => "=",
            TokenKind::EqEq => "==",
            TokenKind::Ne => "!=",
            TokenKind::Lt => "<",
            TokenKind::Le => "<=",
            TokenKind::Gt => ">",
            TokenKind::Ge => ">=",
            TokenKind::AndAnd => "&&",
            TokenKind::OrOr => "||",
            TokenKind::Bang => "!",
            TokenKind::Amp => "&",
            TokenKind::Pipe => "|",
            TokenKind::Caret => "^",
            TokenKind::Shl => "<<",
            TokenKind::Shr => ">>",
            TokenKind::PlusEq => "+=",
            TokenKind::MinusEq => "-=",
            TokenKind::StarEq => "*=",
            TokenKind::SlashEq => "/=",
            TokenKind::PercentEq => "%=",
            TokenKind::Arrow => "->",
            TokenKind::FatArrow => "=>",
            TokenKind::ColonColon => "::",
            TokenKind::Dot => ".",
            TokenKind::Comma => ",",
            TokenKind::Semi => ";",
            TokenKind::Colon => ":",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            _ => "token",
        }
    }

    pub fn same_variant(&self, other: &TokenKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
