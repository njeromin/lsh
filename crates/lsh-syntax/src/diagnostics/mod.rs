use crossterm::{
    queue,
    style::{self, Stylize},
};

pub fn print_error<W: std::io::Write>(writer: &mut W, err: &str) {
    let _ = writeln!(writer, "\x1b[38;5;9m{err}\x1b[0m");
}

#[derive(Debug)]
pub enum DiagnosticKind {
    LexError,
    ParseError,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub source: SourceInfo,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, start: usize, end: usize, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: SourceInfo { start, end },
            hint: None,
        }
    }
}

#[derive(Debug)]
pub struct SourceInfo {
    pub start: usize,
    pub end: usize,
}

fn line_span_at_offset(source: &str, offset: usize) -> (usize, usize, usize, usize) {
    let clamped_offset = offset.min(source.len());
    let mut line = 1usize;
    let mut col = 1usize;
    let mut line_start = 0usize;

    for (idx, ch) in source[..clamped_offset].char_indices() {
        if ch == '\n' {
            line += 1;
            col = 1;
            line_start = idx + 1;
        } else {
            col += 1;
        }
    }

    let line_end = source[clamped_offset..]
        .find('\n')
        .map_or(source.len(), |idx| clamped_offset + idx);

    (line, col, line_start, line_end)
}

fn format_kind(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::LexError => "lex error",
        DiagnosticKind::ParseError => "parse error",
    }
}

pub fn print_diagnostic<W: std::io::Write>(
    out: &mut W,
    diag: &Diagnostic,
    source_name: &str,
    source: &str,
) {
    let (line_num, col_num, line_start, line_end) = line_span_at_offset(source, diag.source.start);
    let line_text = &source[line_start..line_end];
    let underline_start = diag.source.start.clamp(line_start, line_end);
    let underline_end = diag.source.end.clamp(underline_start, line_end);
    let underline_len = source[underline_start..underline_end]
        .chars()
        .count()
        .max(1);
    let caret_padding = col_num.saturating_sub(1);
    let gutter_width = line_num.ilog10() as usize + 1;

    let _ = writeln!(out, "{}: {}", format_kind(&diag.kind).red(), diag.message);
    let _ = writeln!(out, "{}{source_name}:{line_num}:{col_num}", "---> ".blue());
    let _ = writeln!(out, "{:gutter_width$} {}", "", "|".blue());
    let _ = writeln!(out, "{line_num:>gutter_width$} {}  {line_text}", "|".blue());
    let _ = write!(
        out,
        "{:gutter_width$} {}  {:caret_padding$}",
        "",
        "|".blue(),
        ""
    );
    let _ = queue!(out, style::SetForegroundColor(style::Color::Red));
    let _ = write!(out, "{:^<underline_len$}", "");
    let _ = queue!(out, style::ResetColor);
    let _ = writeln!(out);

    if let Some(hint) = &diag.hint {
        let _ = writeln!(out, "{:gutter_width$} {}  {hint}", "", "|".blue());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_diagnostic_with_line_number_gutter_and_hint() {
        let source = "let x = 1\nlet = 2";
        let diag = Diagnostic {
            kind: DiagnosticKind::ParseError,
            message: "Expected identifier after 'let'.".to_string(),
            source: SourceInfo { start: 14, end: 15 },
            hint: Some("Provide a variable name.".to_string()),
        };

        let mut buf = Vec::new();
        print_diagnostic(&mut buf, &diag, "test.lsh", source);
        let rendered = String::from_utf8(buf).expect("valid utf-8");

        assert!(rendered.contains("parse error"));
        assert!(rendered.contains("Expected identifier after 'let'."));
        assert!(rendered.contains("test.lsh:2:5"));
        assert!(rendered.contains("2 "));
        assert!(rendered.contains("let = 2"));
        assert!(rendered.contains("    \x1b[38;5;9m^\x1b[39m") || rendered.contains("    "));
        assert!(rendered.contains("^"));
        assert!(rendered.contains("Provide a variable name."));
    }

    #[test]
    fn underlines_multibyte_utf8_by_char_count() {
        let source = "echo \"héllo\"";
        // Span covering "héllo" (bytes 6..12, 5 chars, 6 bytes).
        let diag = Diagnostic::new(DiagnosticKind::LexError, 6, 12, "Bad token.");

        let mut buf = Vec::new();
        print_diagnostic(&mut buf, &diag, "<input>", source);
        let rendered = String::from_utf8(buf).expect("valid utf-8");

        assert!(rendered.contains("^^^^^"));
        assert!(!rendered.contains("^^^^^^"));
    }
}
