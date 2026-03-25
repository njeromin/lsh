use crossterm::{
    queue,
    style::{self, Stylize},
};

#[derive(Debug)]
pub enum DiagnosticKind {
    LexError,
    ParseError,
    RuntimeError,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub source: SourceInfo,
    pub notes: Vec<String>,
    pub hint: Option<String>,
}

// TODO: Source maps
#[derive(Debug)]
pub struct SourceInfo {
    pub source_name: String,
    pub source: String,
    pub start: usize,
    pub end: usize,
}

fn line_col_from_offset(source: &str, offset: usize) -> (usize, usize) {
    let clamped_offset = offset.min(source.len());
    let mut line = 1usize;
    let mut col = 1usize;

    for (idx, ch) in source.char_indices() {
        if idx >= clamped_offset {
            break;
        }

        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    (line, col)
}

fn line_text_at_offset(source: &str, offset: usize) -> String {
    let clamped_offset = offset.min(source.len());
    let line_start = source[..clamped_offset]
        .rfind('\n')
        .map_or(0, |idx| idx + 1);
    let line_end = source[clamped_offset..]
        .find('\n')
        .map_or(source.len(), |idx| clamped_offset + idx);

    source[line_start..line_end].to_string()
}

fn format_kind(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::LexError => "lex error",
        DiagnosticKind::ParseError => "parse error",
        DiagnosticKind::RuntimeError => "runtime error",
    }
}

pub fn print_diagnostic<W: std::io::Write>(out: &mut W, diag: &Diagnostic) {
    let src = &diag.source;
    let (line_num, col_num) = line_col_from_offset(&src.source, src.start);
    let line_text = line_text_at_offset(&src.source, src.start);
    let caret_padding = col_num.saturating_sub(1);

    queue!(
        out,
        style::Print(format!("{}: ", format_kind(&diag.kind)).red()),
        style::Print(format!("{}\n", diag.message)),
        style::Print("---> ".blue()),
        style::Print(format!("{}:{}:{}\n", src.source_name, line_num, col_num)),
    )
    .expect("Failed to write diagnostic header");

    queue!(out, style::Print(" |  \n".blue())).unwrap();

    queue!(out, style::Print(" |  ".blue())).unwrap();
    queue!(out, style::Print(format!("{}\n", line_text))).unwrap();

    queue!(out, style::Print(" |  ".blue())).unwrap();
    queue!(out, style::Print(" ".repeat(caret_padding))).unwrap();
    queue!(out, style::Print("^".red())).unwrap();
    queue!(out, style::Print("\n")).unwrap();

    for note in &diag.notes {
        queue!(out, style::Print(" |  ".blue())).unwrap();
        queue!(out, style::Print(format!("{}\n", note))).unwrap();
    }
    if let Some(hint) = &diag.hint {
        queue!(out, style::Print(" |  ".blue())).unwrap();
        queue!(out, style::Print(format!("{}\n", hint))).unwrap();
    }
}
