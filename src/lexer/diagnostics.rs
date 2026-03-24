use crate::diagnostics::{Diagnostic, DiagnosticKind, SourceInfo};

pub fn create_lex_diagnostic(
    source_name: &str,
    source: &str,
    start: usize,
    end: usize,
    message: &str,
) -> Diagnostic {
    Diagnostic {
        kind: DiagnosticKind::LexError,
        message: message.to_string(),
        source: SourceInfo {
            source_name: source_name.to_string(),
            source: source.to_string(),
            start,
            end,
        },
        notes: Vec::new(),
        hint: None,
    }
}
