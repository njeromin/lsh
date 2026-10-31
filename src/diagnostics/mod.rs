use crossterm::{
    queue,
    style::{self, Stylize},
};

use crate::{diagnostics::source_map::SourceMap, terminal::print_error};

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

fn line_bounds_at_offset(source: &str, offset: usize) -> (usize, usize) {
    let clamped_offset = offset.min(source.len());
    let line_start = source[..clamped_offset]
        .rfind('\n')
        .map_or(0, |idx| idx + 1);
    let line_end = source[clamped_offset..]
        .find('\n')
        .map_or(source.len(), |idx| clamped_offset + idx);
    (line_start, line_end)
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
    let (line_start, line_end) = line_bounds_at_offset(&src.contents, src_info.start);
    let underline_start = src_info.start.clamp(line_start, line_end);
    let underline_end = src_info.end.clamp(underline_start, line_end);
    let underline_len = (underline_end.saturating_sub(underline_start)).max(1);
    let caret_padding = col_num.saturating_sub(1);
    let underline = "^".repeat(underline_len);

    queue!(
        out,
        style::Print(format!("{}: ", format_kind(&diag.kind)).red()),
        style::Print(format!("{}\n", diag.message)),
        style::Print("---> ".blue()),
        style::Print(format!("{}:{}:{}\n", src.name, line_num, col_num)),
        style::Print(" |  \n".blue()),
        style::Print(" |  ".blue()),
        style::Print(format!("{line_text}\n")),
        style::Print(" |  ".blue()),
        style::Print(" ".repeat(caret_padding)),
        style::Print(underline.red()),
        style::Print("\n"),
    )
    .expect("Failed to print diagnostic");

    for note in &diag.notes {
        queue!(out, style::Print(" |  ".blue()), style::Print(format!("{note}\n")))
            .expect("Failed to print diagnostic note");
    }

    if let Some(hint) = &diag.hint {
        queue!(out, style::Print(" |  ".blue()), style::Print(format!("{hint}\n")))
            .expect("Failed to print diagnostic hint");
    }
}
