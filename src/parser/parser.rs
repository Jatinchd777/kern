use crate::diagnostics::{Diagnostic, Diagnostics, Span};
use crate::lexer::{Token, TokenKind};

use super::ast::*;

pub struct ParseResult {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(tokens: Vec<Token>) -> ParseResult {
    Parser::new(tokens).parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Diagnostics,
    allow_struct_literal: bool,
}

impl Parser {
    fn new(mut tokens: Vec<Token>) -> Self {
        if tokens.is_empty() {
            tokens.push(Token::new(TokenKind::Eof, Span::default()));
        }
        Self {
            tokens,
            pos: 0,
            diagnostics: Diagnostics::new(),
            allow_struct_literal: true,
        }
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .or_else(|| self.tokens.last())
            .expect("token stream is never empty")
    }

    fn current_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn span(&self) -> Span {
        self.current().span
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.current_kind().same_variant(kind)
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, context: &str) -> Option<Token> {
        if self.at(kind) {
            Some(self.advance())
        } else {
            self.diagnostics.error(
                self.span(),
                format!(
                    "expected {}, found {} while parsing {}",
                    kind.describe(),
                    self.current_kind().describe(),
                    context
                ),
            );
            None
        }
    }

    fn expect_identifier(&mut self, context: &str) -> Option<(String, Span)> {
        match self.current().clone() {
            Token {
                kind: TokenKind::Ident(name),
                span,
            } => {
                self.advance();
                Some((name, span))
            }
            token => {
                self.diagnostics.error(
                    token.span,
                    format!(
                        "expected identifier, found {} while parsing {}",
                        token.kind.describe(),
                        context
                    ),
                );
                None
            }
        }
    }

    fn synchronize_item(&mut self) {
        while !matches!(self.current_kind(), TokenKind::Eof) {
            if matches!(
                self.current_kind(),
                TokenKind::Fn | TokenKind::Struct | TokenKind::Enum | TokenKind::Use
            ) {
                return;
            }
            self.advance();
        }
    }

    fn synchronize_statement(&mut self) {
        while !matches!(self.current_kind(), TokenKind::Eof | TokenKind::RBrace) {
            if self.eat(&TokenKind::Semi) {
                return;
            }
            self.advance();
        }
    }

    fn parse_program(mut self) -> ParseResult {
        let start = self.span();
        let mut items = Vec::new();

        while !matches!(self.current_kind(), TokenKind::Eof) {
            let before = self.pos;
            match self.parse_item() {
                Some(item) => items.push(item),
                None => {
                    if self.pos == before {
                        self.advance();
                    }
                    self.synchronize_item();
                }
            }
        }

        let end = self.span();
        let program = Program {
            items,
            span: start.join(end),
        };
        ParseResult {
            program,
            diagnostics: self.diagnostics.into_vec(),
        }
    }

    fn parse_item(&mut self) -> Option<Item> {
        match self.current_kind() {
            TokenKind::Fn => self.parse_function().map(Item::Function),
            TokenKind::Struct => self.parse_struct().map(Item::Struct),
            TokenKind::Enum => self.parse_enum().map(Item::Enum),
            TokenKind::Use => self.parse_use().map(Item::Use),
            TokenKind::Match => {
                self.diagnostics.error(
                    self.span(),
                    "`match` is only supported as a statement inside a function",
                );
                None
            }
            other => {
                self.diagnostics.error(
                    self.span(),
                    format!(
                        "expected a top-level item (`fn`, `struct`, `enum`, or `use`), found {}",
                        other.describe()
                    ),
                );
                None
            }
        }
    }

    fn parse_function(&mut self) -> Option<Function> {
        let start = self.expect(&TokenKind::Fn, "function declaration")?.span;
        let (name, _) = self.expect_identifier("function name")?;
        self.expect(&TokenKind::LParen, "function parameters")?;

        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !matches!(self.current_kind(), TokenKind::Eof) {
            let param_start = self.span();
            let Some((param_name, param_name_span)) = self.expect_identifier("function parameter")
            else {
                self.skip_to_parameter_boundary();
                continue;
            };
            if self.expect(&TokenKind::Colon, "parameter type").is_none() {
                self.skip_to_parameter_boundary();
                continue;
            }
            let Some(ty) = self.parse_type() else {
                self.skip_to_parameter_boundary();
                continue;
            };
            let span = param_name_span.join(ty.span).join(param_start);
            params.push(Param {
                name: param_name,
                ty,
                span,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "function parameters")?;

        let return_type = if self.eat(&TokenKind::Arrow) {
            self.parse_type()
        } else {
            None
        };

        let body = self.parse_block()?;
        let span = start.join(body.span);
        Some(Function {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    fn skip_to_parameter_boundary(&mut self) {
        while !matches!(
            self.current_kind(),
            TokenKind::Comma | TokenKind::RParen | TokenKind::Eof
        ) {
            self.advance();
        }
        self.eat(&TokenKind::Comma);
    }

    fn parse_struct(&mut self) -> Option<StructDecl> {
        let start = self.expect(&TokenKind::Struct, "struct declaration")?.span;
        let (name, _) = self.expect_identifier("struct name")?;
        self.expect(&TokenKind::LBrace, "struct fields")?;

        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !matches!(self.current_kind(), TokenKind::Eof) {
            let field_start = self.span();
            let Some((field_name, field_name_span)) = self.expect_identifier("struct field") else {
                self.synchronize_field();
                continue;
            };
            if self.expect(&TokenKind::Colon, "field type").is_none() {
                self.synchronize_field();
                continue;
            }
            let Some(ty) = self.parse_type() else {
                self.synchronize_field();
                continue;
            };
            let span = field_name_span.join(ty.span).join(field_start);
            fields.push(FieldDecl {
                name: field_name,
                ty,
                span,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(&TokenKind::RBrace, "struct fields")?.span;
        Some(StructDecl {
            name,
            fields,
            span: start.join(end),
        })
    }

    fn synchronize_field(&mut self) {
        while !matches!(
            self.current_kind(),
            TokenKind::Comma | TokenKind::RBrace | TokenKind::Eof
        ) {
            self.advance();
        }
        self.eat(&TokenKind::Comma);
    }

    fn parse_enum(&mut self) -> Option<EnumDecl> {
        let start = self.expect(&TokenKind::Enum, "enum declaration")?.span;
        let (name, _) = self.expect_identifier("enum name")?;
        self.expect(&TokenKind::LBrace, "enum variants")?;

        let mut variants = Vec::new();
        while !self.at(&TokenKind::RBrace) && !matches!(self.current_kind(), TokenKind::Eof) {
            let variant_start = self.span();
            let Some((variant_name, variant_name_span)) = self.expect_identifier("enum variant")
            else {
                self.synchronize_variant();
                continue;
            };
            let mut payload = Vec::new();
            let mut end = variant_name_span;
            if self.eat(&TokenKind::LParen) {
                while !self.at(&TokenKind::RParen) && !matches!(self.current_kind(), TokenKind::Eof)
                {
                    let Some(ty) = self.parse_type() else {
                        self.synchronize_variant();
                        break;
                    };
                    end = end.join(ty.span);
                    payload.push(ty);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                if let Some(close) = self.expect(&TokenKind::RParen, "enum variant payload") {
                    end = end.join(close.span);
                }
            }
            variants.push(EnumVariantDecl {
                name: variant_name,
                payload,
                span: variant_start.join(end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(&TokenKind::RBrace, "enum variants")?.span;
        Some(EnumDecl {
            name,
            variants,
            span: start.join(end),
        })
    }

    fn synchronize_variant(&mut self) {
        while !matches!(
            self.current_kind(),
            TokenKind::Comma | TokenKind::RBrace | TokenKind::Eof
        ) {
            self.advance();
        }
        self.eat(&TokenKind::Comma);
    }

    fn parse_use(&mut self) -> Option<UseDecl> {
        let start = self.expect(&TokenKind::Use, "import")?.span;
        let mut path = Vec::new();
        let Some((first, _)) = self.expect_identifier("import path") else {
            return None;
        };
        path.push(first);
        while self.eat(&TokenKind::ColonColon) {
            let Some((segment, _)) = self.expect_identifier("import path") else {
                break;
            };
            path.push(segment);
        }
        if self.at(&TokenKind::LBrace) || self.at(&TokenKind::Star) {
            self.diagnostics.error(
                self.span(),
                "grouped and glob imports are not supported yet",
            );
        }
        let end = self.expect(&TokenKind::Semi, "import")?.span;
        Some(UseDecl {
            path,
            span: start.join(end),
        })
    }

    fn parse_block(&mut self) -> Option<Block> {
        let start = self.expect(&TokenKind::LBrace, "block")?.span;
        let mut statements = Vec::new();
        while !self.at(&TokenKind::RBrace) && !matches!(self.current_kind(), TokenKind::Eof) {
            let before = self.pos;
            match self.parse_statement() {
                Some(statement) => statements.push(statement),
                None => {
                    if self.pos == before {
                        self.advance();
                    }
                    self.synchronize_statement();
                }
            }
        }
        let end = self.expect(&TokenKind::RBrace, "block")?.span;
        Some(Block {
            statements,
            span: start.join(end),
        })
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        match self.current_kind() {
            TokenKind::Let => self.parse_let(true),
            TokenKind::Print => self.parse_print(true, false),
            TokenKind::Println => self.parse_print(true, true),
            TokenKind::Cin => self.parse_cin(true),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Loop => self.parse_loop(),
            TokenKind::For => self.parse_for(),
            TokenKind::Break => {
                let start = self.advance().span;
                let end = self
                    .expect(&TokenKind::Semi, "break statement")
                    .map(|token| token.span)
                    .unwrap_or(start);
                Some(Stmt {
                    kind: StmtKind::Break,
                    span: start.join(end),
                })
            }
            TokenKind::Continue => {
                let start = self.advance().span;
                let end = self
                    .expect(&TokenKind::Semi, "continue statement")
                    .map(|token| token.span)
                    .unwrap_or(start);
                Some(Stmt {
                    kind: StmtKind::Continue,
                    span: start.join(end),
                })
            }
            TokenKind::Return => self.parse_return(),
            TokenKind::Match => self.parse_match(),
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let span = block.span;
                Some(Stmt {
                    kind: StmtKind::Block(block),
                    span,
                })
            }
            _ => self.parse_expression_statement(true),
        }
    }

    fn parse_let(&mut self, require_semi: bool) -> Option<Stmt> {
        let start = self.expect(&TokenKind::Let, "variable declaration")?.span;
        let mutable = self.eat(&TokenKind::Mut);
        let (name, name_span) = self.expect_identifier("variable name")?;
        let ty = if self.eat(&TokenKind::Colon) {
            self.parse_type()
        } else {
            None
        };
        let value = if self.eat(&TokenKind::Eq) {
            self.parse_expression()
        } else {
            None
        };
        let mut end = value
            .as_ref()
            .map(|expr| expr.span)
            .or_else(|| ty.as_ref().map(|ty| ty.span))
            .unwrap_or(name_span);
        if require_semi {
            if let Some(token) = self.expect(&TokenKind::Semi, "variable declaration") {
                end = end.join(token.span);
            }
        }
        Some(Stmt {
            kind: StmtKind::Let {
                name,
                mutable,
                ty,
                value,
            },
            span: start.join(end),
        })
    }

    fn parse_print(&mut self, require_semi: bool, newline: bool) -> Option<Stmt> {
        let start = self.advance().span;
        self.expect(&TokenKind::LParen, "print arguments")?;
        let mut args = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                args.push(self.parse_print_arg()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        let mut end = self.expect(&TokenKind::RParen, "print arguments")?.span;
        if require_semi {
            if let Some(token) = self.expect(&TokenKind::Semi, "print statement") {
                end = end.join(token.span);
            }
        }
        Some(Stmt {
            kind: StmtKind::Print { args, newline },
            span: start.join(end),
        })
    }

    fn parse_print_arg(&mut self) -> Option<PrintArg> {
        let token = self.current().clone();
        if let TokenKind::Str { value, raw } = token.kind {
            self.advance();
            if raw.contains('{') || raw.contains('}') {
                return Some(PrintArg::Template(
                    self.parse_template(&raw, token.span.start + 1),
                ));
            }
            return Some(PrintArg::Expr(Expr {
                kind: ExprKind::Str(value),
                span: token.span,
            }));
        }
        self.parse_expression().map(PrintArg::Expr)
    }

    fn parse_template(&mut self, raw: &str, source_offset: usize) -> Vec<TemplatePart> {
        let mut parts = Vec::new();
        let mut text_start = 0;
        let mut cursor = 0;

        while cursor < raw.len() {
            if raw.as_bytes()[cursor] == b'{' && raw.as_bytes().get(cursor + 1) == Some(&b'{') {
                cursor += 2;
                continue;
            }
            if raw.as_bytes()[cursor] == b'}' && raw.as_bytes().get(cursor + 1) == Some(&b'}') {
                cursor += 2;
                continue;
            }
            if raw.as_bytes()[cursor] == b'}' {
                self.diagnostics.error(
                    Span::new(source_offset + cursor, source_offset + cursor + 1),
                    "unmatched `}` in format string; use `}}` for a literal brace",
                );
                cursor += 1;
                continue;
            }
            if raw.as_bytes()[cursor] != b'{' {
                cursor += raw[cursor..].chars().next().unwrap().len_utf8();
                continue;
            }

            if text_start < cursor {
                parts.push(TemplatePart::Text(
                    self.decode_template_text(&raw[text_start..cursor]),
                ));
            }

            let Some(close) = find_template_close(raw, cursor + 1) else {
                self.diagnostics.error(
                    Span::new(source_offset + cursor, source_offset + raw.len()),
                    "unclosed `{` in format string",
                );
                parts.push(TemplatePart::Text(
                    self.decode_template_text(&raw[cursor..]),
                ));
                return parts;
            };

            let content = &raw[cursor + 1..close];
            let span = Span::new(source_offset + cursor, source_offset + close + 1);
            if content.starts_with(':') {
                parts.push(TemplatePart::Placeholder {
                    format: self.parse_format_spec(&content[1..], span),
                    span,
                });
            } else if content.is_empty() {
                parts.push(TemplatePart::Placeholder {
                    format: FormatSpec::default(),
                    span,
                });
            } else if let Some(expr) = self.parse_interpolation(content, source_offset + cursor + 1)
            {
                parts.push(TemplatePart::Interpolation(expr));
            }
            cursor = close + 1;
            text_start = cursor;
        }

        if text_start < raw.len() {
            parts.push(TemplatePart::Text(
                self.decode_template_text(&raw[text_start..]),
            ));
        }
        parts
    }

    fn decode_template_text(&mut self, raw: &str) -> String {
        let wrapped = format!("\"{raw}\"");
        let lexed = crate::lexer::lex(&wrapped);
        match lexed.tokens.into_iter().next().map(|token| token.kind) {
            Some(TokenKind::Str { value, .. }) => value.replace("{{", "{").replace("}}", "}"),
            _ => raw.to_string(),
        }
    }

    fn parse_interpolation(&mut self, source: &str, source_offset: usize) -> Option<Expr> {
        let mut lexed = crate::lexer::lex(source);
        for token in &mut lexed.tokens {
            token.span.start += source_offset;
            token.span.end += source_offset;
        }
        for mut diagnostic in lexed.diagnostics {
            diagnostic.span.start += source_offset;
            diagnostic.span.end += source_offset;
            self.diagnostics.push(diagnostic);
        }

        let mut parser = Parser::new(lexed.tokens);
        let expression = parser.parse_expression();
        if expression.is_some() && !parser.at(&TokenKind::Eof) {
            parser.diagnostics.error(
                parser.span(),
                "expected the end of an interpolation expression",
            );
        }
        for diagnostic in parser.diagnostics.into_vec() {
            self.diagnostics.push(diagnostic);
        }
        expression
    }

    fn parse_format_spec(&mut self, spec: &str, span: Span) -> FormatSpec {
        let mut format = FormatSpec::default();
        match spec {
            "x" => format.radix = Some(Radix::HexLower),
            "X" => format.radix = Some(Radix::HexUpper),
            "b" => format.radix = Some(Radix::Binary),
            "o" => format.radix = Some(Radix::Octal),
            _ if spec.starts_with('.') => {
                if spec.len() > 1 {
                    match spec[1..].parse::<usize>() {
                        Ok(precision) => format.precision = Some(precision),
                        Err(_) => {
                            self.diagnostics
                                .error(span, "invalid floating-point precision specifier");
                        }
                    }
                } else {
                    self.diagnostics
                        .error(span, "invalid floating-point precision specifier");
                }
            }
            _ if spec.bytes().all(|byte| byte.is_ascii_digit()) && !spec.is_empty() => {
                format.zero_pad = spec.starts_with('0');
                let width = if format.zero_pad { &spec[1..] } else { spec };
                match if width.is_empty() {
                    Ok(0)
                } else {
                    width.parse::<usize>()
                } {
                    Ok(width) => format.width = Some(width),
                    Err(_) => self.diagnostics.error(span, "format width is too large"),
                }
            }
            _ => {
                self.diagnostics
                    .error(span, format!("unsupported format specifier `:{spec}`"));
                return format;
            }
        }
        format
    }

    fn parse_cin(&mut self, require_semi: bool) -> Option<Stmt> {
        let start = self.expect(&TokenKind::Cin, "cin statement")?.span;
        let mut targets = Vec::new();
        self.expect(&TokenKind::Shr, "`>>`")?;
        let (name, span) = self.expect_identifier("cin target")?;
        targets.push(Expr {
            kind: ExprKind::Var(name),
            span,
        });
        while self.eat(&TokenKind::Shr) {
            let (name, span) = self.expect_identifier("cin target")?;
            targets.push(Expr {
                kind: ExprKind::Var(name),
                span,
            });
        }
        let mut end = targets.last().map(|expr| expr.span).unwrap_or(start);
        if require_semi {
            if let Some(token) = self.expect(&TokenKind::Semi, "cin statement") {
                end = end.join(token.span);
            }
        }
        Some(Stmt {
            kind: StmtKind::Cin(targets),
            span: start.join(end),
        })
    }

    fn parse_if(&mut self) -> Option<Stmt> {
        let start = self.expect(&TokenKind::If, "if statement")?.span;
        let condition = self.parse_expression_no_struct()?;
        let then_block = self.parse_block()?;
        let mut end = then_block.span;

        let else_branch = if self.eat(&TokenKind::Else) {
            if self.at(&TokenKind::If) {
                let nested = self.parse_if()?;
                end = nested.span;
                Some(ElseBranch::If(Box::new(nested)))
            } else {
                let block = self.parse_block()?;
                end = block.span;
                Some(ElseBranch::Block(block))
            }
        } else {
            None
        };

        Some(Stmt {
            kind: StmtKind::If {
                condition,
                then_block,
                else_branch,
            },
            span: start.join(end),
        })
    }

    fn parse_while(&mut self) -> Option<Stmt> {
        let start = self.expect(&TokenKind::While, "while loop")?.span;
        let condition = self.parse_expression_no_struct()?;
        let body = self.parse_block()?;
        let span = start.join(body.span);
        Some(Stmt {
            kind: StmtKind::While { condition, body },
            span,
        })
    }

    fn parse_loop(&mut self) -> Option<Stmt> {
        let start = self.expect(&TokenKind::Loop, "loop")?.span;
        let body = self.parse_block()?;
        let span = start.join(body.span);
        Some(Stmt {
            kind: StmtKind::Loop { body },
            span,
        })
    }

    fn parse_for(&mut self) -> Option<Stmt> {
        let start = self.expect(&TokenKind::For, "for loop")?.span;
        self.expect(&TokenKind::LParen, "for loop header")?;

        let init = if self.at(&TokenKind::Let) {
            self.parse_let(false).map(Box::new)
        } else if !self.at(&TokenKind::Semi) {
            self.parse_simple_statement().map(Box::new)
        } else {
            None
        };
        self.expect(&TokenKind::Semi, "for loop initializer")?;

        let condition = if !self.at(&TokenKind::Semi) {
            self.parse_expression()
        } else {
            None
        };
        self.expect(&TokenKind::Semi, "for loop condition")?;

        let update = if !self.at(&TokenKind::RParen) {
            self.parse_simple_statement().map(Box::new)
        } else {
            None
        };
        self.expect(&TokenKind::RParen, "for loop update")?;

        let body = self.parse_block()?;
        let span = start.join(body.span);
        Some(Stmt {
            kind: StmtKind::For {
                init,
                condition,
                update,
                body,
            },
            span,
        })
    }

    fn parse_return(&mut self) -> Option<Stmt> {
        let start = self.expect(&TokenKind::Return, "return statement")?.span;
        let value = if self.at(&TokenKind::Semi) {
            None
        } else {
            self.parse_expression()
        };
        let end = self
            .expect(&TokenKind::Semi, "return statement")
            .map(|token| token.span)
            .unwrap_or(start);
        Some(Stmt {
            kind: StmtKind::Return(value),
            span: start.join(end),
        })
    }

    fn parse_match(&mut self) -> Option<Stmt> {
        let start = self.expect(&TokenKind::Match, "match statement")?.span;
        let scrutinee = self.parse_expression_no_struct()?;
        self.expect(&TokenKind::LBrace, "match arms")?;

        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) && !matches!(self.current_kind(), TokenKind::Eof) {
            let arm_start = self.span();
            let pattern = if let TokenKind::Ident(name) = self.current_kind() {
                if name == "_" {
                    let span = self.advance().span;
                    Pattern::Wildcard { span }
                } else {
                    self.parse_pattern_path()?
                }
            } else {
                self.diagnostics.error(
                    self.span(),
                    format!(
                        "expected an enum variant pattern or `_`, found {}",
                        self.current_kind().describe()
                    ),
                );
                self.synchronize_match_arm();
                continue;
            };

            if self.expect(&TokenKind::FatArrow, "match arm").is_none() {
                self.synchronize_match_arm();
                continue;
            }

            let body_is_block = self.at(&TokenKind::LBrace);
            let body = if body_is_block {
                self.parse_block()?
            } else if self.at(&TokenKind::Print) || self.at(&TokenKind::Println) {
                let newline = self.at(&TokenKind::Println);
                let statement = self.parse_print(false, newline)?;
                self.eat(&TokenKind::Semi);
                let span = statement.span;
                Block {
                    statements: vec![statement],
                    span,
                }
            } else {
                let expression = self.parse_expression()?;
                let span = expression.span;
                let statement = Stmt {
                    kind: StmtKind::Expr(expression),
                    span,
                };
                self.eat(&TokenKind::Semi);
                Block {
                    statements: vec![statement],
                    span,
                }
            };
            let span = arm_start.join(body.span);
            arms.push(MatchArm {
                pattern,
                body,
                span,
            });

            if self.eat(&TokenKind::Comma) {
                continue;
            }
            // Block arms do not require a comma when another arm follows.
            if body_is_block
                && (self.at(&TokenKind::RBrace)
                    || matches!(self.current_kind(), TokenKind::Ident(_)))
            {
                continue;
            }
            if !self.at(&TokenKind::RBrace) {
                self.diagnostics
                    .error(self.span(), "expected `,` or `}` after match arm");
                self.synchronize_match_arm();
            }
        }

        let end = self.expect(&TokenKind::RBrace, "match arms")?.span;
        Some(Stmt {
            kind: StmtKind::Match { scrutinee, arms },
            span: start.join(end),
        })
    }

    fn parse_pattern_path(&mut self) -> Option<Pattern> {
        let start = self.span();
        let (first, _) = self.expect_identifier("match pattern")?;
        let mut path = vec![first];
        while self.eat(&TokenKind::ColonColon) {
            let Some((segment, _)) = self.expect_identifier("match pattern") else {
                break;
            };
            path.push(segment);
        }
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|token| token.span)
            .unwrap_or(start);
        Some(Pattern::EnumVariant {
            path,
            span: start.join(end),
        })
    }

    fn synchronize_match_arm(&mut self) {
        while !matches!(
            self.current_kind(),
            TokenKind::Comma | TokenKind::RBrace | TokenKind::Eof
        ) {
            self.advance();
        }
        self.eat(&TokenKind::Comma);
    }

    fn parse_expression_statement(&mut self, require_semi: bool) -> Option<Stmt> {
        let statement = self.parse_simple_statement()?;
        if require_semi {
            self.expect(&TokenKind::Semi, "statement")?;
        }
        Some(statement)
    }

    fn parse_simple_statement(&mut self) -> Option<Stmt> {
        let target = self.parse_expression()?;
        let op = match self.current_kind() {
            TokenKind::Eq => Some(AssignOp::Assign),
            TokenKind::PlusEq => Some(AssignOp::Add),
            TokenKind::MinusEq => Some(AssignOp::Sub),
            TokenKind::StarEq => Some(AssignOp::Mul),
            TokenKind::SlashEq => Some(AssignOp::Div),
            TokenKind::PercentEq => Some(AssignOp::Rem),
            _ => None,
        };

        if let Some(op) = op {
            self.advance();
            let value = self.parse_expression()?;
            let span = target.span.join(value.span);
            Some(Stmt {
                kind: StmtKind::Assign { target, op, value },
                span,
            })
        } else {
            let span = target.span;
            Some(Stmt {
                kind: StmtKind::Expr(target),
                span,
            })
        }
    }

    fn parse_type(&mut self) -> Option<TypeExpr> {
        let start = self.span();

        if self.eat(&TokenKind::LParen) {
            if let Some(close) = self.take(&TokenKind::RParen) {
                return Some(TypeExpr {
                    kind: TypeExprKind::Unit,
                    span: start.join(close.span),
                });
            }
            let mut elements = Vec::new();
            loop {
                elements.push(self.parse_type()?);
                if self.eat(&TokenKind::Comma) {
                    if self.at(&TokenKind::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }
            let close = self.expect(&TokenKind::RParen, "tuple type")?;
            return Some(TypeExpr {
                kind: TypeExprKind::Tuple(elements),
                span: start.join(close.span),
            });
        }

        if self.eat(&TokenKind::Amp) {
            let mutable = self.eat(&TokenKind::Mut);
            let inner = self.parse_type()?;
            let span = start.join(inner.span);
            return Some(TypeExpr {
                kind: TypeExprKind::Ref {
                    mutable,
                    inner: Box::new(inner),
                },
                span,
            });
        }

        if self.eat(&TokenKind::LBracket) {
            let element = self.parse_type()?;
            self.expect(&TokenKind::Semi, "array type length")?;
            let length = match self.current().clone() {
                Token {
                    kind:
                        TokenKind::Int {
                            value,
                            suffix: None,
                        },
                    span,
                } => {
                    self.advance();
                    match usize::try_from(value) {
                        Ok(value) => value,
                        Err(_) => {
                            self.diagnostics
                                .error(span, "array length does not fit in `usize`");
                            0
                        }
                    }
                }
                token => {
                    self.diagnostics
                        .error(token.span, "expected an integer literal for array length");
                    0
                }
            };
            let close = self.expect(&TokenKind::RBracket, "array type")?;
            return Some(TypeExpr {
                kind: TypeExprKind::Array {
                    element: Box::new(element),
                    len: length,
                },
                span: start.join(close.span),
            });
        }

        let (first, first_span) = self.expect_identifier("type")?;
        let mut name = first;
        let mut end = first_span;
        while self.eat(&TokenKind::ColonColon) {
            let Some((segment, segment_span)) = self.expect_identifier("type path") else {
                break;
            };
            name.push_str("::");
            name.push_str(&segment);
            end = segment_span;
        }

        if self.eat(&TokenKind::Lt) {
            let mut args = Vec::new();
            loop {
                args.push(self.parse_type()?);
                if self.eat(&TokenKind::Comma) {
                    if self.at(&TokenKind::Gt) {
                        break;
                    }
                    continue;
                }
                break;
            }
            let close = self.expect(&TokenKind::Gt, "generic type arguments")?;
            return Some(TypeExpr {
                kind: TypeExprKind::Generic { name, args },
                span: start.join(close.span),
            });
        }

        Some(TypeExpr {
            kind: TypeExprKind::Named(name),
            span: start.join(end),
        })
    }

    fn take(&mut self, kind: &TokenKind) -> Option<Token> {
        if self.at(kind) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn parse_expression(&mut self) -> Option<Expr> {
        self.parse_binary(0)
    }

    fn parse_expression_no_struct(&mut self) -> Option<Expr> {
        let saved = self.allow_struct_literal;
        self.allow_struct_literal = false;
        let result = self.parse_expression();
        self.allow_struct_literal = saved;
        result
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Option<Expr> {
        let mut left = self.parse_unary()?;

        loop {
            let Some((op, precedence)) = binary_operator(self.current_kind()) else {
                break;
            };
            if precedence < min_precedence {
                break;
            }
            self.advance();
            let right = self.parse_binary(precedence + 1)?;
            let span = left.span.join(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span,
            };
        }

        Some(left)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        let op = match self.current_kind() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            let start = self.advance().span;
            let expr = self.parse_unary()?;
            let span = start.join(expr.span);
            return Some(Expr {
                kind: ExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                },
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.eat(&TokenKind::Dot) {
                let Some((name, name_span)) = self.expect_identifier("field access") else {
                    break;
                };
                let span = expr.span.join(name_span);
                expr = Expr {
                    kind: ExprKind::Field {
                        base: Box::new(expr),
                        name,
                    },
                    span,
                };
            } else if self.eat(&TokenKind::LBracket) {
                let index = self.parse_expression()?;
                let close = self.expect(&TokenKind::RBracket, "index expression")?;
                let span = expr.span.join(close.span);
                expr = Expr {
                    kind: ExprKind::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    },
                    span,
                };
            } else {
                break;
            }
        }

        Some(expr)
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let token = self.current().clone();
        match token.kind {
            TokenKind::Int { value, suffix } => {
                self.advance();
                Some(Expr {
                    kind: ExprKind::Int { value, suffix },
                    span: token.span,
                })
            }
            TokenKind::Float { value, suffix } => {
                self.advance();
                Some(Expr {
                    kind: ExprKind::Float { value, suffix },
                    span: token.span,
                })
            }
            TokenKind::Str { value, .. } => {
                self.advance();
                Some(Expr {
                    kind: ExprKind::Str(value),
                    span: token.span,
                })
            }
            TokenKind::Char(value) => {
                self.advance();
                Some(Expr {
                    kind: ExprKind::Char(value),
                    span: token.span,
                })
            }
            TokenKind::True | TokenKind::False => {
                self.advance();
                Some(Expr {
                    kind: ExprKind::Bool(matches!(token.kind, TokenKind::True)),
                    span: token.span,
                })
            }
            TokenKind::LParen => {
                self.advance();
                if let Some(close) = self.take(&TokenKind::RParen) {
                    return Some(Expr {
                        kind: ExprKind::Unit,
                        span: token.span.join(close.span),
                    });
                }
                let saved = self.allow_struct_literal;
                self.allow_struct_literal = true;
                let expression = self.parse_expression();
                self.allow_struct_literal = saved;
                let expression = expression?;
                let close = self.expect(&TokenKind::RParen, "parenthesized expression")?;
                Some(Expr {
                    span: expression.span.join(close.span),
                    kind: expression.kind,
                })
            }
            TokenKind::Ident(name) => {
                self.advance();
                let mut path = vec![name];
                let mut end = token.span;
                while self.eat(&TokenKind::ColonColon) {
                    let Some((segment, segment_span)) = self.expect_identifier("path") else {
                        break;
                    };
                    path.push(segment);
                    end = segment_span;
                }

                if self.at(&TokenKind::LParen) {
                    if path.len() != 1 {
                        self.diagnostics.error(
                            self.span(),
                            "function calls through paths are not supported yet",
                        );
                    }
                    self.advance();
                    let mut args = Vec::new();
                    while !self.at(&TokenKind::RParen)
                        && !matches!(self.current_kind(), TokenKind::Eof)
                    {
                        args.push(self.parse_expression()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let close = self.expect(&TokenKind::RParen, "call arguments")?;
                    end = close.span;
                    let callee = path.first().cloned().unwrap_or_default();
                    return Some(Expr {
                        kind: ExprKind::Call { callee, args },
                        span: token.span.join(end),
                    });
                }

                if path.len() == 1 && self.allow_struct_literal && self.at(&TokenKind::LBrace) {
                    return self.parse_struct_literal(token.span, path.remove(0));
                }

                if path.len() == 1 {
                    Some(Expr {
                        kind: ExprKind::Var(path.remove(0)),
                        span: token.span.join(end),
                    })
                } else {
                    Some(Expr {
                        kind: ExprKind::Path(path),
                        span: token.span.join(end),
                    })
                }
            }
            TokenKind::Match => {
                self.diagnostics.error(
                    token.span,
                    "`match` expressions are not supported yet; use a match statement",
                );
                None
            }
            other => {
                self.diagnostics.error(
                    token.span,
                    format!("expected an expression, found {}", other.describe()),
                );
                None
            }
        }
    }

    fn parse_struct_literal(&mut self, start: Span, name: String) -> Option<Expr> {
        self.expect(&TokenKind::LBrace, "struct literal")?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !matches!(self.current_kind(), TokenKind::Eof) {
            let field_start = self.span();
            let Some((field_name, field_name_span)) =
                self.expect_identifier("struct literal field")
            else {
                self.synchronize_field();
                continue;
            };
            let value = if self.eat(&TokenKind::Colon) {
                self.parse_expression()?
            } else {
                Expr {
                    kind: ExprKind::Var(field_name.clone()),
                    span: field_name_span,
                }
            };
            let span = field_start.join(value.span);
            fields.push(FieldInit {
                name: field_name,
                value,
                span,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let close = self.expect(&TokenKind::RBrace, "struct literal")?;
        Some(Expr {
            kind: ExprKind::StructLiteral { name, fields },
            span: start.join(close.span),
        })
    }
}

fn binary_operator(kind: &TokenKind) -> Option<(BinaryOp, u8)> {
    let result = match kind {
        TokenKind::OrOr => (BinaryOp::Or, 1),
        TokenKind::AndAnd => (BinaryOp::And, 2),
        TokenKind::Pipe => (BinaryOp::BitOr, 3),
        TokenKind::Caret => (BinaryOp::BitXor, 4),
        TokenKind::Amp => (BinaryOp::BitAnd, 5),
        TokenKind::EqEq => (BinaryOp::Eq, 6),
        TokenKind::Ne => (BinaryOp::Ne, 6),
        TokenKind::Lt => (BinaryOp::Lt, 7),
        TokenKind::Le => (BinaryOp::Le, 7),
        TokenKind::Gt => (BinaryOp::Gt, 7),
        TokenKind::Ge => (BinaryOp::Ge, 7),
        TokenKind::Shl => (BinaryOp::Shl, 8),
        TokenKind::Shr => (BinaryOp::Shr, 8),
        TokenKind::Plus => (BinaryOp::Add, 9),
        TokenKind::Minus => (BinaryOp::Sub, 9),
        TokenKind::Star => (BinaryOp::Mul, 10),
        TokenKind::Slash => (BinaryOp::Div, 10),
        TokenKind::Percent => (BinaryOp::Rem, 10),
        _ => return None,
    };
    Some(result)
}

fn find_template_close(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut cursor = start;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if let Some(active_quote) = quote {
            if byte == active_quote {
                quote = None;
            }
        } else {
            match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'{' => depth += 1,
                b'}' if depth == 0 => return Some(cursor),
                b'}' => depth -= 1,
                _ => {}
            }
        }
        cursor += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_source(source: &str) -> ParseResult {
        parse(lex(source).tokens)
    }

    fn first_function(result: &ParseResult) -> &Function {
        match &result.program.items[0] {
            Item::Function(function) => function,
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parses_function_and_precedence() {
        let result = parse_source("fn main() { let x = 1 + 2 * 3; print(x); }");
        assert!(result.diagnostics.is_empty());
        let function = first_function(&result);
        let StmtKind::Let {
            value:
                Some(Expr {
                    kind: ExprKind::Binary { op, right, .. },
                    ..
                }),
            ..
        } = &function.body.statements[0].kind
        else {
            panic!("expected let");
        };
        assert_eq!(*op, BinaryOp::Add);
        assert!(matches!(
            right.kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }

    #[test]
    fn parses_control_flow_and_structs() {
        let source = r#"
            struct User { name: String, age: u32 }
            enum Status { Active, Inactive }
            fn main() {
                let mut i: i32 = 0;
                for (let mut j: i32 = 0; j < 10; j = j + 1) { i = i + j; }
                while i < 100 { i = i + 1; }
                if i > 10 { print(i); } else if i > 5 { print("mid"); } else { print("low"); }
                let user = User { name: "Kern", age: 1 };
                print(user.age);
            }
        "#;
        let result = parse_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.program.items.len(), 3);
    }

    #[test]
    fn parses_match_statement() {
        let source = r#"
            fn main() {
                let status = Status::Active;
                match status {
                    Status::Active => print("Active"),
                    Status::Inactive => { print("Inactive"); }
                    _ => print("Unknown"),
                }
            }
        "#;
        let result = parse_source(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    }

    #[test]
    fn reports_missing_semicolon_without_panicking() {
        let result = parse_source("fn main() { let x = 1 }");
        assert!(!result.diagnostics.is_empty());
    }
}
