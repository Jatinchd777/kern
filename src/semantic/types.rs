use std::fmt;

use crate::hir::{EnumId, StructId};

/// The semantic type of a Kern expression or variable.
///
/// `Error` is an internal recovery type: it suppresses cascading diagnostics
/// and is never emitted by a successful compilation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Unit,
    Bool,
    Char,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
    String,
    Struct(StructId),
    Enum(EnumId),
    Never,
    Error,
}

impl Type {
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::I128
                | Type::Isize
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::U128
                | Type::Usize
        )
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::I128 | Type::Isize
        )
    }

    pub fn is_unsigned_integer(&self) -> bool {
        matches!(
            self,
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::Usize
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    pub fn is_scalar(&self) -> bool {
        self.is_numeric()
            || matches!(
                self,
                Type::Bool | Type::Char | Type::String | Type::Enum(_)
            )
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Type::Unit)
    }

    /// Width in bits for primitive integer and float types.
    pub fn bit_width(&self) -> Option<u32> {
        match self {
            Type::I8 | Type::U8 => Some(8),
            Type::I16 | Type::U16 => Some(16),
            Type::I32 | Type::U32 => Some(32),
            Type::I64 | Type::U64 | Type::Isize | Type::Usize => Some(64),
            Type::I128 | Type::U128 => Some(128),
            Type::F32 => Some(32),
            Type::F64 => Some(64),
            _ => None,
        }
    }

    pub fn signed(&self) -> bool {
        self.is_signed_integer()
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Type::Unit => "()",
            Type::Bool => "bool",
            Type::Char => "char",
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::I128 => "i128",
            Type::Isize => "isize",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::U64 => "u64",
            Type::U128 => "u128",
            Type::Usize => "usize",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::String => "String",
            Type::Struct(id) => return write!(f, "struct#{}", id.0),
            Type::Enum(id) => return write!(f, "enum#{}", id.0),
            Type::Never => "!",
            Type::Error => "<error>",
        };
        f.write_str(name)
    }
}
