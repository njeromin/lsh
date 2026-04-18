use crossterm::{
    queue,
    style::{self, Stylize},
};

use crate::{diagnostics::source_map::SourceMap, ui::print_error};

pub mod source_map;

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

impl Diagnostic {
    pub fn new(
        kind: DiagnosticKind,
        source_id: usize,
        start: usize,
        end: usize,
        message: &str,
    ) -> Self {
        Self {
            kind,
            message: message.to_string(),
            source: SourceInfo {
                source_id,
                start,
                end,
            },
            notes: Vec::new(),
            hint: None,
        }
    }
}

#[derive(Debug)]
pub struct SourceInfo {
    pub source_id: usize,
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

pub fn print_diagnostic<W: std::io::Write>(out: &mut W, diag: &Diagnostic, source_map: &SourceMap) {
    let src_info = &diag.source;
    let src = source_map.get(src_info.source_id);

    if src.is_none() {
        print_error(out, "Failed to find source for diagnostic.");
        return;
    }

    let src = src.unwrap();

    let (line_num, col_num) = line_col_from_offset(&src.contents, src_info.start);
    let line_text = line_text_at_offset(&src.contents, src_info.start);
    let caret_padding = col_num.saturating_sub(1);

    queue!(
        out,
        style::Print(format!("{}: ", format_kind(&diag.kind)).red()),
        style::Print(format!("{}\n", diag.message)),
        style::Print("---> ".blue()),
        style::Print(format!("{}:{}:{}\n", src.name, line_num, col_num)),
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
