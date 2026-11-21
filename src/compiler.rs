use crate::diagnostics::Diagnostic;
use crate::hir;
use crate::lexer::lex;
use crate::parser;
use crate::semantic;

pub struct CheckedProgram {
    pub hir: hir::Program,
}

pub struct SourceAnalysis {
    pub syntax: parser::ast::Program,
    pub typed: Option<hir::Program>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Runs the compiler frontend and returns its syntax tree even when analysis
/// reports errors, allowing editor tooling to inspect incomplete documents.
pub fn analyze_source(source: &str) -> SourceAnalysis {
    let lexed = lex(source);
    let mut diagnostics = lexed.diagnostics;
    let parsed = parser::parse(lexed.tokens);
    diagnostics.extend(parsed.diagnostics);
    let syntax = parsed.program;

    let has_frontend_errors = diagnostics.iter().any(Diagnostic::is_error);
    if has_frontend_errors {
        return SourceAnalysis {
            syntax,
            typed: None,
            diagnostics,
        };
    }

    let analyzed = semantic::analyze(syntax.clone());
    diagnostics.extend(analyzed.diagnostics);
    SourceAnalysis {
        syntax,
        typed: Some(analyzed.program),
        diagnostics,
    }
}

/// Runs the lexer, parser, and semantic analyzer. Code generation is only
/// attempted when the frontend produced no errors.
pub fn check_source(source: &str) -> Result<CheckedProgram, Vec<Diagnostic>> {
    let lexed = lex(source);
    if lexed.has_errors() {
        return Err(lexed.diagnostics);
    }

    let mut diagnostics = Vec::new();
    let parsed = parser::parse(lexed.tokens);
    diagnostics.extend(parsed.diagnostics);
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }

    let analyzed = semantic::analyze(parsed.program);
    diagnostics.extend(analyzed.diagnostics);
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }

    Ok(CheckedProgram {
        hir: analyzed.program,
    })
}
