use crate::{
    diagnostics::{Diagnostic, DiagnosticKind},
    parser::ast::Expr,
};

use super::Parser;

impl Parser {
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
                    'n' => current_literal.push('\n'),
                    'r' => current_literal.push('\r'),
                    't' => current_literal.push('\t'),
                    '0' => current_literal.push('\0'),
                    '\\' => current_literal.push('\\'),
                    '"' => current_literal.push('"'),
                    '\'' => current_literal.push('\''),
                    '$' => current_literal.push('$'),
                    other => {
                        current_literal.push(other);
                    }
                }
                continue;
            }

            if ch == '$' {
                if let Some(&(_, '(')) = chars.peek() {
                    chars.next(); // consume '('
                    let sub_start_idx = idx; // starts at '$'

                    let mut paren_depth = 1usize;
                    let mut expr_end_idx = None;

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
                        if inner_ch == '(' {
                            paren_depth += 1;
                        } else if inner_ch == ')' {
                            paren_depth -= 1;
                            if paren_depth == 0 {
                                expr_end_idx = Some(inner_idx);
                                break;
                            }
                        }
                    }

                    let Some(sub_end_idx) = expr_end_idx else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            src_id,
                            content_base + sub_start_idx,
                            content_base + raw.len(),
                            "Unterminated command substitution in string interpolation.",
                        ));
                    };

                    let cmd_raw_with_dollar = &raw[sub_start_idx..=sub_end_idx];

                    let tokens = crate::lexer::lex_input(src_id, cmd_raw_with_dollar).map_err(
                        |mut diag| {
                            diag.source.start += content_base + sub_start_idx;
                            diag.source.end += content_base + sub_start_idx;
                            diag
                        },
                    )?;

                    let mut parser = Parser::new(src_id, tokens);
                    let parsed_expr = parser.parse_primary().map_err(|mut diag| {
                        diag.source.start += content_base + sub_start_idx;
                        diag.source.end += content_base + sub_start_idx;
                        diag
                    })?;

                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(parsed_expr);
                    continue;
                }

                if let Some(&(_, '{')) = chars.peek() {
                    chars.next(); // consume '{'
                    let sub_start_idx = idx; // starts at '$'
                    let expr_start_idx = idx + 2; // after '${'

                    let mut brace_depth = 1usize;
                    let mut expr_end_idx = None;

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
                        if inner_ch == '{' {
                            brace_depth += 1;
                        } else if inner_ch == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                expr_end_idx = Some(inner_idx);
                                break;
                            }
                        }
                    }

                    let Some(sub_end_idx) = expr_end_idx else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            src_id,
                            content_base + sub_start_idx,
                            content_base + raw.len(),
                            "Unterminated expression substitution in string interpolation.",
                        ));
                    };

                    let expr_str = &raw[expr_start_idx..sub_end_idx];
                    if expr_str.trim().is_empty() {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ParseError,
                            src_id,
                            content_base + sub_start_idx,
                            content_base + sub_end_idx + 1,
                            "Empty expression in string interpolation.",
                        ));
                    }

                    let tokens =
                        crate::lexer::lex_input(src_id, expr_str).map_err(|mut diag| {
                            diag.source.start += content_base + expr_start_idx;
                            diag.source.end += content_base + expr_start_idx;
                            diag
                        })?;

                    let mut parser = Parser::new(src_id, tokens);
                    let parsed_expr = parser.parse_expr().map_err(|mut diag| {
                        diag.source.start += content_base + expr_start_idx;
                        diag.source.end += content_base + expr_start_idx;
                        diag
                    })?;

                    if !current_literal.is_empty() {
                        parts.push(Expr::QuotedString(std::mem::take(&mut current_literal)));
                    }
                    parts.push(parsed_expr);
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
