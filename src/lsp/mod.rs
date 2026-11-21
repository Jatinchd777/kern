mod index;

use crate::compiler::{self, SourceAnalysis};
use crate::diagnostics::{Diagnostic as KernDiagnostic, Span};
use index::{IndexedSymbol, SymbolIndex};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionParams, CodeActionProviderCapability, CompletionItem,
    CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentSymbol, DocumentSymbolParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverProviderCapability, Location, MarkupContent, MarkupKind, Position, PositionEncodingKind,
    ReferenceParams, RenameOptions, RenameParams, ServerCapabilities, SymbolKind,
    TextDocumentContentChangeEvent, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextEdit, Uri, WorkspaceEdit,
};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

struct Document {
    text: String,
    version: i32,
    analysis: Option<SourceAnalysis>,
}

struct AnalysisReady {
    uri: Uri,
    version: i32,
    analysis: SourceAnalysis,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let (connection, io_threads) = Connection::stdio();
    let (initialize_id, _) = connection.initialize_start()?;
    connection.initialize_finish(
        initialize_id,
        serde_json::to_value(lsp_types::InitializeResult {
            capabilities: server_capabilities(),
            server_info: Some(lsp_types::ServerInfo {
                name: "kern-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })?,
    )?;
    log_debug("initialized on stdio");

    let mut documents: HashMap<Uri, Document> = HashMap::new();
    let (analysis_sender, analysis_receiver) = mpsc::channel();
    loop {
        match connection.receiver.recv_timeout(Duration::from_millis(40)) {
            Ok(Message::Request(request)) => {
                if request.method == "shutdown" {
                    connection
                        .sender
                        .send(Message::Response(Response::new_ok(request.id, Value::Null)))?;
                } else {
                    handle_request(&connection, request, &documents)?;
                }
            }
            Ok(Message::Notification(notification)) => {
                if notification.method == "exit" {
                    break;
                }
                handle_notification(&connection, notification, &mut documents, &analysis_sender)?;
            }
            Ok(Message::Response(_)) => {}
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
        publish_completed_analyses(&connection, &mut documents, &analysis_receiver)?;
    }

    drop(connection);
    io_threads.join()?;
    Ok(())
}

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF16),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                ..Default::default()
            },
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        references_provider: Some(lsp_types::OneOf::Left(true)),
        document_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        rename_provider: Some(lsp_types::OneOf::Right(RenameOptions {
            prepare_provider: None,
            work_done_progress_options: Default::default(),
        })),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        ..Default::default()
    }
}

fn handle_notification(
    connection: &Connection,
    notification: Notification,
    documents: &mut HashMap<Uri, Document>,
    analysis_sender: &Sender<AnalysisReady>,
) -> Result<(), Box<dyn Error>> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(notification.params)?;
            let uri = params.text_document.uri;
            let version = params.text_document.version;
            let text = params.text_document.text;
            log_debug(&format!("opened {} at version {version}", uri.as_str()));
            documents.insert(
                uri.clone(),
                Document {
                    text: text.clone(),
                    version,
                    analysis: None,
                },
            );
            schedule_analysis(uri, version, text, analysis_sender.clone());
        }
        "textDocument/didChange" => {
            let params: DidChangeTextDocumentParams = serde_json::from_value(notification.params)?;
            if let Some(document) = documents.get_mut(&params.text_document.uri) {
                if params.text_document.version <= document.version {
                    return Ok(());
                }
                apply_changes(&mut document.text, &params.content_changes);
                document.version = params.text_document.version;
                document.analysis = None;
                log_debug(&format!(
                    "changed {} to version {}",
                    params.text_document.uri.as_str(),
                    document.version
                ));
                schedule_analysis(
                    params.text_document.uri,
                    document.version,
                    document.text.clone(),
                    analysis_sender.clone(),
                );
            }
        }
        "textDocument/didClose" => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(notification.params)?;
            documents.remove(&params.text_document.uri);
            send_publish_diagnostics(connection, params.text_document.uri, None, Vec::new())?;
        }
        _ => {}
    }
    Ok(())
}

fn schedule_analysis(uri: Uri, version: i32, text: String, sender: Sender<AnalysisReady>) {
    thread::spawn(move || {
        let analysis = compiler::analyze_source(&text);
        let _ = sender.send(AnalysisReady {
            uri,
            version,
            analysis,
        });
    });
}

fn publish_completed_analyses(
    connection: &Connection,
    documents: &mut HashMap<Uri, Document>,
    receiver: &Receiver<AnalysisReady>,
) -> Result<(), Box<dyn Error>> {
    while let Ok(ready) = receiver.try_recv() {
        let Some(document) = documents.get_mut(&ready.uri) else {
            continue;
        };
        if document.version != ready.version {
            continue;
        }
        let diagnostics: Vec<lsp_types::Diagnostic> = ready
            .analysis
            .diagnostics
            .iter()
            .map(|diagnostic| to_lsp_diagnostic(diagnostic, &document.text))
            .collect();
        log_debug(&format!(
            "analyzed {} at version {}: {} diagnostic(s)",
            ready.uri.as_str(),
            ready.version,
            diagnostics.len()
        ));
        document.analysis = Some(ready.analysis);
        send_publish_diagnostics(connection, ready.uri, Some(document.version), diagnostics)?;
    }
    Ok(())
}

fn send_publish_diagnostics(
    connection: &Connection,
    uri: Uri,
    version: Option<i32>,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Result<(), Box<dyn Error>> {
    let params = lsp_types::PublishDiagnosticsParams {
        uri,
        diagnostics,
        version,
    };
    connection.sender.send(Message::Notification(Notification {
        method: "textDocument/publishDiagnostics".to_string(),
        params: serde_json::to_value(params)?,
    }))?;
    Ok(())
}

fn to_lsp_diagnostic(diagnostic: &KernDiagnostic, source: &str) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: span_to_range(source, diagnostic.span),
        severity: Some(match diagnostic.severity {
            crate::diagnostics::Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
            crate::diagnostics::Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
        }),
        code: Some(lsp_types::NumberOrString::String(
            "kern-frontend".to_string(),
        )),
        source: Some("kern".to_string()),
        message: diagnostic.message.clone(),
        related_information: None,
        tags: None,
        data: None,
        code_description: None,
    }
}

fn handle_request(
    connection: &Connection,
    request: Request,
    documents: &HashMap<Uri, Document>,
) -> Result<(), Box<dyn Error>> {
    log_debug(&format!("request {}", request.method));
    let id = request.id.clone();
    let result = match request.method.as_str() {
        "textDocument/completion" => {
            let params: CompletionParams = serde_json::from_value(request.params)?;
            document_query(
                documents,
                &params.text_document_position.text_document.uri,
                |document| completion_response(document, params.text_document_position.position),
            )
        }
        "textDocument/hover" => {
            let params: lsp_types::HoverParams = serde_json::from_value(request.params)?;
            document_query(
                documents,
                &params.text_document_position_params.text_document.uri,
                |document| hover_response(document, params.text_document_position_params.position),
            )
        }
        "textDocument/definition" => {
            let params: lsp_types::GotoDefinitionParams = serde_json::from_value(request.params)?;
            document_query(
                documents,
                &params.text_document_position_params.text_document.uri,
                |document| {
                    definition_response(
                        document,
                        &params.text_document_position_params.text_document.uri,
                        params.text_document_position_params.position,
                    )
                },
            )
        }
        "textDocument/references" => {
            let params: ReferenceParams = serde_json::from_value(request.params)?;
            document_query(
                documents,
                &params.text_document_position.text_document.uri,
                |document| {
                    references_response(
                        document,
                        &params.text_document_position.text_document.uri,
                        params.text_document_position.position,
                        params.context.include_declaration,
                    )
                },
            )
        }
        "textDocument/documentSymbol" => {
            let params: DocumentSymbolParams = serde_json::from_value(request.params)?;
            document_query(
                documents,
                &params.text_document.uri,
                document_symbols_response,
            )
        }
        "textDocument/rename" => {
            let params: RenameParams = serde_json::from_value(request.params)?;
            document_query(
                documents,
                &params.text_document_position.text_document.uri,
                |document| {
                    rename_response(
                        document,
                        &params.text_document_position.text_document.uri,
                        params.text_document_position.position,
                        &params.new_name,
                    )
                },
            )
        }
        "textDocument/codeAction" => {
            let params: CodeActionParams = serde_json::from_value(request.params)?;
            document_query(documents, &params.text_document.uri, |document| {
                code_actions_response(document, &params)
            })
        }
        _ => Some(Ok(Value::Null)),
    };

    match result {
        Some(Ok(value)) => connection
            .sender
            .send(Message::Response(Response::new_ok(id, value)))?,
        Some(Err(error)) => connection.sender.send(Message::Response(Response::new_err(
            id,
            lsp_server::ErrorCode::InvalidParams as i32,
            error,
        )))?,
        None => connection
            .sender
            .send(Message::Response(Response::new_ok(id, Value::Null)))?,
    }
    Ok(())
}

fn document_query(
    documents: &HashMap<Uri, Document>,
    uri: &Uri,
    query: impl FnOnce(&Document) -> Result<Value, String>,
) -> Option<Result<Value, String>> {
    documents.get(uri).map(query)
}

fn completion_response(document: &Document, position: Position) -> Result<Value, String> {
    let offset = position_to_offset(&document.text, position);
    if index::token_is_string_or_char(&document.text, offset) {
        return serde_json::to_value(CompletionResponse::Array(Vec::new()))
            .map_err(|error| error.to_string());
    }
    let analysis = get_analysis(document);
    let index = SymbolIndex::new(&document.text, &analysis.syntax);
    let prefix = document.text.get(..offset).unwrap_or_default();
    let items = if let Some(enum_name) = enum_completion_context(prefix) {
        index
            .enum_variants_for(&enum_name)
            .into_iter()
            .map(|symbol| completion_item(&index.symbols[symbol]))
            .collect()
    } else if let Some(receiver) = field_completion_context(prefix) {
        index
            .symbol_at(offset.saturating_sub(receiver.len() + 1))
            .and_then(|symbol| index.symbols[symbol].ty.as_ref())
            .and_then(|ty| {
                index
                    .global_symbols()
                    .find(|(_, symbol)| symbol.name == *ty && symbol.kind == SymbolKind::STRUCT)
                    .map(|(symbol, _)| symbol)
            })
            .map(|structure| {
                index
                    .children(structure)
                    .into_iter()
                    .filter(|(_, symbol)| symbol.kind == SymbolKind::FIELD)
                    .map(|(_, symbol)| completion_item(symbol))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        let mut items: Vec<_> = index
            .visible_symbols(offset)
            .into_iter()
            .map(|symbol| completion_item(&index.symbols[symbol]))
            .collect();
        items.extend(
            [
                "fn", "let", "mut", "if", "else", "while", "loop", "for", "break", "continue",
                "return", "match", "struct", "enum", "use", "print", "println", "cin", "bool",
                "char", "String", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32",
                "u64", "u128", "usize", "f32", "f64", "true", "false",
            ]
            .into_iter()
            .map(|keyword| CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            }),
        );
        items
    };
    serde_json::to_value(CompletionResponse::Array(items)).map_err(|error| error.to_string())
}

fn completion_item(symbol: &IndexedSymbol) -> CompletionItem {
    CompletionItem {
        label: symbol.name.clone(),
        kind: Some(match symbol.kind {
            SymbolKind::FUNCTION => CompletionItemKind::FUNCTION,
            SymbolKind::STRUCT => CompletionItemKind::STRUCT,
            SymbolKind::CLASS => CompletionItemKind::CLASS,
            SymbolKind::ENUM => CompletionItemKind::ENUM,
            SymbolKind::ENUM_MEMBER => CompletionItemKind::ENUM_MEMBER,
            SymbolKind::FIELD => CompletionItemKind::FIELD,
            _ => CompletionItemKind::VARIABLE,
        }),
        detail: Some(symbol.detail.clone()),
        ..Default::default()
    }
}

fn hover_response(document: &Document, position: Position) -> Result<Value, String> {
    let analysis = get_analysis(document);
    let index = SymbolIndex::new(&document.text, &analysis.syntax);
    let offset = position_to_offset(&document.text, position);
    let Some(symbol_id) = index.symbol_at(offset) else {
        return Ok(Value::Null);
    };
    let symbol = &index.symbols[symbol_id];
    let range = index
        .occurrence_at(offset)
        .map(|span| span_to_range(&document.text, span));
    let hover = Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("```kern\n{}\n```", symbol.detail),
        }),
        range,
    };
    serde_json::to_value(hover).map_err(|error| error.to_string())
}

fn definition_response(
    document: &Document,
    uri: &Uri,
    position: Position,
) -> Result<Value, String> {
    let analysis = get_analysis(document);
    let index = SymbolIndex::new(&document.text, &analysis.syntax);
    let offset = position_to_offset(&document.text, position);
    let Some(symbol) = index.symbol_at(offset) else {
        return Ok(Value::Null);
    };
    let location = Location {
        uri: uri.clone(),
        range: span_to_range(&document.text, index.symbols[symbol].definition),
    };
    serde_json::to_value(GotoDefinitionResponse::Scalar(location))
        .map_err(|error| error.to_string())
}

fn references_response(
    document: &Document,
    uri: &Uri,
    position: Position,
    include_declaration: bool,
) -> Result<Value, String> {
    let analysis = get_analysis(document);
    let index = SymbolIndex::new(&document.text, &analysis.syntax);
    let offset = position_to_offset(&document.text, position);
    let Some(symbol) = index.symbol_at(offset) else {
        return serde_json::to_value(Vec::<Location>::new()).map_err(|error| error.to_string());
    };
    let definition = index.symbols[symbol].definition;
    let locations = index
        .references(symbol)
        .into_iter()
        .filter(|span| include_declaration || *span != definition)
        .map(|span| Location {
            uri: uri.clone(),
            range: span_to_range(&document.text, span),
        })
        .collect::<Vec<_>>();
    serde_json::to_value(locations).map_err(|error| error.to_string())
}

fn rename_response(
    document: &Document,
    uri: &Uri,
    position: Position,
    new_name: &str,
) -> Result<Value, String> {
    if !valid_identifier(new_name) || is_keyword(new_name) {
        return Err("the new name is not a valid Kern identifier".to_string());
    }
    let analysis = get_analysis(document);
    let index = SymbolIndex::new(&document.text, &analysis.syntax);
    let offset = position_to_offset(&document.text, position);
    let Some(symbol) = index.symbol_at(offset) else {
        return Ok(Value::Null);
    };
    let edits = index
        .references(symbol)
        .into_iter()
        .map(|span| TextEdit {
            range: span_to_range(&document.text, span),
            new_text: new_name.to_string(),
        })
        .collect();
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);
    serde_json::to_value(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
    .map_err(|error| error.to_string())
}

fn document_symbols_response(document: &Document) -> Result<Value, String> {
    let analysis = get_analysis(document);
    let index = SymbolIndex::new(&document.text, &analysis.syntax);
    let symbols = index
        .global_symbols()
        .map(|(id, symbol)| document_symbol(&index, id, symbol, &document.text))
        .collect::<Vec<_>>();
    serde_json::to_value(symbols).map_err(|error| error.to_string())
}

#[allow(deprecated)]
fn document_symbol(
    index: &SymbolIndex,
    id: usize,
    symbol: &IndexedSymbol,
    source: &str,
) -> DocumentSymbol {
    let children = index.children(id);
    DocumentSymbol {
        name: symbol.name.clone(),
        detail: Some(symbol.detail.clone()),
        kind: symbol.kind,
        tags: None,
        deprecated: None,
        range: span_to_range(source, symbol.range),
        selection_range: span_to_range(source, symbol.definition),
        children: (!children.is_empty()).then(|| {
            children
                .into_iter()
                .map(|(child_id, child)| document_symbol(index, child_id, child, source))
                .collect()
        }),
    }
}

fn code_actions_response(document: &Document, params: &CodeActionParams) -> Result<Value, String> {
    let mut actions = Vec::new();
    for diagnostic in &params.context.diagnostics {
        if !diagnostic.message.contains("expected `;`") {
            continue;
        }
        let offset = position_to_offset(&document.text, diagnostic.range.start);
        if document.text[..offset.min(document.text.len())]
            .chars()
            .next_back()
            == Some(';')
        {
            continue;
        }
        let edit = TextEdit {
            range: lsp_types::Range::new(diagnostic.range.start, diagnostic.range.start),
            new_text: ";".to_string(),
        };
        let mut changes = HashMap::new();
        changes.insert(params.text_document.uri.clone(), vec![edit]);
        actions.push(CodeAction {
            title: "Insert missing semicolon".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }),
            is_preferred: Some(true),
            ..Default::default()
        });
    }
    serde_json::to_value(actions).map_err(|error| error.to_string())
}

fn get_analysis(document: &Document) -> SourceAnalysis {
    if let Some(analysis) = &document.analysis {
        return SourceAnalysis {
            syntax: analysis.syntax.clone(),
            typed: analysis.typed.clone(),
            diagnostics: analysis.diagnostics.clone(),
        };
    }
    compiler::analyze_source(&document.text)
}

fn enum_completion_context(prefix: &str) -> Option<String> {
    let before_separator = prefix.trim_end().strip_suffix("::")?.trim_end();
    let start = before_separator
        .char_indices()
        .rev()
        .find(|(_, character)| !is_identifier_continue(*character))
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(0);
    let name = &before_separator[start..];
    (!name.is_empty()).then(|| name.to_string())
}

fn field_completion_context(prefix: &str) -> Option<String> {
    let before_dot = prefix.trim_end().strip_suffix('.')?.trim_end();
    let start = before_dot
        .char_indices()
        .rev()
        .find(|(_, character)| !is_identifier_continue(*character))
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(0);
    let name = &before_dot[start..];
    (!name.is_empty()).then(|| name.to_string())
}

fn valid_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && characters.all(is_identifier_continue)
}

fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "fn" | "let"
            | "mut"
            | "if"
            | "else"
            | "while"
            | "loop"
            | "for"
            | "break"
            | "continue"
            | "return"
            | "struct"
            | "enum"
            | "match"
            | "use"
            | "true"
            | "false"
            | "print"
            | "println"
            | "cin"
    )
}

fn apply_changes(text: &mut String, changes: &[TextDocumentContentChangeEvent]) {
    for change in changes {
        if let Some(range) = change.range {
            let start = position_to_offset(text, range.start);
            let end = position_to_offset(text, range.end);
            let start = previous_char_boundary(text, start);
            let end = previous_char_boundary(text, end).max(start);
            text.replace_range(start..end, &change.text);
        } else {
            *text = change.text.clone();
        }
    }
}

fn previous_char_boundary(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn position_to_offset(text: &str, position: Position) -> usize {
    let target_line = position.line as usize;
    let target_character = position.character as usize;
    let mut line = 0usize;
    let mut offset = 0usize;
    for segment in text.split_inclusive('\n') {
        let content = segment.strip_suffix('\n').unwrap_or(segment);
        if line == target_line {
            let content = content.strip_suffix('\r').unwrap_or(content);
            let mut utf16 = 0usize;
            for (byte_index, character) in content.char_indices() {
                if utf16 >= target_character {
                    return offset + byte_index;
                }
                let width = character.len_utf16();
                if utf16 + width > target_character {
                    return offset + byte_index;
                }
                utf16 += width;
            }
            return offset + content.len();
        }
        offset += segment.len();
        line += 1;
    }
    if line == target_line {
        return text.len();
    }
    text.len()
}

fn offset_to_position(text: &str, offset: usize) -> Position {
    let offset = previous_char_boundary(text, offset);
    let prefix = &text[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let line_prefix = prefix[line_start..]
        .strip_suffix('\r')
        .unwrap_or(&prefix[line_start..]);
    Position::new(line as u32, line_prefix.encode_utf16().count() as u32)
}

fn span_to_range(source: &str, span: Span) -> lsp_types::Range {
    lsp_types::Range::new(
        offset_to_position(source, span.start),
        offset_to_position(source, span.end),
    )
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn log_debug(message: &str) {
    if std::env::var_os("KERN_LSP_LOG").is_some() {
        eprintln!("kern-lsp: {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_positions_using_utf16_units() {
        let text = "a😀b\nsecond";
        assert_eq!(position_to_offset(text, Position::new(0, 3)), 5);
        assert_eq!(offset_to_position(text, 5), Position::new(0, 3));
        assert_eq!(position_to_offset(text, Position::new(1, 2)), 9);
    }

    #[test]
    fn applies_incremental_edits_and_full_sync_changes() {
        let mut text = "let value = 1;\n".to_string();
        apply_changes(
            &mut text,
            &[TextDocumentContentChangeEvent {
                range: Some(lsp_types::Range::new(
                    Position::new(0, 12),
                    Position::new(0, 13),
                )),
                range_length: Some(1),
                text: "2".to_string(),
            }],
        );
        assert_eq!(text, "let value = 2;\n");
        apply_changes(
            &mut text,
            &[TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "fn main() {}".to_string(),
            }],
        );
        assert_eq!(text, "fn main() {}");
    }

    #[test]
    fn builds_real_symbol_index_for_locals_and_functions() {
        let source =
            "fn add(x: i32) -> i32 { return x; }\nfn main() { let y = add(1); println(y); }";
        let analysis = compiler::analyze_source(source);
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let index = SymbolIndex::new(source, &analysis.syntax);
        let function = index
            .symbols
            .iter()
            .position(|symbol| symbol.name == "add" && symbol.kind == SymbolKind::FUNCTION)
            .unwrap();
        assert_eq!(index.symbols[function].detail, "fn add(x: i32) -> i32");
        let local = index
            .symbols
            .iter()
            .position(|symbol| symbol.name == "y" && symbol.local)
            .unwrap();
        assert_eq!(index.references(local).len(), 2);
    }

    #[test]
    fn serves_symbol_navigation_and_completion_from_compiler_ast() {
        let source = "fn compute(value: i32) -> i32 { return value + 1; }\nfn main() { let answer: i32 = compute(4); println(answer); }";
        let analysis = compiler::analyze_source(source);
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let document = Document {
            text: source.to_string(),
            version: 1,
            analysis: Some(analysis),
        };
        let call_offset = source.find("compute(4)").unwrap();
        let hover: Value = serde_json::from_value(
            hover_response(&document, offset_to_position(source, call_offset)).unwrap(),
        )
        .unwrap();
        assert!(
            hover["contents"]["value"]
                .as_str()
                .unwrap()
                .contains("fn compute(value: i32) -> i32")
        );

        let use_offset = source.rfind("answer").unwrap();
        let definition: Value = serde_json::from_value(
            definition_response(
                &document,
                &"file:///tmp/test.kr".parse().unwrap(),
                offset_to_position(source, use_offset),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(definition["range"]["start"]["line"], 1);
        assert_eq!(definition["range"]["start"]["character"], 16);

        let completions: Value = serde_json::from_value(
            completion_response(&document, offset_to_position(source, use_offset)).unwrap(),
        )
        .unwrap();
        let items = completions
            .as_array()
            .or_else(|| completions["items"].as_array())
            .unwrap();
        assert!(items.iter().any(|item| item["label"] == "answer"));
    }
}
