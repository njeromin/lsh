use std::{iter::Peekable, str::CharIndices};

use crate::{
    diagnostics::{Diagnostic, DiagnosticKind},
    lexer::resolve_basic_escape,
    parser::ast::Expr,
};

use super::Parser;

impl Parser<'_> {
    fn scan_balanced_delimited(
        chars: &mut Peekable<CharIndices<'_>>,
        open: char,
        close: char,
    ) -> Option<usize> {
        let mut depth = 1usize;

        while let Some((inner_idx, inner_ch)) = chars.next() {
            if inner_ch == '\\' {
                chars.next();
                continue;
            }
            if inner_ch == '"' || inner_ch == '\'' {
                let quote = inner_ch;
                while let Some((_, qc)) = chars.next() {
                    if qc == '\\' {
                        chars.next();
                        continue;
                    }
                    if qc == quote {
                        break;
                    }
                }
                continue;
            }
            if inner_ch == open {
                depth += 1;
            } else if inner_ch == close {
                depth -= 1;
                if depth == 0 {
                    return Some(inner_idx);
                }
            }
        }

        None
    }

    fn lex_and_parse_embedded(
        snippet: &str,
        offset_shift: usize,
        parse_fn: impl FnOnce(&mut Parser) -> Result<Expr, Diagnostic>,
    ) -> Result<Expr, Diagnostic> {
        let shift_diag = |mut diag: Diagnostic| {
            diag.source.start += offset_shift;
            diag.source.end += offset_shift;
            diag
        };

        let tokens = crate::lexer::lex_input(snippet).map_err(shift_diag)?;
        let mut parser = Parser::new(tokens);
        parse_fn(&mut parser).map_err(shift_diag)
    }

    pub fn parse_interpolated_string(raw: &str, base_offset: usize) -> Result<Expr, Diagnostic> {
        if !raw.contains(['\\', '$']) {
            return Ok(Expr::QuotedString(raw.to_owned()));
        }

        let mut parts: Vec<Expr> = Vec::new();
        let mut current_literal = String::new();
        let mut lit_start = 0usize;
        let mut chars = raw.char_indices().peekable();
        let content_base = base_offset + 1; // skip leading '"'

        while let Some((idx, ch)) = chars.next() {
            if ch == '\\' {
                current_literal.push_str(&raw[lit_start..idx]);
                let Some((esc_idx, next_ch)) = chars.next() else {
                    return Err(Diagnostic::new(
                        DiagnosticKind::ParseError,
                        content_base + idx,
                        content_base + idx + 1,
                        "Unterminated escape sequence in string literal.",
                    ));
                };
                match next_ch {
                    '"' => current_literal.push('"'),
                    '$' => current_literal.push('$'),
                    other => current_literal.push(resolve_basic_escape(other)),
                }
                lit_start = esc_idx + next_ch.len_utf8();
                continue;
            }

            if ch == '$' {
                if let Some(&(_, '(')) = chars.peek() {
                    chars.next(); // consume '('
                    let Some(sub_end_idx) = Self::scan_balanced_delimited(&mut chars, '(', ')')
                    else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            content_base + idx,
                            content_base + raw.len(),
                            "Unterminated command substitution in string interpolation.",
                        ));
                    };

                    let parsed_expr = Self::lex_and_parse_embedded(
                        &raw[idx..=sub_end_idx],
                        content_base + idx,
                        |p| p.parse_primary(),
                    )?;

                    current_literal.push_str(&raw[lit_start..idx]);
                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(parsed_expr);
                    lit_start = sub_end_idx + 1;
                    continue;
                }

                if let Some(&(_, '{')) = chars.peek() {
                    chars.next(); // consume '{'
                    let expr_start_idx = idx + 2; // after '${'
                    let Some(sub_end_idx) = Self::scan_balanced_delimited(&mut chars, '{', '}')
                    else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            content_base + idx,
                            content_base + raw.len(),
                            "Unterminated expression substitution in string interpolation.",
                        ));
                    };

                    let expr_str = &raw[expr_start_idx..sub_end_idx];
                    if expr_str.trim().is_empty() {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            content_base + idx,
                            content_base + sub_end_idx + 1,
                            "Empty expression in string interpolation.",
                        ));
                    }

                    let parsed_expr = Self::lex_and_parse_embedded(
                        expr_str,
                        content_base + expr_start_idx,
                        |p| p.parse_expr(),
                    )?;

                    current_literal.push_str(&raw[lit_start..idx]);
                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(parsed_expr);
                    lit_start = sub_end_idx + 1;
                    continue;
                }

                // Special variables: $?, $$, $!, $#, $*, $@
                if let Some(&(_, next_c)) = chars.peek()
                    && matches!(next_c, '?' | '$' | '!' | '#' | '*' | '@')
                {
                    let (next_idx, _) = chars.next().unwrap();
                    current_literal.push_str(&raw[lit_start..idx]);
                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    let next_end = next_idx + next_c.len_utf8();
                    parts.push(Expr::Variable(raw[next_idx..next_end].to_owned()));
                    lit_start = next_end;
                    continue;
                }

                // Variable substitution: $var
                if let Some(&(var_start, next_c)) = chars.peek()
                    && (next_c == '_' || next_c.is_ascii_alphabetic())
                {
                    let mut var_end = var_start;
                    while let Some(&(char_idx, var_c)) = chars.peek() {
                        if var_c == '_' || var_c.is_ascii_alphanumeric() {
                            var_end = char_idx + var_c.len_utf8();
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    current_literal.push_str(&raw[lit_start..idx]);
                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(Expr::Variable(raw[var_start..var_end].to_owned()));
                    lit_start = var_end;
                    continue;
                }

                // Literal '$'
                continue;
            }
        }

        current_literal.push_str(&raw[lit_start..]);

        if parts.is_empty() {
            Ok(Expr::QuotedString(current_literal))
        } else {
            if !current_literal.is_empty() {
                parts.push(Expr::QuotedString(current_literal));
            }
            Ok(Expr::InterpolatedString(parts))
        }
    }
}
