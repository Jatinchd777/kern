use std::collections::HashMap;

use crate::diagnostics::Span;
use crate::hir::LocalId;
use crate::semantic::types::Type;

#[derive(Debug, Clone)]
pub struct LocalInfo {
    pub id: LocalId,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug)]
pub struct SymbolTable {
    scopes: Vec<HashMap<String, LocalInfo>>,
    next_local: u32,
    pub local_types: Vec<Type>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            next_local: 0,
            local_types: Vec::new(),
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        debug_assert!(self.scopes.len() > 1);
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn allocate(&mut self, ty: Type) -> LocalId {
        let id = LocalId(self.next_local);
        self.next_local += 1;
        self.local_types.push(ty);
        id
    }

    /// Declares a local in the current scope. Returns the existing declaration
    /// if the name is already used in this scope.
    pub fn declare(
        &mut self,
        name: String,
        ty: Type,
        mutable: bool,
        span: Span,
    ) -> Result<LocalId, LocalInfo> {
        if let Some(existing) = self.scopes.last().and_then(|scope| scope.get(&name)) {
            return Err(existing.clone());
        }
        let id = self.allocate(ty.clone());
        self.scopes.last_mut().expect("scope stack is non-empty").insert(
            name,
            LocalInfo {
                id,
                ty,
                mutable,
                span,
            },
        );
        Ok(id)
    }

    /// Allocates a local without adding it to a scope (used for parameters).
    pub fn declare_parameter(&mut self, name: String, ty: Type, span: Span) -> LocalId {
        let id = self.allocate(ty.clone());
        self.scopes.last_mut().expect("scope stack is non-empty").insert(
            name,
            LocalInfo {
                id,
                ty,
                mutable: false,
                span,
            },
        );
        id
    }

    pub fn lookup(&self, name: &str) -> Option<&LocalInfo> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub fn reset(&mut self) {
        self.scopes.clear();
        self.scopes.push(HashMap::new());
        self.next_local = 0;
        self.local_types.clear();
    }
}
