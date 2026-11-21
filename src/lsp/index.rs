use crate::diagnostics::Span;
use crate::lexer::{TokenKind, lex};
use crate::parser::ast;
use lsp_types::SymbolKind;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct IndexedSymbol {
    pub name: String,
    pub detail: String,
    pub kind: SymbolKind,
    pub definition: Span,
    pub range: Span,
    pub ty: Option<String>,
    pub parent: Option<usize>,
    pub scope: Option<usize>,
    pub visible_from: usize,
    pub local: bool,
    pub references: Vec<Span>,
}

#[derive(Debug, Clone)]
struct Scope {
    range: Span,
    symbols: Vec<usize>,
    owner: usize,
}

#[derive(Debug, Clone)]
struct FunctionInfo {
    symbol: usize,
    declaration: ast::Function,
}

#[derive(Debug, Clone)]
pub struct SymbolIndex {
    pub symbols: Vec<IndexedSymbol>,
    scopes: Vec<Scope>,
    occurrences: Vec<(Span, usize)>,
    functions: HashMap<String, FunctionInfo>,
    function_definitions: HashMap<Span, usize>,
    ambiguous_functions: HashSet<String>,
    structs: HashMap<String, usize>,
    enums: HashMap<String, usize>,
    enum_variants: HashMap<(String, String), usize>,
}

impl SymbolIndex {
    pub fn new(source: &str, program: &ast::Program) -> Self {
        let mut index = Self {
            symbols: Vec::new(),
            scopes: Vec::new(),
            occurrences: Vec::new(),
            functions: HashMap::new(),
            function_definitions: HashMap::new(),
            ambiguous_functions: HashSet::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            enum_variants: HashMap::new(),
        };

        for item in &program.items {
            match item {
                ast::Item::Function(function) => {
                    let name_span = find_identifier(source, function.span, &function.name)
                        .unwrap_or(function.span);
                    let signature = function_signature(function);
                    let symbol = index.add_symbol(
                        function.name.clone(),
                        signature.clone(),
                        SymbolKind::FUNCTION,
                        name_span,
                        function.span,
                        None,
                        None,
                        0,
                        false,
                        Some(
                            function
                                .return_type
                                .as_ref()
                                .map(type_text)
                                .unwrap_or_else(|| "()".to_string()),
                        ),
                    );
                    index.function_definitions.insert(function.span, symbol);
                    if !index.ambiguous_functions.contains(&function.name) {
                        if index.functions.remove(&function.name).is_some() {
                            index.ambiguous_functions.insert(function.name.clone());
                        } else {
                            index.functions.insert(
                                function.name.clone(),
                                FunctionInfo {
                                    symbol,
                                    declaration: function.clone(),
                                },
                            );
                        }
                    }
                    index.occurrences.push((name_span, symbol));
                }
                ast::Item::Struct(structure) => {
                    let name_span = find_identifier(source, structure.span, &structure.name)
                        .unwrap_or(structure.span);
                    let symbol = index.add_symbol(
                        structure.name.clone(),
                        format!("struct {}", structure.name),
                        SymbolKind::STRUCT,
                        name_span,
                        structure.span,
                        None,
                        None,
                        0,
                        false,
                        Some(structure.name.clone()),
                    );
                    index.structs.insert(structure.name.clone(), symbol);
                    index.occurrences.push((name_span, symbol));
                    for field in &structure.fields {
                        let field_name =
                            find_identifier(source, field.span, &field.name).unwrap_or(field.span);
                        let field_symbol = index.add_symbol(
                            field.name.clone(),
                            format!("{}: {}", field.name, type_text(&field.ty)),
                            SymbolKind::FIELD,
                            field_name,
                            field.span,
                            Some(symbol),
                            None,
                            0,
                            false,
                            Some(type_text(&field.ty)),
                        );
                        index.occurrences.push((field_name, field_symbol));
                    }
                }
                ast::Item::Enum(enumeration) => {
                    let name_span = find_identifier(source, enumeration.span, &enumeration.name)
                        .unwrap_or(enumeration.span);
                    let symbol = index.add_symbol(
                        enumeration.name.clone(),
                        format!("enum {}", enumeration.name),
                        SymbolKind::ENUM,
                        name_span,
                        enumeration.span,
                        None,
                        None,
                        0,
                        false,
                        Some(enumeration.name.clone()),
                    );
                    index.enums.insert(enumeration.name.clone(), symbol);
                    index.occurrences.push((name_span, symbol));
                    for variant in &enumeration.variants {
                        let variant_name = find_identifier(source, variant.span, &variant.name)
                            .unwrap_or(variant.span);
                        let detail = if variant.payload.is_empty() {
                            format!("{}::{}", enumeration.name, variant.name)
                        } else {
                            format!("{}::{}(…)", enumeration.name, variant.name)
                        };
                        let variant_symbol = index.add_symbol(
                            variant.name.clone(),
                            detail,
                            SymbolKind::ENUM_MEMBER,
                            variant_name,
                            variant.span,
                            Some(symbol),
                            None,
                            0,
                            false,
                            Some(enumeration.name.clone()),
                        );
                        index.enum_variants.insert(
                            (enumeration.name.clone(), variant.name.clone()),
                            variant_symbol,
                        );
                        index.occurrences.push((variant_name, variant_symbol));
                    }
                }
                ast::Item::Use(_) => {}
            }
        }

        for item in &program.items {
            match item {
                ast::Item::Function(function) => index.index_function(source, function),
                ast::Item::Struct(structure) => {
                    for field in &structure.fields {
                        index.index_type(source, &field.ty);
                    }
                }
                ast::Item::Enum(enumeration) => {
                    for variant in &enumeration.variants {
                        for ty in &variant.payload {
                            index.index_type(source, ty);
                        }
                    }
                }
                ast::Item::Use(_) => {}
            }
        }
        index
    }

    fn add_symbol(
        &mut self,
        name: String,
        detail: String,
        kind: SymbolKind,
        definition: Span,
        range: Span,
        parent: Option<usize>,
        scope: Option<usize>,
        visible_from: usize,
        local: bool,
        ty: Option<String>,
    ) -> usize {
        let id = self.symbols.len();
        self.symbols.push(IndexedSymbol {
            name,
            detail,
            kind,
            definition,
            range,
            ty,
            parent,
            scope,
            visible_from,
            local,
            references: Vec::new(),
        });
        if let Some(scope) = scope {
            self.scopes[scope].symbols.push(id);
        }
        id
    }

    fn add_scope(&mut self, range: Span, owner: usize) -> usize {
        let id = self.scopes.len();
        self.scopes.push(Scope {
            range,
            symbols: Vec::new(),
            owner,
        });
        id
    }

    fn index_function(&mut self, source: &str, function: &ast::Function) {
        let function_symbol = self
            .function_definitions
            .get(&function.span)
            .copied()
            .unwrap_or(0);
        let scope = self.add_scope(function.body.span, function_symbol);
        let mut active_scopes = vec![scope];

        for parameter in &function.params {
            let name_span =
                find_identifier(source, parameter.span, &parameter.name).unwrap_or(parameter.span);
            let ty = type_text(&parameter.ty);
            let symbol = self.add_symbol(
                parameter.name.clone(),
                format!("{}: {ty}", parameter.name),
                SymbolKind::VARIABLE,
                name_span,
                parameter.span,
                Some(function_symbol),
                Some(scope),
                function.body.span.start,
                true,
                Some(ty),
            );
            self.occurrences.push((name_span, symbol));
            self.index_type(source, &parameter.ty);
        }
        if let Some(return_type) = &function.return_type {
            self.index_type(source, return_type);
        }
        self.index_block_contents(source, &function.body, &mut active_scopes);
    }

    fn index_block_contents(&mut self, source: &str, block: &ast::Block, scopes: &mut Vec<usize>) {
        for statement in &block.statements {
            self.index_statement(source, statement, scopes);
        }
    }

    fn index_nested_block(&mut self, source: &str, block: &ast::Block, scopes: &mut Vec<usize>) {
        let owner = self.scopes[*scopes.last().expect("function scope is active")].owner;
        let scope = self.add_scope(block.span, owner);
        scopes.push(scope);
        self.index_block_contents(source, block, scopes);
        scopes.pop();
    }

    fn index_statement(&mut self, source: &str, statement: &ast::Stmt, scopes: &mut Vec<usize>) {
        match &statement.kind {
            ast::StmtKind::Let {
                name, ty, value, ..
            } => {
                if let Some(ty) = ty {
                    self.index_type(source, ty);
                }
                if let Some(value) = value {
                    self.index_expr(source, value, scopes);
                }
                let name_span =
                    find_identifier(source, statement.span, name).unwrap_or(statement.span);
                let inferred = ty.as_ref().map(type_text).or_else(|| {
                    value
                        .as_ref()
                        .and_then(|expr| self.infer_expr_type(expr, scopes))
                });
                let detail = inferred
                    .as_ref()
                    .map(|ty| format!("{name}: {ty}"))
                    .unwrap_or_else(|| name.clone());
                let symbol = self.add_symbol(
                    name.clone(),
                    detail,
                    SymbolKind::VARIABLE,
                    name_span,
                    statement.span,
                    Some(self.scopes[*scopes.last().expect("function scope is active")].owner),
                    scopes.last().copied(),
                    name_span.end,
                    true,
                    inferred,
                );
                self.occurrences.push((name_span, symbol));
            }
            ast::StmtKind::Assign { target, value, .. } => {
                self.index_expr(source, target, scopes);
                self.index_expr(source, value, scopes);
            }
            ast::StmtKind::Expr(expr) => self.index_expr(source, expr, scopes),
            ast::StmtKind::Print { args, .. } => {
                for arg in args {
                    match arg {
                        ast::PrintArg::Expr(expr) => self.index_expr(source, expr, scopes),
                        ast::PrintArg::Template(parts) => {
                            for part in parts {
                                if let ast::TemplatePart::Interpolation(expr) = part {
                                    self.index_expr(source, expr, scopes);
                                }
                            }
                        }
                    }
                }
            }
            ast::StmtKind::Cin(targets) => {
                for target in targets {
                    self.index_expr(source, target, scopes);
                }
            }
            ast::StmtKind::If {
                condition,
                then_block,
                else_branch,
            } => {
                self.index_expr(source, condition, scopes);
                self.index_nested_block(source, then_block, scopes);
                match else_branch {
                    Some(ast::ElseBranch::Block(block)) => {
                        self.index_nested_block(source, block, scopes)
                    }
                    Some(ast::ElseBranch::If(statement)) => {
                        self.index_statement(source, statement, scopes)
                    }
                    None => {}
                }
            }
            ast::StmtKind::While { condition, body } => {
                self.index_expr(source, condition, scopes);
                self.index_nested_block(source, body, scopes);
            }
            ast::StmtKind::Loop { body } => self.index_nested_block(source, body, scopes),
            ast::StmtKind::For {
                init,
                condition,
                update,
                body,
            } => {
                let owner = self.scopes[*scopes.last().expect("function scope is active")].owner;
                let scope = self.add_scope(statement.span, owner);
                scopes.push(scope);
                if let Some(init) = init {
                    self.index_statement(source, init, scopes);
                }
                if let Some(condition) = condition {
                    self.index_expr(source, condition, scopes);
                }
                if let Some(update) = update {
                    self.index_statement(source, update, scopes);
                }
                self.index_nested_block(source, body, scopes);
                scopes.pop();
            }
            ast::StmtKind::Break | ast::StmtKind::Continue => {}
            ast::StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.index_expr(source, value, scopes);
                }
            }
            ast::StmtKind::Block(block) => self.index_nested_block(source, block, scopes),
            ast::StmtKind::Match { scrutinee, arms } => {
                self.index_expr(source, scrutinee, scopes);
                for arm in arms {
                    if let ast::Pattern::EnumVariant { path, span } = &arm.pattern {
                        self.index_path(source, path, *span);
                    }
                    self.index_nested_block(source, &arm.body, scopes);
                }
            }
        }
    }

    fn index_expr(&mut self, source: &str, expr: &ast::Expr, scopes: &[usize]) {
        match &expr.kind {
            ast::ExprKind::Var(name) => {
                if let Some(symbol) = self.resolve_local(name, expr.span.start, scopes) {
                    self.add_reference(expr.span, symbol);
                }
            }
            ast::ExprKind::Path(path) => self.index_path(source, path, expr.span),
            ast::ExprKind::Unary { expr, .. } => self.index_expr(source, expr, scopes),
            ast::ExprKind::Binary { left, right, .. } => {
                self.index_expr(source, left, scopes);
                self.index_expr(source, right, scopes);
            }
            ast::ExprKind::Call { callee, args } => {
                if let Some(function) = self.functions.get(callee) {
                    let name_span = find_identifier(source, expr.span, callee).unwrap_or(expr.span);
                    self.add_reference(name_span, function.symbol);
                }
                for arg in args {
                    self.index_expr(source, arg, scopes);
                }
            }
            ast::ExprKind::Field { base, name } => {
                self.index_expr(source, base, scopes);
                let field_owner = self
                    .infer_expr_type(base, scopes)
                    .and_then(|name| self.structs.get(&name).copied());
                let candidate = field_owner
                    .map(|owner| self.find_child_symbol(owner, name))
                    .unwrap_or_else(|| self.unique_field(name));
                if let Some(symbol) = candidate {
                    let name_span = find_identifier(source, expr.span, name).unwrap_or(expr.span);
                    self.add_reference(name_span, symbol);
                }
            }
            ast::ExprKind::StructLiteral { name, fields } => {
                let struct_id = self.structs.get(name).copied();
                if let Some(symbol) = struct_id {
                    let name_span = find_identifier(source, expr.span, name).unwrap_or(expr.span);
                    self.add_reference(name_span, symbol);
                }
                for field in fields {
                    if let Some(owner) = struct_id {
                        if let Some(symbol) = self.find_child_symbol(owner, &field.name) {
                            let name_span = find_identifier(source, field.span, &field.name)
                                .unwrap_or(field.span);
                            self.add_reference(name_span, symbol);
                        }
                    }
                    self.index_expr(source, &field.value, scopes);
                }
            }
            ast::ExprKind::Index { base, index } => {
                self.index_expr(source, base, scopes);
                self.index_expr(source, index, scopes);
            }
            ast::ExprKind::Int { .. }
            | ast::ExprKind::Float { .. }
            | ast::ExprKind::Bool(_)
            | ast::ExprKind::Char(_)
            | ast::ExprKind::Str(_)
            | ast::ExprKind::Unit => {}
        }
    }

    fn index_path(&mut self, source: &str, path: &[String], span: Span) {
        if path.len() != 2 {
            return;
        }
        if let Some(enum_id) = self.enums.get(&path[0]).copied() {
            let type_span = find_identifier(source, span, &path[0]).unwrap_or(span);
            self.add_reference(type_span, enum_id);
            if let Some(variant) = self
                .enum_variants
                .get(&(path[0].clone(), path[1].clone()))
                .copied()
            {
                let variant_span =
                    find_identifier_after(source, span, &path[0], &path[1]).unwrap_or(span);
                self.add_reference(variant_span, variant);
            }
        }
    }

    fn index_type(&mut self, source: &str, ty: &ast::TypeExpr) {
        match &ty.kind {
            ast::TypeExprKind::Named(name) => {
                if let Some(symbol) = self.enums.get(name).or_else(|| self.structs.get(name)) {
                    let name_span = find_identifier(source, ty.span, name).unwrap_or(ty.span);
                    self.add_reference(name_span, *symbol);
                }
            }
            ast::TypeExprKind::Generic { name, args } => {
                if let Some(symbol) = self.structs.get(name) {
                    let name_span = find_identifier(source, ty.span, name).unwrap_or(ty.span);
                    self.add_reference(name_span, *symbol);
                }
                for arg in args {
                    self.index_type(source, arg);
                }
            }
            ast::TypeExprKind::Array { element, .. }
            | ast::TypeExprKind::Ref { inner: element, .. } => self.index_type(source, element),
            ast::TypeExprKind::Tuple(items) => {
                for item in items {
                    self.index_type(source, item);
                }
            }
            ast::TypeExprKind::Unit => {}
        }
    }

    fn resolve_local(&self, name: &str, offset: usize, scopes: &[usize]) -> Option<usize> {
        for scope_id in scopes.iter().rev() {
            let scope = self.scopes.get(*scope_id)?;
            if offset < scope.range.start || offset > scope.range.end {
                continue;
            }
            if let Some(symbol) = scope.symbols.iter().rev().find(|symbol| {
                let symbol = &self.symbols[**symbol];
                symbol.name == name && symbol.visible_from <= offset
            }) {
                return Some(*symbol);
            }
        }
        None
    }

    fn infer_expr_type(&self, expr: &ast::Expr, scopes: &[usize]) -> Option<String> {
        match &expr.kind {
            ast::ExprKind::Int { suffix, .. } => {
                Some(suffix.clone().unwrap_or_else(|| "i32".to_string()))
            }
            ast::ExprKind::Float { suffix, .. } => {
                Some(suffix.clone().unwrap_or_else(|| "f64".to_string()))
            }
            ast::ExprKind::Bool(_) => Some("bool".to_string()),
            ast::ExprKind::Char(_) => Some("char".to_string()),
            ast::ExprKind::Str(_) => Some("String".to_string()),
            ast::ExprKind::Var(name) => self
                .resolve_local(name, expr.span.start, scopes)
                .and_then(|symbol| self.symbols[symbol].ty.clone()),
            ast::ExprKind::Path(path) if path.len() == 2 => Some(path[0].clone()),
            ast::ExprKind::StructLiteral { name, .. } => Some(name.clone()),
            ast::ExprKind::Call { callee, .. } => self
                .functions
                .get(callee)
                .and_then(|function| function.declaration.return_type.as_ref())
                .map(type_text),
            ast::ExprKind::Binary { left, .. } => self.infer_expr_type(left, scopes),
            ast::ExprKind::Unary { expr, .. } => self.infer_expr_type(expr, scopes),
            ast::ExprKind::Field { .. }
            | ast::ExprKind::Index { .. }
            | ast::ExprKind::Path(_)
            | ast::ExprKind::Unit => None,
        }
    }

    fn find_child_symbol(&self, parent: usize, name: &str) -> Option<usize> {
        self.symbols
            .iter()
            .position(|symbol| symbol.parent == Some(parent) && symbol.name == name)
    }

    fn unique_field(&self, name: &str) -> Option<usize> {
        let mut matching = self
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, symbol)| symbol.kind == SymbolKind::FIELD && symbol.name == name)
            .map(|(index, _)| index);
        let first = matching.next()?;
        matching.next().is_none().then_some(first)
    }

    fn add_reference(&mut self, span: Span, symbol: usize) {
        if let Some(entry) = self.symbols.get_mut(symbol) {
            entry.references.push(span);
        }
        self.occurrences.push((span, symbol));
    }

    pub fn symbol_at(&self, offset: usize) -> Option<usize> {
        self.occurrences
            .iter()
            .filter(|(span, _)| span.start <= offset && offset <= span.end)
            .min_by_key(|(span, _)| span.end.saturating_sub(span.start))
            .map(|(_, symbol)| *symbol)
    }

    pub fn occurrence_at(&self, offset: usize) -> Option<Span> {
        self.occurrences
            .iter()
            .filter(|(span, _)| span.start <= offset && offset <= span.end)
            .min_by_key(|(span, _)| span.end.saturating_sub(span.start))
            .map(|(span, _)| *span)
    }

    pub fn references(&self, symbol: usize) -> Vec<Span> {
        self.symbols
            .get(symbol)
            .map(|entry| {
                let mut references = entry.references.clone();
                references.push(entry.definition);
                references.sort_by_key(|span| span.start);
                references.dedup();
                references
            })
            .unwrap_or_default()
    }

    pub fn visible_symbols(&self, offset: usize) -> Vec<usize> {
        let mut visible: HashMap<String, usize> = HashMap::new();
        for (index, symbol) in self.symbols.iter().enumerate() {
            if !symbol.local && symbol.parent.is_none() {
                visible.insert(symbol.name.clone(), index);
            } else if let Some(scope_id) = symbol.scope {
                let scope = &self.scopes[scope_id];
                if scope.range.start <= offset
                    && offset <= scope.range.end
                    && symbol.visible_from <= offset
                {
                    let replace = visible.get(&symbol.name).is_none_or(|old| {
                        let old = &self.symbols[*old];
                        old.scope
                            .map(|id| {
                                self.scopes[scope_id]
                                    .range
                                    .end
                                    .saturating_sub(self.scopes[scope_id].range.start)
                                    < self.scopes[id]
                                        .range
                                        .end
                                        .saturating_sub(self.scopes[id].range.start)
                            })
                            .unwrap_or(true)
                    });
                    if replace {
                        visible.insert(symbol.name.clone(), index);
                    }
                }
            }
        }
        let mut values: Vec<_> = visible.into_values().collect();
        values.sort_by(|left, right| self.symbols[*left].name.cmp(&self.symbols[*right].name));
        values
    }

    pub fn global_symbols(&self) -> impl Iterator<Item = (usize, &IndexedSymbol)> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|(_, symbol)| !symbol.local && symbol.parent.is_none())
    }

    pub fn children(&self, parent: usize) -> Vec<(usize, &IndexedSymbol)> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|(_, symbol)| symbol.parent == Some(parent))
            .collect()
    }

    pub fn enum_variants_for(&self, name: &str) -> Vec<usize> {
        let mut variants: Vec<_> = self
            .enum_variants
            .iter()
            .filter_map(|((enum_name, _), symbol)| (enum_name == name).then_some(*symbol))
            .collect();
        variants.sort_by(|left, right| self.symbols[*left].name.cmp(&self.symbols[*right].name));
        variants
    }
}

fn function_signature(function: &ast::Function) -> String {
    let parameters = function
        .params
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, type_text(&parameter.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = function
        .return_type
        .as_ref()
        .map(|ty| format!(" -> {}", type_text(ty)))
        .unwrap_or_default();
    format!("fn {}({parameters}){return_type}", function.name)
}

fn type_text(ty: &ast::TypeExpr) -> String {
    match &ty.kind {
        ast::TypeExprKind::Unit => "()".to_string(),
        ast::TypeExprKind::Named(name) => name.clone(),
        ast::TypeExprKind::Generic { name, args } => format!(
            "{}<{}>",
            name,
            args.iter().map(type_text).collect::<Vec<_>>().join(", ")
        ),
        ast::TypeExprKind::Array { element, len } => format!("[{}; {len}]", type_text(element)),
        ast::TypeExprKind::Tuple(items) => {
            format!(
                "({})",
                items.iter().map(type_text).collect::<Vec<_>>().join(", ")
            )
        }
        ast::TypeExprKind::Ref { mutable, inner } => format!(
            "&{}{}",
            if *mutable { "mut " } else { "" },
            type_text(inner)
        ),
    }
}

fn find_identifier(source: &str, span: Span, name: &str) -> Option<Span> {
    find_identifier_after(source, span, "", name)
}

fn find_identifier_after(source: &str, span: Span, first: &str, name: &str) -> Option<Span> {
    let start = span.start.min(source.len());
    let end = span.end.min(source.len());
    let text = source.get(start..end)?;
    let mut previous_match = None;
    for (offset, _) in text.match_indices(name) {
        let absolute = start + offset;
        let before = text[..offset].chars().next_back();
        let after = text[offset + name.len()..].chars().next();
        if before.is_none_or(|character| !is_identifier_continue(character))
            && after.is_none_or(|character| !is_identifier_continue(character))
        {
            let candidate_span = Span::new(absolute, absolute + name.len());
            if first.is_empty() {
                return Some(candidate_span);
            }
            previous_match = Some(candidate_span);
        }
    }
    if first.is_empty() {
        return None;
    }
    previous_match
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

pub fn token_is_string_or_char(source: &str, offset: usize) -> bool {
    for token in &lex(source).tokens {
        if token.span.start > offset || token.span.end < offset {
            continue;
        }
        match token.kind {
            TokenKind::Char(_) => return true,
            TokenKind::Str { .. } => {
                let raw_start = token.span.start.saturating_add(1);
                let raw_end = token.span.end.saturating_sub(1);
                let Some(raw) = source.get(raw_start..raw_end) else {
                    return true;
                };
                let mut cursor = 0;
                while cursor < raw.len() {
                    if raw.as_bytes()[cursor] == b'{' {
                        if raw.as_bytes().get(cursor + 1) == Some(&b'{') {
                            cursor += 2;
                            continue;
                        }
                        if let Some(close) = string_interpolation_close(raw, cursor + 1) {
                            let body = &raw[cursor + 1..close];
                            let absolute = raw_start + cursor;
                            if offset > absolute
                                && offset < raw_start + close
                                && !body.starts_with(':')
                            {
                                return false;
                            }
                            cursor = close + 1;
                            continue;
                        }
                    }
                    cursor += raw[cursor..].chars().next().unwrap().len_utf8();
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

fn string_interpolation_close(raw: &str, start: usize) -> Option<usize> {
    let bytes = raw.as_bytes();
    let mut nested = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut cursor = start;
    while cursor < bytes.len() {
        let current = bytes[cursor];
        if escaped {
            escaped = false;
        } else if current == b'\\' {
            escaped = true;
        } else if let Some(active) = quote {
            if current == active {
                quote = None;
            }
        } else {
            match current {
                b'"' | b'\'' => quote = Some(current),
                b'{' => nested += 1,
                b'}' if nested == 0 => return Some(cursor),
                b'}' => nested -= 1,
                _ => {}
            }
        }
        cursor += 1;
    }
    None
}
