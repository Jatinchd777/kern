use std::collections::{HashMap, HashSet};

use crate::diagnostics::{Diagnostic, Diagnostics, Span};
use crate::hir::{
    Block, ElseBranch, EnumDef, EnumId, Expr, ExprKind, Field, Function, FunctionId, LocalId,
    MatchArm, MatchPattern, Param, PrintItem, Program, Stmt, StmtKind, StructDef, StructId,
    Variant,
};
use crate::parser::ast::{self, AssignOp, BinaryOp, UnaryOp};
use crate::semantic::symbols::SymbolTable;
use crate::semantic::types::Type;

pub struct AnalysisResult {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn analyze(program: ast::Program) -> AnalysisResult {
    let mut analyzer = Analyzer::new();
    analyzer.collect_type_names(&program);
    analyzer.resolve_structs();
    analyzer.resolve_enums();
    analyzer.check_imports(&program);
    analyzer.collect_functions(&program);
    analyzer.check_struct_cycles();

    let mut functions = Vec::new();
    for index in 0..analyzer.functions.len() {
        let function = analyzer.functions[index].clone();
        functions.push(analyzer.check_function(&function));
    }

    let structs = analyzer
        .structs
        .iter()
        .enumerate()
        .map(|(index, info)| StructDef {
            id: StructId(index),
            name: info.name.clone(),
            fields: info.fields.clone(),
            span: info.span,
        })
        .collect();
    let enums = analyzer
        .enums
        .iter()
        .enumerate()
        .map(|(index, info)| EnumDef {
            id: EnumId(index),
            name: info.name.clone(),
            variants: info.variants.clone(),
            span: info.span,
        })
        .collect();

    AnalysisResult {
        program: Program {
            structs,
            enums,
            functions,
        },
        diagnostics: analyzer.diagnostics.into_vec(),
    }
}

#[derive(Clone)]
struct StructInfo {
    name: String,
    ast_fields: Vec<ast::FieldDecl>,
    fields: Vec<Field>,
    span: Span,
}

#[derive(Clone)]
struct EnumInfo {
    name: String,
    ast_variants: Vec<ast::EnumVariantDecl>,
    variants: Vec<Variant>,
    span: Span,
}

#[derive(Clone)]
struct ParamInfo {
    name: String,
    ty: Type,
    span: Span,
}

#[derive(Clone)]
struct FunctionInfo {
    id: FunctionId,
    name: String,
    params: Vec<ParamInfo>,
    return_type: Type,
    body: ast::Block,
    span: Span,
}

struct Analyzer {
    diagnostics: Diagnostics,
    structs: Vec<StructInfo>,
    struct_names: HashMap<String, StructId>,
    enums: Vec<EnumInfo>,
    enum_names: HashMap<String, EnumId>,
    functions: Vec<FunctionInfo>,
    function_names: HashMap<String, FunctionId>,
    symbols: SymbolTable,
    current_return_type: Type,
    loop_depth: usize,
}

impl Analyzer {
    fn new() -> Self {
        Self {
            diagnostics: Diagnostics::new(),
            structs: Vec::new(),
            struct_names: HashMap::new(),
            enums: Vec::new(),
            enum_names: HashMap::new(),
            functions: Vec::new(),
            function_names: HashMap::new(),
            symbols: SymbolTable::new(),
            current_return_type: Type::Unit,
            loop_depth: 0,
        }
    }

    fn collect_type_names(&mut self, program: &ast::Program) {
        for item in &program.items {
            match item {
                ast::Item::Struct(decl) => {
                    if let Some(note) = self.builtin_type_note(&decl.name) {
                        self.diagnostics.error(decl.span, note);
                        continue;
                    }
                    if let Some(previous) = self.type_definition_span(&decl.name) {
                        let _ = previous;
                        self.diagnostics.error_with_note(
                            decl.span,
                            format!("type `{}` is declared more than once", decl.name),
                            "type names share one namespace; rename one of the declarations",
                        );
                        continue;
                    }
                    let id = StructId(self.structs.len());
                    self.struct_names.insert(decl.name.clone(), id);
                    self.structs.push(StructInfo {
                        name: decl.name.clone(),
                        ast_fields: decl.fields.clone(),
                        fields: Vec::new(),
                        span: decl.span,
                    });
                }
                ast::Item::Enum(decl) => {
                    if let Some(note) = self.builtin_type_note(&decl.name) {
                        self.diagnostics.error(decl.span, note);
                        continue;
                    }
                    if let Some(previous) = self.type_definition_span(&decl.name) {
                        let _ = previous;
                        self.diagnostics.error_with_note(
                            decl.span,
                            format!("type `{}` is declared more than once", decl.name),
                            "type names share one namespace; rename one of the declarations",
                        );
                        continue;
                    }
                    let id = EnumId(self.enums.len());
                    self.enum_names.insert(decl.name.clone(), id);
                    self.enums.push(EnumInfo {
                        name: decl.name.clone(),
                        ast_variants: decl.variants.clone(),
                        variants: Vec::new(),
                        span: decl.span,
                    });
                }
                _ => {}
            }
        }
    }

    fn builtin_type_note(&self, name: &str) -> Option<String> {
        if is_builtin_type_name(name) {
            Some(format!(
                "`{name}` is a built-in type and cannot be redefined"
            ))
        } else {
            None
        }
    }

    fn type_definition_span(&self, name: &str) -> Option<Span> {
        if let Some(id) = self.struct_names.get(name) {
            return self.structs.get(id.0).map(|info| info.span);
        }
        if let Some(id) = self.enum_names.get(name) {
            return self.enums.get(id.0).map(|info| info.span);
        }
        None
    }

    fn resolve_structs(&mut self) {
        for index in 0..self.structs.len() {
            let fields = self.structs[index].ast_fields.clone();
            let mut resolved = Vec::new();
            let mut seen: HashMap<String, Span> = HashMap::new();
            for field in fields {
                if let Some(previous) = seen.get(&field.name) {
                    let _ = previous;
                    self.diagnostics.error_with_note(
                        field.span,
                        format!(
                            "field `{}` is declared more than once in struct `{}`",
                            field.name, self.structs[index].name
                        ),
                        "field names must be unique within a struct",
                    );
                } else {
                    seen.insert(field.name.clone(), field.span);
                }
                let ty = self.resolve_type(&field.ty);
                if ty.is_unit() {
                    self.diagnostics.error_with_note(
                        field.span,
                        format!("struct field `{}` cannot have unit type `()`", field.name),
                        "unit values carry no data and cannot be stored in a struct",
                    );
                }
                resolved.push(Field {
                    name: field.name,
                    ty,
                    span: field.span,
                });
            }
            self.structs[index].fields = resolved;
        }
    }

    fn resolve_enums(&mut self) {
        for index in 0..self.enums.len() {
            let variants = self.enums[index].ast_variants.clone();
            let mut resolved = Vec::new();
            let mut seen: HashMap<String, Span> = HashMap::new();
            for (tag, variant) in variants.into_iter().enumerate() {
                if let Some(previous) = seen.get(&variant.name) {
                    let _ = previous;
                    self.diagnostics.error_with_note(
                        variant.span,
                        format!(
                            "variant `{}` is declared more than once in enum `{}`",
                            variant.name, self.enums[index].name
                        ),
                        "variant names must be unique within an enum",
                    );
                } else {
                    seen.insert(variant.name.clone(), variant.span);
                }
                if !variant.payload.is_empty() {
                    self.diagnostics.error_with_note(
                        variant.span,
                        "enum variants with payloads are not implemented yet",
                        "only fieldless enums are supported in this milestone",
                    );
                }
                resolved.push(Variant {
                    name: variant.name,
                    tag: tag as u32,
                    span: variant.span,
                });
            }
            self.enums[index].variants = resolved;
        }
    }

    fn check_imports(&mut self, program: &ast::Program) {
        for item in &program.items {
            if let ast::Item::Use(import) = item {
                self.diagnostics.error_with_note(
                    import.span,
                    "module imports are not implemented yet",
                    format!(
                        "`use {}` was parsed, but no module resolver exists in this milestone",
                        import.path.join("::")
                    ),
                );
            }
        }
    }

    fn collect_functions(&mut self, program: &ast::Program) {
        for item in &program.items {
            let ast::Item::Function(function) = item else {
                continue;
            };
            if let Some(previous) = self.function_names.get(&function.name) {
                let _ = previous;
                self.diagnostics.error_with_note(
                    function.span,
                    format!("function `{}` is declared more than once", function.name),
                    "function names must be unique in a program",
                );
                continue;
            }

            let mut params = Vec::new();
            let mut seen_params: HashSet<String> = HashSet::new();
            for param in &function.params {
                if !seen_params.insert(param.name.clone()) {
                    self.diagnostics.error_with_note(
                        param.span,
                        format!(
                            "parameter `{}` is declared more than once in function `{}`",
                            param.name, function.name
                        ),
                        "parameter names must be unique",
                    );
                    continue;
                }
                let ty = self.resolve_type(&param.ty);
                if ty.is_unit() {
                    self.diagnostics.error_with_note(
                        param.span,
                        format!("parameter `{}` cannot have unit type `()`", param.name),
                        "unit values carry no data and cannot be passed as arguments",
                    );
                }
                params.push(ParamInfo {
                    name: param.name.clone(),
                    ty,
                    span: param.span,
                });
            }

            let return_type = function
                .return_type
                .as_ref()
                .map(|ty| self.resolve_type(ty))
                .unwrap_or(Type::Unit);

            if function.name == "main" {
                if !params.is_empty() {
                    self.diagnostics.error_with_note(
                        function.span,
                        "`main` must not take parameters",
                        "the entry point is called by the operating system with no Kern arguments",
                    );
                }
                if return_type != Type::Unit && return_type != Type::I32 && !return_type.is_error()
                {
                    let type_name = self.type_name(&return_type);
                    self.diagnostics.error_with_note(
                        function.span,
                        format!("`main` must return `()` or `i32`, found `{type_name}`"),
                        "use `fn main()` for a process exit status of zero, or `fn main() -> i32`",
                    );
                }
            }

            let id = FunctionId(self.functions.len());
            self.function_names.insert(function.name.clone(), id);
            self.functions.push(FunctionInfo {
                id,
                name: function.name.clone(),
                params,
                return_type,
                body: function.body.clone(),
                span: function.span,
            });
        }
    }

    fn check_struct_cycles(&mut self) {
        for index in 0..self.structs.len() {
            let mut path = Vec::new();
            if self.find_struct_cycle(StructId(index), &mut path).is_some() {
                let name = self.structs[index].name.clone();
                let span = self.structs[index].span;
                self.diagnostics.error_with_note(
                    span,
                    format!("recursive struct `{name}` has infinite size"),
                    "reference types are not implemented yet, so a by-value cycle cannot be broken",
                );
            }
        }
    }

    fn find_struct_cycle(&self, id: StructId, path: &mut Vec<StructId>) -> Option<Vec<StructId>> {
        if path.contains(&id) {
            let mut cycle = path.clone();
            cycle.push(id);
            return Some(cycle);
        }
        path.push(id);
        let fields = self.structs.get(id.0)?.fields.clone();
        for field in &fields {
            if let Type::Struct(next) = field.ty {
                if let Some(cycle) = self.find_struct_cycle(next, path) {
                    return Some(cycle);
                }
            }
        }
        path.pop();
        None
    }

    fn check_function(&mut self, function: &FunctionInfo) -> Function {
        self.symbols.reset();
        self.current_return_type = function.return_type.clone();
        self.loop_depth = 0;

        let mut params = Vec::new();
        for param in &function.params {
            let local =
                self.symbols
                    .declare_parameter(param.name.clone(), param.ty.clone(), param.span);
            params.push(Param {
                local,
                name: param.name.clone(),
                ty: param.ty.clone(),
                span: param.span,
            });
        }

        // The function body shares the scope containing its parameters.
        let body = self.check_block_contents(&function.body);

        if !function.return_type.is_unit()
            && !function.return_type.is_error()
            && !block_guarantees_return(&body)
        {
            let return_name = self.type_name(&function.return_type);
            let message = format!(
                "function `{}` may reach the end of its body without returning a value of type `{return_name}`",
                function.name
            );
            self.diagnostics.error_with_note(
                function.body.span,
                message,
                "add a `return` on every path, or make the function return `()`",
            );
        }

        Function {
            id: function.id,
            name: function.name.clone(),
            params,
            return_type: function.return_type.clone(),
            body,
            local_types: self.symbols.local_types.clone(),
            span: function.span,
        }
    }

    fn check_block(&mut self, block: &ast::Block) -> Block {
        self.symbols.enter_scope();
        let result = self.check_block_contents(block);
        self.symbols.exit_scope();
        result
    }

    fn check_block_contents(&mut self, block: &ast::Block) -> Block {
        let statements = block
            .statements
            .iter()
            .map(|statement| self.check_statement(statement))
            .collect();
        Block {
            statements,
            span: block.span,
        }
    }

    fn check_statement(&mut self, statement: &ast::Stmt) -> Stmt {
        let kind = match &statement.kind {
            ast::StmtKind::Let {
                name,
                mutable,
                ty,
                value,
            } => self.check_let(statement.span, name, *mutable, ty.as_ref(), value.as_ref()),
            ast::StmtKind::Assign { target, op, value } => {
                self.check_assignment(statement.span, target, *op, value)
            }
            ast::StmtKind::Expr(expr) => StmtKind::Expr(self.check_expr(expr, None)),
            ast::StmtKind::Print { args, newline } => self.check_print(args, *newline),
            ast::StmtKind::Cin(targets) => {
                let mut locals = Vec::new();
                for target in targets {
                    let (local, ty, mutable) = match &target.kind {
                        ast::ExprKind::Var(name) => match self.symbols.lookup(name) {
                            Some(info) => (info.id, info.ty.clone(), info.mutable),
                            None => {
                                self.diagnostics.error(
                                    target.span,
                                    format!("cannot read into undefined variable `{name}`"),
                                );
                                (LocalId(u32::MAX), Type::Error, true)
                            }
                        },
                        _ => {
                            self.diagnostics
                                .error(target.span, "cin targets must be local variables");
                            (LocalId(u32::MAX), Type::Error, true)
                        }
                    };
                    if !mutable {
                        self.diagnostics.error_with_note(
                            target.span,
                            "cannot read into an immutable variable",
                            "declare it with `let mut` to make it mutable",
                        );
                    }
                    if !ty.is_numeric() && !matches!(ty, Type::Bool | Type::Char) && !ty.is_error()
                    {
                        self.diagnostics.error(
                            target.span,
                            format!(
                                "cannot read a value of type `{}` with cin",
                                self.type_name(&ty)
                            ),
                        );
                    }
                    if matches!(ty, Type::I128 | Type::U128) {
                        self.diagnostics
                            .error(target.span, "cin does not support 128-bit integers");
                    }
                    locals.push(local);
                }
                StmtKind::Cin(locals)
            }
            ast::StmtKind::If {
                condition,
                then_block,
                else_branch,
            } => {
                let condition = self.check_expr(condition, Some(&Type::Bool));
                let then_block = self.check_block(then_block);
                let else_branch = match else_branch {
                    Some(ast::ElseBranch::Block(block)) => {
                        Some(ElseBranch::Block(self.check_block(block)))
                    }
                    Some(ast::ElseBranch::If(statement)) => {
                        Some(ElseBranch::If(Box::new(self.check_statement(statement))))
                    }
                    None => None,
                };
                StmtKind::If {
                    condition,
                    then_block,
                    else_branch,
                }
            }
            ast::StmtKind::While { condition, body } => {
                let condition = self.check_expr(condition, Some(&Type::Bool));
                self.loop_depth += 1;
                let body = self.check_block(body);
                self.loop_depth -= 1;
                StmtKind::While { condition, body }
            }
            ast::StmtKind::Loop { body } => {
                self.loop_depth += 1;
                let body = self.check_block(body);
                self.loop_depth -= 1;
                StmtKind::Loop { body }
            }
            ast::StmtKind::For {
                init,
                condition,
                update,
                body,
            } => {
                self.symbols.enter_scope();
                let init = init
                    .as_ref()
                    .map(|statement| Box::new(self.check_statement(statement)));
                let condition = condition
                    .as_ref()
                    .map(|expr| self.check_expr(expr, Some(&Type::Bool)));
                let update = update
                    .as_ref()
                    .map(|statement| Box::new(self.check_statement(statement)));
                self.loop_depth += 1;
                let body = self.check_block(body);
                self.loop_depth -= 1;
                self.symbols.exit_scope();
                StmtKind::For {
                    init,
                    condition,
                    update,
                    body,
                }
            }
            ast::StmtKind::Break => {
                if self.loop_depth == 0 {
                    self.diagnostics.error_with_note(
                        statement.span,
                        "`break` outside of a loop",
                        "`break` can only be used inside `while`, `loop`, or `for`",
                    );
                }
                StmtKind::Break
            }
            ast::StmtKind::Continue => {
                if self.loop_depth == 0 {
                    self.diagnostics.error_with_note(
                        statement.span,
                        "`continue` outside of a loop",
                        "`continue` can only be used inside `while`, `loop`, or `for`",
                    );
                }
                StmtKind::Continue
            }
            ast::StmtKind::Return(value) => self.check_return(statement.span, value.as_ref()),
            ast::StmtKind::Block(block) => StmtKind::Block(self.check_block(block)),
            ast::StmtKind::Match { scrutinee, arms } => {
                self.check_match(statement.span, scrutinee, arms)
            }
        };

        Stmt {
            kind,
            span: statement.span,
        }
    }

    fn check_let(
        &mut self,
        span: Span,
        name: &str,
        mutable: bool,
        annotation: Option<&ast::TypeExpr>,
        value: Option<&ast::Expr>,
    ) -> StmtKind {
        let declared_type = annotation.map(|ty| self.resolve_type(ty));
        let value = match (&declared_type, value) {
            (Some(declared), Some(expr)) => Some(self.check_expr(expr, Some(declared))),
            (None, Some(expr)) => Some(self.check_expr(expr, None)),
            (Some(_), None) => {
                self.diagnostics.error_with_note(
                    span,
                    "variable declarations require an initializer",
                    "uninitialized locals are not supported yet",
                );
                None
            }
            (None, None) => {
                self.diagnostics.error_with_note(
                    span,
                    "variable declaration needs a type annotation or an initializer",
                    "for example: `let x: i32 = 0;` or `let x = 0;`",
                );
                None
            }
        };

        let ty = match (annotation, &value) {
            (Some(_), _) => declared_type.unwrap_or(Type::Error),
            (None, Some(expr)) => expr.ty.clone(),
            (None, None) => Type::Error,
        };

        if ty.is_unit() {
            self.diagnostics.error_with_note(
                span,
                "cannot bind a value of type `()`",
                "`()` is the unit type and carries no value",
            );
        }

        let local = match self
            .symbols
            .declare(name.to_string(), ty.clone(), mutable, span)
        {
            Ok(local) => local,
            Err(existing) => {
                self.diagnostics.error_with_note(
                    span,
                    format!("variable `{name}` is declared more than once in this scope"),
                    "a variable with this name is already declared in the same scope",
                );
                let _ = existing;
                LocalId(u32::MAX)
            }
        };

        StmtKind::Let {
            local,
            ty,
            mutable,
            value,
        }
    }

    fn check_assignment(
        &mut self,
        span: Span,
        target: &ast::Expr,
        op: AssignOp,
        value: &ast::Expr,
    ) -> StmtKind {
        let (local, target_type, mutable, declaration_span) = match &target.kind {
            ast::ExprKind::Var(name) => match self.symbols.lookup(name) {
                Some(info) => (info.id, info.ty.clone(), info.mutable, info.span),
                None => {
                    self.diagnostics.error(
                        target.span,
                        format!("cannot assign to undefined variable `{name}`"),
                    );
                    (LocalId(u32::MAX), Type::Error, true, target.span)
                }
            },
            _ => {
                self.diagnostics.error_with_note(
                    target.span,
                    "only local variables can be assigned to yet",
                    "field and index assignment will be added with the ownership model",
                );
                (LocalId(u32::MAX), Type::Error, true, target.span)
            }
        };

        if !mutable {
            let _ = declaration_span;
            self.diagnostics.error_with_note(
                target.span,
                "cannot assign to an immutable variable",
                "declare it with `let mut` to make it mutable",
            );
        }

        let value = match op {
            AssignOp::Assign => self.check_expr(value, Some(&target_type)),
            AssignOp::Add | AssignOp::Sub | AssignOp::Mul | AssignOp::Div | AssignOp::Rem => {
                if !target_type.is_numeric() && !target_type.is_error() {
                    let type_name = self.type_name(&target_type);
                    self.diagnostics.error_with_note(
                        target.span,
                        format!(
                            "compound assignment requires a numeric variable, found `{type_name}`"
                        ),
                        "use `x = value` for non-numeric types",
                    );
                }
                let right = self.check_expr(value, Some(&target_type));
                let left = Expr {
                    kind: ExprKind::Local(local),
                    ty: target_type.clone(),
                    span: target.span,
                };
                let binary_op = match op {
                    AssignOp::Add => BinaryOp::Add,
                    AssignOp::Sub => BinaryOp::Sub,
                    AssignOp::Mul => BinaryOp::Mul,
                    AssignOp::Div => BinaryOp::Div,
                    AssignOp::Rem => BinaryOp::Rem,
                    AssignOp::Assign => unreachable!(),
                };
                Expr {
                    kind: ExprKind::Binary {
                        left: Box::new(left),
                        op: binary_op,
                        right: Box::new(right),
                    },
                    ty: target_type.clone(),
                    span,
                }
            }
        };

        StmtKind::Assign { local, value }
    }

    fn check_print(&mut self, args: &[ast::PrintArg], newline: bool) -> StmtKind {
        let placeholders: Vec<_> = args
            .iter()
            .flat_map(|arg| match arg {
                ast::PrintArg::Template(parts) => parts
                    .iter()
                    .filter_map(|part| match part {
                        ast::TemplatePart::Placeholder { format, span } => Some((*format, *span)),
                        _ => None,
                    })
                    .collect(),
                ast::PrintArg::Expr(_) => Vec::new(),
            })
            .collect();
        let first_placeholder_arg = args.iter().position(|arg| {
            matches!(
                arg,
                ast::PrintArg::Template(parts)
                    if parts.iter().any(|part| matches!(part, ast::TemplatePart::Placeholder { .. }))
            )
        });
        let format_args: Vec<_> = args
            .iter()
            .enumerate()
            .filter_map(|(index, arg)| match arg {
                ast::PrintArg::Expr(expr)
                    if first_placeholder_arg.is_some_and(|first| index > first) =>
                {
                    Some(expr)
                }
                _ => None,
            })
            .collect();
        let has_placeholders = !placeholders.is_empty();

        if has_placeholders && format_args.len() < placeholders.len() {
            for (_, placeholder_span) in placeholders.iter().skip(format_args.len()) {
                self.diagnostics
                    .error(*placeholder_span, "missing formatting argument");
            }
        } else if has_placeholders && format_args.len() > placeholders.len() {
            self.diagnostics.error(
                format_args[placeholders.len()].span,
                "unused formatting argument",
            );
        }

        let mut items = Vec::new();
        let mut format_arg_index = 0;
        let mut top_level_index = 0;
        for (arg_index, arg) in args.iter().enumerate() {
            match arg {
                ast::PrintArg::Expr(expr)
                    if !has_placeholders
                        || first_placeholder_arg.is_some_and(|first| arg_index < first) =>
                {
                    let value = self.check_expr(expr, None);
                    self.check_printable(&value);
                    items.push(PrintItem::Value {
                        expr: value,
                        format: ast::FormatSpec::default(),
                        separator_before: top_level_index > 0,
                    });
                    top_level_index += 1;
                }
                ast::PrintArg::Expr(_) => {}
                ast::PrintArg::Template(parts) => {
                    let first_part = items.len();
                    for part in parts {
                        match part {
                            ast::TemplatePart::Text(value) => items.push(PrintItem::Text {
                                value: value.clone(),
                                separator_before: false,
                            }),
                            ast::TemplatePart::Interpolation(expr) => {
                                let value = self.check_expr(expr, None);
                                self.check_printable(&value);
                                items.push(PrintItem::Value {
                                    expr: value,
                                    format: ast::FormatSpec::default(),
                                    separator_before: false,
                                });
                            }
                            ast::TemplatePart::Placeholder { format, span } => {
                                if let Some(expr) = format_args.get(format_arg_index) {
                                    let value = self.check_expr(expr, None);
                                    self.check_printable(&value);
                                    self.check_format_spec(&value, *format, *span);
                                    items.push(PrintItem::Value {
                                        expr: value,
                                        format: *format,
                                        separator_before: false,
                                    });
                                }
                                format_arg_index += 1;
                            }
                        }
                    }
                    if items.len() == first_part {
                        items.push(PrintItem::Text {
                            value: String::new(),
                            separator_before: false,
                        });
                    }
                    if top_level_index > 0 {
                        match &mut items[first_part] {
                            PrintItem::Text {
                                separator_before, ..
                            }
                            | PrintItem::Value {
                                separator_before, ..
                            } => *separator_before = true,
                        }
                    }
                    top_level_index += 1;
                }
            }
        }
        StmtKind::Print { items, newline }
    }

    fn check_format_spec(&mut self, value: &Expr, format: ast::FormatSpec, span: Span) {
        if value.ty.is_error() {
            return;
        }
        if format.radix.is_some() && !value.ty.is_integer() {
            self.diagnostics.error(
                span,
                format!(
                    "radix formatting requires an integer, found `{}`",
                    self.type_name(&value.ty)
                ),
            );
        }
        if format.precision.is_some() && !value.ty.is_float() {
            self.diagnostics.error(
                span,
                format!(
                    "precision formatting requires a float, found `{}`",
                    self.type_name(&value.ty)
                ),
            );
        }
        if format.zero_pad && !value.ty.is_numeric() {
            self.diagnostics
                .error(span, "zero-padding requires a numeric value");
        }
    }

    fn check_printable(&mut self, value: &Expr) {
        let ok = matches!(
            value.ty,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::Isize
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::Usize
                | Type::F32
                | Type::F64
                | Type::Bool
                | Type::Char
                | Type::String
        ) || value.ty.is_error();
        if !ok {
            let type_name = self.type_name(&value.ty);
            let note = if matches!(value.ty, Type::I128 | Type::U128) {
                "printing 128-bit integers is not supported yet"
            } else if matches!(value.ty, Type::Struct(_)) {
                "printing structs is not supported yet; print their fields instead"
            } else if matches!(value.ty, Type::Enum(_)) {
                "printing enums is not supported yet; match on the value and print each case"
            } else {
                "supported print types are integers (up to 64 bits), floats, bool, char, and String"
            };
            self.diagnostics.error_with_note(
                value.span,
                format!("cannot print a value of type `{type_name}`"),
                note,
            );
        }
    }

    fn check_return(&mut self, span: Span, value: Option<&ast::Expr>) -> StmtKind {
        let value = match value {
            Some(expr) => {
                if self.current_return_type.is_unit() {
                    self.diagnostics.error_with_note(
                        expr.span,
                        "function returns `()` but a value was returned",
                        "use `return;` or give the function a return type",
                    );
                    Some(self.check_expr(expr, None))
                } else if self.current_return_type.is_error() {
                    Some(self.check_expr(expr, None))
                } else {
                    let expected = self.current_return_type.clone();
                    Some(self.check_expr(expr, Some(&expected)))
                }
            }
            None => {
                if !self.current_return_type.is_unit() && !self.current_return_type.is_error() {
                    let type_name = self.type_name(&self.current_return_type);
                    self.diagnostics.error(
                        span,
                        format!("expected a return value of type `{type_name}`"),
                    );
                }
                None
            }
        };
        StmtKind::Return(value)
    }

    fn check_match(
        &mut self,
        span: Span,
        scrutinee: &ast::Expr,
        arms: &[ast::MatchArm],
    ) -> StmtKind {
        let scrutinee = self.check_expr(scrutinee, None);
        let enum_id = match scrutinee.ty {
            Type::Enum(id) => Some(id),
            Type::Error => None,
            _ => {
                let type_name = self.type_name(&scrutinee.ty);
                self.diagnostics.error_with_note(
                    scrutinee.span,
                    format!("`match` currently supports enum values only, found `{type_name}`"),
                    "integer and string patterns are not implemented yet",
                );
                None
            }
        };

        let mut wildcard_seen = false;
        let mut wildcard_span = None;
        let mut covered: HashSet<usize> = HashSet::new();
        let mut covered_spans: HashMap<usize, Span> = HashMap::new();
        let mut hir_arms = Vec::new();

        for arm in arms {
            let pattern = match &arm.pattern {
                ast::Pattern::Wildcard { span: pattern_span } => {
                    if wildcard_seen {
                        self.diagnostics.error_with_note(
                            *pattern_span,
                            "multiple `_` arms in one match",
                            "the first `_` arm already covers every remaining value",
                        );
                    } else {
                        wildcard_seen = true;
                        wildcard_span = Some(*pattern_span);
                    }
                    MatchPattern::Wildcard
                }
                ast::Pattern::EnumVariant {
                    path,
                    span: pattern_span,
                } => {
                    let resolved =
                        enum_id.and_then(|id| self.resolve_match_variant(id, path, *pattern_span));
                    match resolved {
                        Some(variant) => {
                            if !covered.insert(variant) {
                                self.diagnostics.error_with_note(
                                    *pattern_span,
                                    "this enum variant is already covered by an earlier arm",
                                    "remove the duplicate arm",
                                );
                            } else {
                                covered_spans.insert(variant, *pattern_span);
                            }
                            match enum_id {
                                Some(enum_id) => MatchPattern::Variant { enum_id, variant },
                                None => MatchPattern::Wildcard,
                            }
                        }
                        None => MatchPattern::Wildcard,
                    }
                }
            };

            let body = self.check_block(&arm.body);
            hir_arms.push(MatchArm {
                pattern,
                body,
                span: arm.span,
            });
        }

        if let Some(id) = enum_id {
            if !wildcard_seen {
                if let Some(info) = self.enums.get(id.0) {
                    let missing: Vec<String> = info
                        .variants
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !covered.contains(index))
                        .map(|(_, variant)| format!("{}::{}", info.name, variant.name))
                        .collect();
                    if !missing.is_empty() {
                        self.diagnostics.error_with_note(
                            span,
                            format!("non-exhaustive match: missing {}", missing.join(", ")),
                            "add the missing variants or a `_` arm",
                        );
                    }
                }
            }
        }
        let _ = (wildcard_span, covered_spans);

        StmtKind::Match {
            scrutinee,
            arms: hir_arms,
        }
    }

    fn resolve_match_variant(
        &mut self,
        enum_id: EnumId,
        path: &[String],
        span: Span,
    ) -> Option<usize> {
        let info = self.enums.get(enum_id.0)?.clone();
        if path.len() != 2 {
            self.diagnostics.error_with_note(
                span,
                "expected an enum variant pattern of the form `Enum::Variant`",
                "fieldless enum variants are matched by name",
            );
            return None;
        }
        if path[0] != info.name {
            self.diagnostics.error_with_note(
                span,
                format!(
                    "pattern `{}::{}` does not match enum `{}`",
                    path[0], path[1], info.name
                ),
                "all patterns must belong to the scrutinee's enum type",
            );
            return None;
        }
        match info
            .variants
            .iter()
            .position(|variant| variant.name == path[1])
        {
            Some(index) => Some(index),
            None => {
                self.diagnostics.error_with_note(
                    span,
                    format!("enum `{}` has no variant `{}`", info.name, path[1]),
                    "check the spelling against the enum declaration",
                );
                None
            }
        }
    }

    fn check_expr(&mut self, expr: &ast::Expr, expected: Option<&Type>) -> Expr {
        let mut hir = self.check_expr_inner(expr, expected);
        if let Some(expected) = expected {
            if !expected.is_error() && !hir.ty.is_error() && hir.ty != *expected {
                let expected_name = self.type_name(expected);
                let actual_name = self.type_name(&hir.ty);
                self.diagnostics.error(
                    expr.span,
                    format!("type mismatch: expected `{expected_name}`, found `{actual_name}`"),
                );
            }
        }
        hir = Expr { ty: hir.ty, ..hir };
        hir
    }

    fn check_expr_inner(&mut self, expr: &ast::Expr, expected: Option<&Type>) -> Expr {
        match &expr.kind {
            ast::ExprKind::Int { value, suffix } => {
                let ty = self.int_literal_type(*value, suffix.as_deref(), expected, expr.span);
                Expr {
                    kind: ExprKind::Int(*value),
                    ty,
                    span: expr.span,
                }
            }
            ast::ExprKind::Float { value, suffix } => {
                let ty = self.float_literal_type(suffix.as_deref(), expected, expr.span);
                Expr {
                    kind: ExprKind::Float(*value),
                    ty,
                    span: expr.span,
                }
            }
            ast::ExprKind::Bool(value) => Expr {
                kind: ExprKind::Bool(*value),
                ty: Type::Bool,
                span: expr.span,
            },
            ast::ExprKind::Char(value) => Expr {
                kind: ExprKind::Char(*value),
                ty: Type::Char,
                span: expr.span,
            },
            ast::ExprKind::Str(value) => Expr {
                kind: ExprKind::Str(value.clone()),
                ty: Type::String,
                span: expr.span,
            },
            ast::ExprKind::Unit => Expr {
                kind: ExprKind::Unit,
                ty: Type::Unit,
                span: expr.span,
            },
            ast::ExprKind::Var(name) => match self.symbols.lookup(name) {
                Some(info) => Expr {
                    kind: ExprKind::Local(info.id),
                    ty: info.ty.clone(),
                    span: expr.span,
                },
                None => {
                    self.diagnostics
                        .error(expr.span, format!("undefined variable `{name}`"));
                    Expr {
                        kind: ExprKind::Local(LocalId(u32::MAX)),
                        ty: Type::Error,
                        span: expr.span,
                    }
                }
            },
            ast::ExprKind::Path(path) => self.check_path_expr(path, expr.span),
            ast::ExprKind::Unary { op, expr: operand } => {
                self.check_unary(*op, operand, expected, expr.span)
            }
            ast::ExprKind::Binary { left, op, right } => {
                self.check_binary(left, *op, right, expected, expr.span)
            }
            ast::ExprKind::Call { callee, args } => self.check_call(callee, args, expr.span),
            ast::ExprKind::Field { base, name } => self.check_field(base, name, expr.span),
            ast::ExprKind::StructLiteral { name, fields } => {
                self.check_struct_literal(name, fields, expr.span)
            }
            ast::ExprKind::Index { base, index } => {
                let _ = self.check_expr(base, None);
                let _ = self.check_expr(index, None);
                self.diagnostics.error_with_note(
                    expr.span,
                    "indexing is not implemented yet",
                    "arrays and vectors are not supported in this milestone",
                );
                Expr {
                    kind: ExprKind::Unit,
                    ty: Type::Error,
                    span: expr.span,
                }
            }
        }
    }

    fn check_unary(
        &mut self,
        op: UnaryOp,
        operand: &ast::Expr,
        expected: Option<&Type>,
        span: Span,
    ) -> Expr {
        match op {
            UnaryOp::Neg => {
                if let ast::ExprKind::Int { value, suffix } = &operand.kind {
                    let ty = self.negative_int_literal_type(
                        *value,
                        suffix.as_deref(),
                        expected,
                        operand.span,
                    );
                    let inner = Expr {
                        kind: ExprKind::Int(*value),
                        ty: ty.clone(),
                        span: operand.span,
                    };
                    return Expr {
                        kind: ExprKind::Unary {
                            op,
                            expr: Box::new(inner),
                        },
                        ty,
                        span,
                    };
                }
                let operand_expected = expected.filter(|ty| ty.is_numeric());
                let operand = self.check_expr(operand, operand_expected);
                let ty = operand.ty.clone();
                if !ty.is_numeric() && !ty.is_error() {
                    let type_name = self.type_name(&ty);
                    self.diagnostics.error(
                        operand.span,
                        format!("unary `-` requires a numeric operand, found `{type_name}`"),
                    );
                }
                if ty.is_unsigned_integer() {
                    let type_name = self.type_name(&ty);
                    self.diagnostics.error_with_note(
                        span,
                        format!("cannot negate a value of type `{type_name}`"),
                        "unsigned integers do not have a negative representation",
                    );
                }
                Expr {
                    kind: ExprKind::Unary {
                        op,
                        expr: Box::new(operand),
                    },
                    ty,
                    span,
                }
            }
            UnaryOp::Not => {
                let operand = self.check_expr(operand, Some(&Type::Bool));
                Expr {
                    kind: ExprKind::Unary {
                        op,
                        expr: Box::new(operand),
                    },
                    ty: Type::Bool,
                    span,
                }
            }
        }
    }

    fn check_binary(
        &mut self,
        left: &ast::Expr,
        op: BinaryOp,
        right: &ast::Expr,
        expected: Option<&Type>,
        span: Span,
    ) -> Expr {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let left = self.check_expr(left, Some(&Type::Bool));
            let right = self.check_expr(right, Some(&Type::Bool));
            return Expr {
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                ty: Type::Bool,
                span,
            };
        }

        let arithmetic = matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        );
        let comparison = matches!(
            op,
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
        );
        let ordering = matches!(
            op,
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
        );
        let bitwise = matches!(
            op,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr
        );

        let operand_expected = if arithmetic {
            expected.filter(|ty| ty.is_numeric() || ty.is_error())
        } else {
            None
        };
        let left_hir = self.check_expr(left, operand_expected);
        let left_type = left_hir.ty.clone();
        let right_expected = if left_type.is_error() {
            None
        } else {
            Some(&left_type)
        };
        let right_hir = self.check_expr(right, right_expected);
        let right_type = right_hir.ty.clone();

        if !left_type.is_error() && !right_type.is_error() && left_type != right_type {
            let left_name = self.type_name(&left_type);
            let right_name = self.type_name(&right_type);
            self.diagnostics.error(
                span,
                format!(
                    "binary operator `{}` requires operands of the same type, found `{left_name}` and `{right_name}`",
                    binary_op_symbol(op)
                ),
            );
        }

        let type_name = self.type_name(&left_type);
        if arithmetic && !left_type.is_numeric() && !left_type.is_error() {
            let note = if matches!(left_type, Type::String) {
                "string concatenation is not implemented yet"
            } else {
                "arithmetic operators require integer or floating-point operands"
            };
            self.diagnostics.error_with_note(
                span,
                format!(
                    "binary operator `{}` requires numeric operands, found `{type_name}`",
                    binary_op_symbol(op)
                ),
                note,
            );
        }
        if ordering
            && !left_type.is_numeric()
            && !matches!(left_type, Type::Char)
            && !left_type.is_error()
        {
            self.diagnostics.error(
                span,
                format!(
                    "binary operator `{}` requires numeric or `char` operands, found `{type_name}`",
                    binary_op_symbol(op)
                ),
            );
        }
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
            if matches!(left_type, Type::String) {
                self.diagnostics.error_with_note(
                    span,
                    "string equality is not implemented yet",
                    "string literals currently compile to C-style pointers, so `==` would compare addresses",
                );
            } else if !left_type.is_scalar()
                && !left_type.is_error()
                && !matches!(left_type, Type::Unit)
            {
                self.diagnostics.error(
                    span,
                    format!(
                        "binary operator `{}` cannot compare values of type `{type_name}`",
                        binary_op_symbol(op)
                    ),
                );
            }
        }
        if bitwise && !left_type.is_integer() && !left_type.is_error() {
            self.diagnostics.error(
                span,
                format!(
                    "binary operator `{}` requires integer operands, found `{type_name}`",
                    binary_op_symbol(op)
                ),
            );
        }

        let result_type = if comparison { Type::Bool } else { left_type };

        Expr {
            kind: ExprKind::Binary {
                left: Box::new(left_hir),
                op,
                right: Box::new(right_hir),
            },
            ty: result_type,
            span,
        }
    }

    fn check_call(&mut self, callee: &str, args: &[ast::Expr], span: Span) -> Expr {
        let function_id = self.function_names.get(callee).copied();
        let Some(function_id) = function_id else {
            self.diagnostics
                .error(span, format!("undefined function `{callee}`"));
            let args = args.iter().map(|arg| self.check_expr(arg, None)).collect();
            return Expr {
                kind: ExprKind::Call {
                    function: FunctionId(usize::MAX),
                    args,
                },
                ty: Type::Error,
                span,
            };
        };

        let info = self.functions[function_id.0].clone();
        if args.len() != info.params.len() {
            self.diagnostics.error_with_note(
                span,
                format!(
                    "function `{}` takes {} argument{}, but {} {} supplied",
                    info.name,
                    info.params.len(),
                    if info.params.len() == 1 { "" } else { "s" },
                    args.len(),
                    if args.len() == 1 { "was" } else { "were" }
                ),
                "check the function signature",
            );
        }

        let args = args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                let expected = info.params.get(index).map(|param| &param.ty);
                self.check_expr(arg, expected)
            })
            .collect();

        Expr {
            kind: ExprKind::Call {
                function: function_id,
                args,
            },
            ty: info.return_type,
            span,
        }
    }

    fn check_field(&mut self, base: &ast::Expr, name: &str, span: Span) -> Expr {
        let base = self.check_expr(base, None);
        let base_type = base.ty.clone();
        match base_type {
            Type::Struct(struct_id) => {
                let field = self.structs.get(struct_id.0).and_then(|info| {
                    info.fields
                        .iter()
                        .position(|field| field.name == name)
                        .map(|index| (index, info.fields[index].ty.clone(), info.name.clone()))
                });
                match field {
                    Some((index, ty, _struct_name)) => Expr {
                        kind: ExprKind::Field {
                            base: Box::new(base),
                            field: index,
                        },
                        ty,
                        span,
                    },
                    None => {
                        let struct_name = self
                            .structs
                            .get(struct_id.0)
                            .map(|info| info.name.clone())
                            .unwrap_or_else(|| "<unknown>".to_string());
                        self.diagnostics.error_with_note(
                            span,
                            format!("struct `{struct_name}` has no field `{name}`"),
                            "check the spelling against the struct declaration",
                        );
                        Expr {
                            kind: ExprKind::Field {
                                base: Box::new(base),
                                field: 0,
                            },
                            ty: Type::Error,
                            span,
                        }
                    }
                }
            }
            Type::Error => Expr {
                kind: ExprKind::Field {
                    base: Box::new(base),
                    field: 0,
                },
                ty: Type::Error,
                span,
            },
            other => {
                let type_name = self.type_name(&other);
                self.diagnostics
                    .error(span, format!("type `{type_name}` has no fields"));
                Expr {
                    kind: ExprKind::Field {
                        base: Box::new(base),
                        field: 0,
                    },
                    ty: Type::Error,
                    span,
                }
            }
        }
    }

    fn check_struct_literal(&mut self, name: &str, fields: &[ast::FieldInit], span: Span) -> Expr {
        let struct_id = self.struct_names.get(name).copied();
        let Some(struct_id) = struct_id else {
            self.diagnostics
                .error(span, format!("undefined struct `{name}`"));
            for field in fields {
                let _ = self.check_expr(&field.value, None);
            }
            return Expr {
                kind: ExprKind::Unit,
                ty: Type::Error,
                span,
            };
        };

        let info = self.structs[struct_id.0].clone();
        let mut initialized: Vec<Option<Expr>> = (0..info.fields.len()).map(|_| None).collect();
        let mut seen: HashMap<String, Span> = HashMap::new();

        for init in fields {
            let field_index = info.fields.iter().position(|field| field.name == init.name);
            match field_index {
                Some(index) => {
                    if seen.contains_key(&init.name) {
                        self.diagnostics.error_with_note(
                            init.span,
                            format!("field `{}` is initialized more than once", init.name),
                            "remove the duplicate field initializer",
                        );
                    } else {
                        seen.insert(init.name.clone(), init.span);
                    }
                    let value = self.check_expr(&init.value, Some(&info.fields[index].ty));
                    initialized[index] = Some(value);
                }
                None => {
                    self.diagnostics.error_with_note(
                        init.span,
                        format!("struct `{}` has no field `{}`", info.name, init.name),
                        "check the field name against the struct declaration",
                    );
                    let _ = self.check_expr(&init.value, None);
                }
            }
        }

        for (index, field) in info.fields.iter().enumerate() {
            if initialized[index].is_none() {
                self.diagnostics.error_with_note(
                    span,
                    format!(
                        "missing field `{}` in initializer for struct `{}`",
                        field.name, info.name
                    ),
                    "every field must be initialized",
                );
            }
        }

        let fields = initialized
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| value.map(|value| (index, value)))
            .collect();

        Expr {
            kind: ExprKind::StructLiteral { struct_id, fields },
            ty: Type::Struct(struct_id),
            span,
        }
    }

    fn check_path_expr(&mut self, path: &[String], span: Span) -> Expr {
        if path.len() != 2 {
            self.diagnostics.error_with_note(
                span,
                "path expressions are only supported for enum variants",
                "use `Enum::Variant`; modules and imports are not implemented yet",
            );
            return Expr {
                kind: ExprKind::Unit,
                ty: Type::Error,
                span,
            };
        }
        let Some(enum_id) = self.enum_names.get(&path[0]).copied() else {
            self.diagnostics
                .error(span, format!("`{}` is not an enum", path[0]));
            return Expr {
                kind: ExprKind::Unit,
                ty: Type::Error,
                span,
            };
        };
        let info = self.enums[enum_id.0].clone();
        match info
            .variants
            .iter()
            .position(|variant| variant.name == path[1])
        {
            Some(variant) => Expr {
                kind: ExprKind::EnumVariant { enum_id, variant },
                ty: Type::Enum(enum_id),
                span,
            },
            None => {
                self.diagnostics.error_with_note(
                    span,
                    format!("enum `{}` has no variant `{}`", info.name, path[1]),
                    "check the spelling against the enum declaration",
                );
                Expr {
                    kind: ExprKind::Unit,
                    ty: Type::Error,
                    span,
                }
            }
        }
    }

    fn int_literal_type(
        &mut self,
        value: u128,
        suffix: Option<&str>,
        expected: Option<&Type>,
        span: Span,
    ) -> Type {
        let ty = if let Some(suffix) = suffix {
            match integer_type_from_suffix(suffix) {
                Some(ty) => ty,
                None => {
                    self.diagnostics
                        .error(span, format!("unknown integer suffix `{suffix}`"));
                    expected
                        .filter(|ty| ty.is_integer())
                        .cloned()
                        .unwrap_or(Type::I32)
                }
            }
        } else if let Some(expected) = expected.filter(|ty| ty.is_integer()) {
            expected.clone()
        } else {
            Type::I32
        };

        if ty.is_integer() && !integer_literal_fits(&ty, value) {
            let type_name = self.type_name(&ty);
            self.diagnostics.error(
                span,
                format!("integer literal `{value}` does not fit in type `{type_name}`"),
            );
        }
        ty
    }

    fn negative_int_literal_type(
        &mut self,
        value: u128,
        suffix: Option<&str>,
        expected: Option<&Type>,
        span: Span,
    ) -> Type {
        let ty = if let Some(suffix) = suffix {
            match integer_type_from_suffix(suffix) {
                Some(ty) => ty,
                None => {
                    self.diagnostics
                        .error(span, format!("unknown integer suffix `{suffix}`"));
                    expected
                        .filter(|ty| ty.is_integer())
                        .cloned()
                        .unwrap_or(Type::I32)
                }
            }
        } else if let Some(expected) = expected.filter(|ty| ty.is_integer()) {
            expected.clone()
        } else {
            Type::I32
        };

        if ty.is_unsigned_integer() {
            let type_name = self.type_name(&ty);
            self.diagnostics.error_with_note(
                span,
                format!("cannot negate a literal of unsigned type `{type_name}`"),
                "use a signed integer type such as `i32`",
            );
        } else if let Some(bits) = ty.bit_width() {
            let limit = 1u128 << (bits - 1);
            if value > limit {
                let type_name = self.type_name(&ty);
                self.diagnostics.error(
                    span,
                    format!(
                        "negative integer literal `-{value}` does not fit in type `{type_name}`"
                    ),
                );
            }
        }
        ty
    }

    fn float_literal_type(
        &mut self,
        suffix: Option<&str>,
        expected: Option<&Type>,
        span: Span,
    ) -> Type {
        match suffix {
            Some("f32") => Type::F32,
            Some("f64") => Type::F64,
            Some(other) => {
                self.diagnostics
                    .error(span, format!("unknown floating-point suffix `{other}`"));
                expected
                    .filter(|ty| ty.is_float())
                    .cloned()
                    .unwrap_or(Type::F64)
            }
            None => expected
                .filter(|ty| ty.is_float())
                .cloned()
                .unwrap_or(Type::F64),
        }
    }

    fn resolve_type(&mut self, ty: &ast::TypeExpr) -> Type {
        match &ty.kind {
            ast::TypeExprKind::Unit => Type::Unit,
            ast::TypeExprKind::Named(name) => self.resolve_named_type(name, ty.span),
            ast::TypeExprKind::Generic { name, .. } => {
                self.diagnostics.error_with_note(
                    ty.span,
                    format!("generic type `{name}<...>` is not supported yet"),
                    "vectors and generic containers are not implemented in this milestone",
                );
                Type::Error
            }
            ast::TypeExprKind::Array { .. } => {
                self.diagnostics.error_with_note(
                    ty.span,
                    "array types are not supported yet",
                    "fixed-size arrays are planned after the ownership model",
                );
                Type::Error
            }
            ast::TypeExprKind::Tuple(_) => {
                self.diagnostics.error_with_note(
                    ty.span,
                    "tuple types are not supported yet",
                    "only `()` and named struct types are currently supported",
                );
                Type::Error
            }
            ast::TypeExprKind::Ref { .. } => {
                self.diagnostics.error_with_note(
                    ty.span,
                    "reference types are not supported yet",
                    "`&T` and `&mut T` will arrive with the ownership model",
                );
                Type::Error
            }
        }
    }

    fn resolve_named_type(&mut self, name: &str, span: Span) -> Type {
        match name {
            "i8" => Type::I8,
            "i16" => Type::I16,
            "i32" => Type::I32,
            "i64" => Type::I64,
            "i128" => Type::I128,
            "isize" => Type::Isize,
            "u8" => Type::U8,
            "u16" => Type::U16,
            "u32" => Type::U32,
            "u64" => Type::U64,
            "u128" => Type::U128,
            "usize" => Type::Usize,
            "f32" => Type::F32,
            "f64" => Type::F64,
            "bool" => Type::Bool,
            "char" => Type::Char,
            "String" => Type::String,
            "str" => {
                self.diagnostics.error_with_note(
                    span,
                    "`str` is not supported yet",
                    "use `String` for now; borrowed string slices require references",
                );
                Type::Error
            }
            _ => {
                if let Some(id) = self.struct_names.get(name) {
                    Type::Struct(*id)
                } else if let Some(id) = self.enum_names.get(name) {
                    Type::Enum(*id)
                } else {
                    self.diagnostics
                        .error(span, format!("unknown type `{name}`"));
                    Type::Error
                }
            }
        }
    }

    fn type_name(&self, ty: &Type) -> String {
        match ty {
            Type::Struct(id) => self
                .structs
                .get(id.0)
                .map(|info| info.name.clone())
                .unwrap_or_else(|| "<unknown struct>".to_string()),
            Type::Enum(id) => self
                .enums
                .get(id.0)
                .map(|info| info.name.clone())
                .unwrap_or_else(|| "<unknown enum>".to_string()),
            _ => ty.to_string(),
        }
    }
}

fn is_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "bool"
            | "char"
            | "String"
            | "str"
    )
}

fn integer_type_from_suffix(suffix: &str) -> Option<Type> {
    Some(match suffix {
        "i8" => Type::I8,
        "i16" => Type::I16,
        "i32" => Type::I32,
        "i64" => Type::I64,
        "i128" => Type::I128,
        "isize" => Type::Isize,
        "u8" => Type::U8,
        "u16" => Type::U16,
        "u32" => Type::U32,
        "u64" => Type::U64,
        "u128" => Type::U128,
        "usize" => Type::Usize,
        _ => return None,
    })
}

fn integer_literal_fits(ty: &Type, value: u128) -> bool {
    match ty {
        Type::I8 => value <= i8::MAX as u128,
        Type::I16 => value <= i16::MAX as u128,
        Type::I32 => value <= i32::MAX as u128,
        Type::I64 | Type::Isize => value <= i64::MAX as u128,
        Type::I128 => value <= i128::MAX as u128,
        Type::U8 => value <= u8::MAX as u128,
        Type::U16 => value <= u16::MAX as u128,
        Type::U32 => value <= u32::MAX as u128,
        Type::U64 | Type::Usize => value <= u64::MAX as u128,
        Type::U128 => true,
        _ => true,
    }
}

fn binary_op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
    }
}

fn block_guarantees_return(block: &Block) -> bool {
    block
        .statements
        .last()
        .is_some_and(statement_guarantees_return)
}

fn statement_guarantees_return(statement: &Stmt) -> bool {
    match &statement.kind {
        StmtKind::Return(_) => true,
        StmtKind::If {
            then_block,
            else_branch,
            ..
        } => match else_branch {
            Some(ElseBranch::Block(block)) => {
                block_guarantees_return(then_block) && block_guarantees_return(block)
            }
            Some(ElseBranch::If(statement)) => {
                block_guarantees_return(then_block) && statement_guarantees_return(statement)
            }
            None => false,
        },
        StmtKind::Loop { body } => !block_contains_break(body),
        StmtKind::Block(block) => block_guarantees_return(block),
        StmtKind::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|arm| block_guarantees_return(&arm.body))
        }
        _ => false,
    }
}

fn block_contains_break(block: &Block) -> bool {
    block.statements.iter().any(statement_contains_break)
}

fn statement_contains_break(statement: &Stmt) -> bool {
    match &statement.kind {
        StmtKind::Break => true,
        StmtKind::If {
            then_block,
            else_branch,
            ..
        } => {
            block_contains_break(then_block)
                || match else_branch {
                    Some(ElseBranch::Block(block)) => block_contains_break(block),
                    Some(ElseBranch::If(statement)) => statement_contains_break(statement),
                    None => false,
                }
        }
        StmtKind::Block(block) => block_contains_break(block),
        StmtKind::Match { arms, .. } => arms.iter().any(|arm| block_contains_break(&arm.body)),
        // A break inside a nested loop belongs to that loop.
        StmtKind::While { .. } | StmtKind::Loop { .. } | StmtKind::For { .. } => false,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn analyze_source(source: &str) -> AnalysisResult {
        let tokens = lex(source).tokens;
        let parsed = parse(tokens);
        analyze(parsed.program)
    }

    fn errors(result: &AnalysisResult) -> Vec<String> {
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_error())
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    #[test]
    fn checks_valid_program() {
        let result = analyze_source(
            r#"
            fn add(a: i32, b: i32) -> i32 { return a + b; }
            fn main() {
                let mut x: i32 = 1;
                x = add(x, 2);
                if x > 2 { print(x); } else { print("no"); }
                while x < 10 { x = x + 1; }
                for (let mut i: i32 = 0; i < 3; i = i + 1) { print(i); }
            }
        "#,
        );
        assert!(errors(&result).is_empty(), "{:?}", errors(&result));
        assert_eq!(result.program.functions.len(), 2);
    }

    #[test]
    fn rejects_immutable_assignment_and_undefined_variable() {
        let result = analyze_source(
            r#"
            fn main() {
                let x = 1;
                x = 2;
                print(y);
            }
        "#,
        );
        let messages = errors(&result);
        assert!(messages.iter().any(|message| message.contains("immutable")));
        assert!(
            messages
                .iter()
                .any(|message| message.contains("undefined variable"))
        );
    }

    #[test]
    fn rejects_type_mismatch_and_bad_arguments() {
        let result = analyze_source(
            r#"
            fn add(a: i32, b: i32) -> i32 { return a + b; }
            fn main() {
                let x: i32 = true;
                let y = add(1);
                let z: bool = add(1, 2);
            }
        "#,
        );
        let messages = errors(&result);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("type mismatch"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("takes 2 arguments"))
        );
    }

    #[test]
    fn checks_structs_and_enums() {
        let result = analyze_source(
            r#"
            struct User { name: String, age: u32 }
            enum Status { Active, Inactive }
            fn main() {
                let user = User { name: "Kern", age: 1 };
                print(user.age);
                let status = Status::Active;
                match status {
                    Status::Active => print("active"),
                    Status::Inactive => print("inactive"),
                }
            }
        "#,
        );
        assert!(errors(&result).is_empty(), "{:?}", errors(&result));
    }

    #[test]
    fn rejects_missing_return_path() {
        let result = analyze_source(
            r#"
            fn choose(x: bool) -> i32 {
                if x { return 1; }
            }
        "#,
        );
        assert!(
            errors(&result)
                .iter()
                .any(|message| message.contains("may reach the end"))
        );
    }

    #[test]
    fn rejects_non_exhaustive_match() {
        let result = analyze_source(
            r#"
            enum Status { Active, Inactive }
            fn main() {
                let status = Status::Active;
                match status { Status::Active => print("a"), }
            }
        "#,
        );
        assert!(
            errors(&result)
                .iter()
                .any(|message| message.contains("non-exhaustive"))
        );
    }
}
