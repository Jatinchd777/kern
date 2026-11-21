use crate::diagnostics::Diagnostic;
use crate::hir;
use crate::lexer::lex;
use crate::parser::{self, ast};
use crate::semantic;

pub struct CheckedProgram {
    pub ast: ast::Program,
    pub hir: hir::Program,
}

/// Runs the lexer, parser, and semantic analyzer. Code generation is only
/// attempted when the frontend produced no errors.
pub fn check_source(source: &str) -> Result<CheckedProgram, Vec<Diagnostic>> {
    let lexed = lex(source);
    let mut diagnostics = lexed.diagnostics;
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }

    let parsed = parser::parse(lexed.tokens);
    diagnostics.extend(parsed.diagnostics);
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }

    let analyzed = semantic::analyze(parsed.program.clone());
    diagnostics.extend(analyzed.diagnostics);
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }

    Ok(CheckedProgram {
        ast: parsed.program,
        hir: analyzed.program,
    })
}
