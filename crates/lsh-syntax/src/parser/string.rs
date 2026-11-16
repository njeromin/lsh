use std::{iter::Peekable, str::CharIndices};

use crate::{
    diagnostics::{Diagnostic, DiagnosticKind},
    lexer::resolve_basic_escape,
    parser::ast::Expr,
};

use super::Parser;

impl Parser {
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
        src_id: usize,
        snippet: &str,
        offset_shift: usize,
        parse_fn: impl FnOnce(&mut Parser) -> Result<Expr, Diagnostic>,
    ) -> Result<Expr, Diagnostic> {
        let shift_diag = |mut diag: Diagnostic| {
            diag.source.start += offset_shift;
            diag.source.end += offset_shift;
            diag
        };

        let tokens = crate::lexer::lex_input(src_id, snippet).map_err(shift_diag)?;
        let mut parser = Parser::new(src_id, tokens);
        parse_fn(&mut parser).map_err(shift_diag)
    }

    pub fn parse_interpolated_string(
        src_id: usize,
        raw: &str,
        base_offset: usize,
    ) -> Result<Expr, Diagnostic> {
        let mut parts: Vec<Expr> = Vec::new();
        let mut current_literal = String::new();
        let mut chars = raw.char_indices().peekable();
        let content_base = base_offset + 1; // skip leading '"'

        while let Some((idx, ch)) = chars.next() {
            if ch == '\\' {
                let Some((_, next_ch)) = chars.next() else {
                    return Err(Diagnostic::new(
                        DiagnosticKind::ParseError,
                        src_id,
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
                continue;
            }

            if ch == '$' {
                if let Some(&(_, '(')) = chars.peek() {
                    chars.next(); // consume '('
                    let Some(sub_end_idx) = Self::scan_balanced_delimited(&mut chars, '(', ')')
                    else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            src_id,
                            content_base + idx,
                            content_base + raw.len(),
                            "Unterminated command substitution in string interpolation.",
                        ));
                    };

                    let parsed_expr = Self::lex_and_parse_embedded(
                        src_id,
                        &raw[idx..=sub_end_idx],
                        content_base + idx,
                        |p| p.parse_primary(),
                    )?;

                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(parsed_expr);
                    continue;
                }

                if let Some(&(_, '{')) = chars.peek() {
                    chars.next(); // consume '{'
                    let expr_start_idx = idx + 2; // after '${'
                    let Some(sub_end_idx) = Self::scan_balanced_delimited(&mut chars, '{', '}')
                    else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            src_id,
                            content_base + idx,
                            content_base + raw.len(),
                            "Unterminated expression substitution in string interpolation.",
                        ));
                    };

                    let expr_str = &raw[expr_start_idx..sub_end_idx];
                    if expr_str.trim().is_empty() {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            src_id,
                            content_base + idx,
                            content_base + sub_end_idx + 1,
                            "Empty expression in string interpolation.",
                        ));
                    }

                    let parsed_expr = Self::lex_and_parse_embedded(
                        src_id,
                        expr_str,
                        content_base + expr_start_idx,
                        |p| p.parse_expr(),
                    )?;

                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(parsed_expr);
                    continue;
                }

                // Special variables: $?, $$, $!, $#, $*, $@
                if let Some(&(_, next_c)) = chars.peek()
                    && matches!(next_c, '?' | '$' | '!' | '#' | '*' | '@')
                {
                    chars.next();
                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(Expr::Variable(next_c.to_string()));
                    continue;
                }

                // Variable substitution: $var
                if let Some(&(_, next_c)) = chars.peek()
                    && (next_c == '_' || next_c.is_ascii_alphabetic())
                {
                    let mut var_name = String::new();
                    while let Some(&(_, var_c)) = chars.peek() {
                        if var_c == '_' || var_c.is_ascii_alphanumeric() {
                            var_name.push(var_c);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(Expr::Variable(var_name));
                    continue;
                }

                // Literal '$'
                current_literal.push('$');
                continue;
            }

            current_literal.push(ch);
        }

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
