use std::fmt::Write as _;

/// A half-open byte range in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn join(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// A collection of diagnostics produced by one or more compiler stages.
#[derive(Debug, Default, Clone)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub fn error(&mut self, span: Span, message: impl Into<String>) {
        self.push(Diagnostic::error(span, message));
    }

    pub fn error_with_note(
        &mut self,
        span: Span,
        message: impl Into<String>,
        note: impl Into<String>,
    ) {
        self.push(Diagnostic::error(span, message).with_note(note));
    }

    pub fn extend(&mut self, other: impl IntoIterator<Item = Diagnostic>) {
        self.items.extend(other);
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(Diagnostic::is_error)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.items
    }
}

/// Source text plus a line index, used to render diagnostics with locations.
pub struct SourceFile {
    name: String,
    source: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, source: impl Into<String>) -> Self {
        let source = source.into();
        let mut line_starts = vec![0];
        for (offset, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self {
            name: name.into(),
            source,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns a zero-based line index and one-based column.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.source.len());
        let line = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        let column = self.source[line_start..offset].chars().count() + 1;
        (line, column)
    }

    fn line_text(&self, line: usize) -> &str {
        let start = self.line_starts[line];
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.source.len());
        self.source[start..end].trim_end_matches(['\n', '\r'])
    }

    pub fn render(&self, diagnostics: &[Diagnostic]) -> String {
        let mut output = String::new();
        for diagnostic in diagnostics {
            let (line, column) = self.line_col(diagnostic.span.start);
            let line_text = self.line_text(line);
            let _ = writeln!(
                output,
                "{}: {}",
                diagnostic.severity.label(),
                diagnostic.message
            );
            let _ = writeln!(
                output,
                " --> {}:{}:{}",
                self.name,
                line + 1,
                column
            );
            let _ = writeln!(output, "  |");
            let _ = writeln!(output, "{:>3} | {}", line + 1, line_text);

            // Clamp the caret to the diagnostic's first line.
            let line_end = self
                .line_starts
                .get(line + 1)
                .copied()
                .unwrap_or(self.source.len());
            let span_end = diagnostic.span.end.min(line_end);
            let width = self.source[diagnostic.span.start.min(self.source.len())..span_end]
                .chars()
                .count()
                .max(1);
            let mut caret = String::new();
            for _ in 1..column {
                caret.push(' ');
            }
            for _ in 0..width {
                caret.push('^');
            }
            let _ = writeln!(output, "  | {}", caret);

            for note in &diagnostic.notes {
                let _ = writeln!(output, "  = note: {}", note);
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_location_and_caret() {
        let source = SourceFile::new("test.kr", "fn main() {\n    bad;\n}\n");
        let diagnostic = Diagnostic::error(Span::new(16, 19), "undefined variable `bad`");
        let rendered = source.render(&[diagnostic]);
        assert!(rendered.contains("test.kr:2:5"));
        assert!(rendered.contains("bad;"));
        assert!(rendered.contains("^^^"));
    }
}
