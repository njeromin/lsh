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

fn format_kind(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::LexError => "lex error",
        DiagnosticKind::ParseError => "parse error",
        DiagnosticKind::RuntimeError => "runtime error",
    }
}

pub fn print_diagnostic<W: std::io::Write>(out: &mut W, diag: &Diagnostic) {
    let src = &diag.source;
    let line = src
        .source
        .lines()
        .nth(src.start / src.source.len())
        .unwrap_or("");
    let col = src.start % src.source.len();

    queue!(
        out,
        style::Print(format!("{}: ", format_kind(&diag.kind)).red()),
        style::Print(format!("{}\n", diag.message)),
        style::Print("---> ".blue()),
        style::Print(format!("{}:{}:{}\n", src.source_name, src.start, src.end)),
    )
    .expect("Failed to write diagnostic header");

    queue!(out, style::Print(" |  \n".blue())).unwrap();

    queue!(out, style::Print(" |  ".blue())).unwrap();
    queue!(out, style::Print(format!("{}\n", line))).unwrap();

    queue!(out, style::Print(" |  ".blue())).unwrap();
    queue!(out, style::Print(" ".repeat(col))).unwrap();
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
