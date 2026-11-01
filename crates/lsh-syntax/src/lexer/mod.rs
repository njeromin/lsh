use std::{char, iter::Peekable, str::CharIndices};

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::lexer::token::{Token, TokenKind};

pub mod token;

type CharIndexedIter<'a> = Peekable<CharIndices<'a>>;

pub fn lex_input(src_id: usize, input: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = vec![];

    let mut chars: CharIndexedIter = input.char_indices().peekable();

    while let Some((offset, char)) = chars.next() {
        match char {
            '|' => {
                if chars.next_if(|(_, next_char)| next_char == &'|').is_some() {
                    tokens.push(Token::new(TokenKind::Or, offset, 2));
                } else {
                    tokens.push(Token::new(TokenKind::Pipe, offset, 1));
                }
            }
            '&' => {
                if chars.next_if(|(_, next_char)| next_char == &'&').is_some() {
                    tokens.push(Token::new(TokenKind::And, offset, 2));
                } else {
                    tokens.push(Token::new(TokenKind::Ampersand, offset, 1));
                }
            }
            ';' => tokens.push(Token::new(TokenKind::Semicolon, offset, 1)),
            ',' => tokens.push(Token::new(TokenKind::Comma, offset, 1)),
            '.' => tokens.push(Token::new(TokenKind::Dot, offset, 1)),
            ':' => tokens.push(Token::new(TokenKind::Colon, offset, 1)),
            '=' => {
                if chars.next_if(|(_, next_char)| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::EqualsEquals, offset, 2));
                } else if chars.next_if(|(_, next_char)| next_char == &'>').is_some() {
                    tokens.push(Token::new(TokenKind::FatArrow, offset, 2));
                } else {
                    tokens.push(Token::new(TokenKind::Equals, offset, 1));
                }
            }
            '>' => {
                if chars.next_if(|(_, next_char)| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::GtEquals, offset, 2));
                } else {
                    tokens.push(Token::new(TokenKind::GreaterThan, offset, 1));
                }
            }
            '<' => {
                if chars.next_if(|(_, next_char)| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::LtEquals, offset, 2));
                } else {
                    tokens.push(Token::new(TokenKind::LessThan, offset, 1));
                }
            }
            '!' => {
                if chars.next_if(|(_, next_char)| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::NotEquals, offset, 2));
                } else {
                    tokens.push(Token::new(TokenKind::Not, offset, 1));
                }
            }
            '+' => {
                if let Some(next_char) = chars.peek() {
                    match next_char.1 {
                        '=' => {
                            tokens.push(Token::new(TokenKind::PlusEquals, offset, 2));
                            chars.next();
                        }
                        ' ' => tokens.push(Token::new(TokenKind::Plus, offset, 1)),
                        _ => {
                            if lex_identifier(&mut tokens, '+', offset, &mut chars).is_none() {
                                tokens.push(Token::new(TokenKind::Plus, offset, 1));
                            }
                        }
                    }
                } else {
                    tokens.push(Token::new(TokenKind::Plus, offset, 1));
                }
            }
            '-' => match chars.peek() {
                Some((_, '>')) => {
                    tokens.push(Token::new(TokenKind::ThinArrow, offset, 2));
                    chars.next();
                }
                Some((_, '=')) => {
                    tokens.push(Token::new(TokenKind::MinusEquals, offset, 2));
                    chars.next();
                }
                Some((_, ' ')) => tokens.push(Token::new(TokenKind::Minus, offset, 1)),
                Some((_, c)) if c.is_ascii_digit() => {
                    if lex_identifier(&mut tokens, char, offset, &mut chars).is_none() {
                        tokens.push(Token::new(TokenKind::Minus, offset, 1));
                    }
                }
                Some(_) => {
                    if lex_identifier(&mut tokens, '-', offset, &mut chars).is_none() {
                        tokens.push(Token::new(TokenKind::Minus, offset, 1));
                    }
                }
                None => tokens.push(Token::new(TokenKind::Minus, offset, 1)),
            },
            '*' => {
                if chars.next_if(|(_, next_char)| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::StarEquals, offset, 2));
                } else {
                    tokens.push(Token::new(TokenKind::Star, offset, 1));
                }
            }
            '$' => {
                if let Some(next_char) = chars.peek() {
                    if next_char.1 == '_' || next_char.1.is_ascii_alphanumeric() {
                        let (first_offset, first_var_char) = chars
                            .next()
                            .expect("Failed to get starting character of environment variable.");
                        let mut var_string = String::from(first_var_char);
                        let mut end_offset = first_offset + first_var_char.len_utf8();

                        while let Some(&(var_char_offset, var_char)) = chars.peek() {
                            if var_char == '_' || var_char.is_ascii_alphanumeric() {
                                var_string.push(var_char);
                                end_offset = var_char_offset + var_char.len_utf8();
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        tokens.push(Token::new(
                            TokenKind::Variable(var_string),
                            offset,
                            end_offset - offset,
                        ));
                    } else if next_char.1 == '(' {
                        tokens.push(Token::new(TokenKind::DollarLeftParen, offset, 2));
                        chars.next();
                    } else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::LexError,
                            src_id,
                            next_char.0,
                            next_char.0 + next_char.1.len_utf8(),
                            format!("Unexpected token '{}' after '$'.", next_char.1).as_str(),
                        ));
                    }
                } else {
                    return Err(Diagnostic::new(
                        DiagnosticKind::LexError,
                        src_id,
                        offset,
                        offset + char.len_utf8(),
                        "Unexpected end of input after '$'.",
                    ));
                }
            }
            '/' => match chars.peek().copied().map(|(_, c)| c) {
                Some('=') => {
                    chars.next();
                    tokens.push(Token::new(TokenKind::SlashEquals, offset, 2));
                }
                Some(_) | None => {
                    if lex_identifier(&mut tokens, '/', offset, &mut chars).is_none() {
                        tokens.push(Token::new(TokenKind::Slash, offset, 1));
                    }
                }
            },
            '#' => {
                // Skip single-line comments.
                for (comment_offset, comment_char) in chars.by_ref() {
                    if comment_char == '\n' {
                        if chars.peek().is_some_and(|(_, next_char)| *next_char != '}') {
                            tokens.push(Token::new(TokenKind::NewLine, comment_offset, 1));
                        }
                        break;
                    }
                }
            }
            '{' => tokens.push(Token::new(TokenKind::LeftCurly, offset, 1)),
            '}' => tokens.push(Token::new(TokenKind::RightCurly, offset, 1)),
            '(' => tokens.push(Token::new(TokenKind::LeftParen, offset, 1)),
            ')' => tokens.push(Token::new(TokenKind::RightParen, offset, 1)),
            '[' => tokens.push(Token::new(TokenKind::LeftBracket, offset, 1)),
            ']' => tokens.push(Token::new(TokenKind::RightBracket, offset, 1)),
            '"' | '\'' => {
                let (string, len) = lex_string_literal(src_id, input, offset, char, &mut chars)?;
                tokens.push(Token::new(TokenKind::QuotedString(string), offset, len));
            }
            '\n' => {
                // Only add newline token if not at end of file, and the next char is not a '}'.
                if chars.peek().is_some_and(|(_, next_char)| *next_char != '}') {
                    tokens.push(Token::new(TokenKind::NewLine, offset, 1));
                }
            }
            '\\' => match chars.peek() {
                Some((_, '\n')) => {
                    chars.next();
                    continue;
                }
                Some((_, '\r')) => {
                    chars.next();
                    if chars.peek().is_some_and(|(_, c)| *c == '\n') {
                        chars.next();
                    }
                    continue;
                }
                Some(_) => {
                    let (esc_offset, esc_char) = chars.next().unwrap();
                    let ident_string = String::from(esc_char);
                    let end_offset = esc_offset + esc_char.len_utf8();
                    let has_escapes = true;

                    lex_identifier_rest(
                        &mut tokens,
                        offset,
                        end_offset,
                        ident_string,
                        has_escapes,
                        &mut chars,
                    );
                }
                None => {
                    tokens.push(Token::new(TokenKind::String("\\".to_string()), offset, 1));
                }
            },
            _ => {
                if lex_identifier(&mut tokens, char, offset, &mut chars).is_some() {
                    continue;
                } else if !is_whitespace(&char) {
                    // This is the fallback case so if it is not whitespace, then it is an
                    // unexpected token.
                    return Err(Diagnostic::new(
                        DiagnosticKind::LexError,
                        src_id,
                        offset,
                        offset + char.len_utf8(),
                        format!("Unexpected token '{char}'.").as_str(),
                    ));
                }
            }
        }
    }

    tokens.push(Token::new(TokenKind::EOF, input.len(), 0));

    Ok(tokens)
}

fn lex_string_literal(
    src_id: usize,
    input: &str,
    start_offset: usize,
    delim: char,
    chars: &mut CharIndexedIter,
) -> Result<(String, usize), Diagnostic> {
    let mut string = String::new();
    let mut closed = false;
    let mut end_offset = start_offset + delim.len_utf8();

    while let Some((offset, str_char)) = chars.next() {
        if str_char == delim {
            closed = true;
            end_offset = offset + delim.len_utf8();
            break;
        }

        // Handle string escapes.
        if str_char == '\\' {
            let Some((esc_offset, escaped)) = chars.next() else {
                return Err(Diagnostic::new(
                    DiagnosticKind::LexError,
                    src_id,
                    offset,
                    offset + '\\'.len_utf8(),
                    "Unterminated escape sequence in string literal.",
                ));
            };

            let resolved = match escaped {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                '0' => '\0',
                other => other,
            };

            string.push(resolved);
            end_offset = esc_offset + escaped.len_utf8();
            continue;
        }

        string.push(str_char);
        end_offset = offset + str_char.len_utf8();
    }

    if !closed {
        return Err(Diagnostic::new(
            DiagnosticKind::LexError,
            src_id,
            start_offset,
            input.len(),
            "Unterminated string literal.",
        ));
    }

    Ok((string, end_offset - start_offset))
}

fn is_ident_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            '|' | '&'
                | ';'
                | ','
                | '='
                | '>'
                | '<'
                | '!'
                | '+'
                | '$'
                | '{'
                | '}'
                | '('
                | ')'
                | '['
                | ']'
                | '"'
                | '\''
                | '\\'
        )
}

fn lex_identifier(
    tokens: &mut Vec<Token>,
    start_char: char,
    start_offset: usize,
    chars: &mut CharIndexedIter,
) -> Option<()> {
    if is_ident_char(start_char) {
        let ident_string = String::from(start_char);
        let end_offset = start_offset + start_char.len_utf8();
        let has_escapes = false;

        lex_identifier_rest(
            tokens,
            start_offset,
            end_offset,
            ident_string,
            has_escapes,
            chars,
        );
        return Some(());
    }

    None
}

fn lex_identifier_rest(
    tokens: &mut Vec<Token>,
    start_offset: usize,
    mut end_offset: usize,
    mut ident_string: String,
    mut has_escapes: bool,
    chars: &mut CharIndexedIter,
) {
    while let Some(&(peek_offset, peek_char)) = chars.peek() {
        if peek_char == '\\' {
            chars.next(); // Consume '\\'
            has_escapes = true;
            match chars.peek() {
                Some((_, '\n')) => {
                    chars.next(); // Consume line continuation
                    continue;
                }
                Some((_, '\r')) => {
                    chars.next(); // Consume '\r'
                    if chars.peek().is_some_and(|(_, c)| *c == '\n') {
                        chars.next(); // Consume '\n'
                    }
                    continue;
                }
                Some((esc_offset, esc_char)) => {
                    let esc_char = *esc_char;
                    let esc_offset = *esc_offset;
                    chars.next(); // Consume escaped character
                    ident_string.push(esc_char);
                    end_offset = esc_offset + esc_char.len_utf8();
                    continue;
                }
                None => {
                    ident_string.push('\\');
                    end_offset = peek_offset + 1;
                    break;
                }
            }
        }

        if is_ident_char(peek_char) {
            if peek_char == ':' {
                let (colon_offset, _) = chars.next().expect("Failed to get ':' offset.");
                match chars.peek() {
                    Some((_, next_char))
                        if next_char.is_whitespace() || matches!(next_char, '}' | ',' | '\n') =>
                    {
                        // Stop identifier if the colon is followed by whitespace, EOF, or typical object terminators.
                        // This allows `{ key: value }` to keep `:` as a separate token.
                        push_identifier_or_number(
                            tokens,
                            ident_string,
                            start_offset,
                            end_offset - start_offset,
                            has_escapes,
                        );
                        tokens.push(Token::new(TokenKind::Colon, colon_offset, 1));
                        return;
                    }
                    _ => {
                        ident_string.push(':');
                        end_offset = colon_offset + ':'.len_utf8();
                    }
                }
            } else {
                let (char_offset, c) = chars.next().unwrap();
                ident_string.push(c);
                end_offset = char_offset + c.len_utf8();
            }
        } else {
            break;
        }
    }

    push_identifier_or_number(
        tokens,
        ident_string,
        start_offset,
        end_offset - start_offset,
        has_escapes,
    );
}

fn push_identifier_or_number(
    tokens: &mut Vec<Token>,
    ident_string: String,
    offset: usize,
    len: usize,
    has_escapes: bool,
) {
    if !has_escapes {
        if ident_string == ":" {
            tokens.push(Token::new(TokenKind::Colon, offset, len));
            return;
        }

        if ident_string == "/" {
            tokens.push(Token::new(TokenKind::Slash, offset, len));
            return;
        }

        if let Some(keyword_kind) = match_keyword(ident_string.as_str()) {
            tokens.push(Token::new(keyword_kind, offset, len));
            return;
        }

        let is_numeric = ident_string
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-');
        if is_numeric && let Ok(num) = ident_string.parse::<f64>() {
            tokens.push(Token::new(TokenKind::Number(num), offset, len));
            return;
        }
    }

    tokens.push(Token::new(TokenKind::String(ident_string), offset, len));
}

fn is_whitespace(char: &char) -> bool {
    matches!(char, ' ' | '\n' | '\t')
}

fn match_keyword(keyword: &str) -> Option<TokenKind> {
    match keyword {
        "let" => Some(TokenKind::Let),
        "if" => Some(TokenKind::If),
        "else" => Some(TokenKind::Else),
        "while" => Some(TokenKind::While),
        "fn" => Some(TokenKind::Fn),
        "typeof" => Some(TokenKind::TypeOf),
        "return" => Some(TokenKind::Return),
        "true" => Some(TokenKind::Bool(true)),
        "false" => Some(TokenKind::Bool(false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostics::source_map::SourceMap;

    use super::*;

    fn token_kinds(input: &str) -> Vec<TokenKind> {
        let mut src_map = SourceMap::default();
        let src_id = src_map.insert("<test data>", input);

        let toks = lex_input(src_id, input)
            .expect("lexer should succeed in test")
            .into_iter()
            .map(|token| token.kind)
            .collect();

        src_map.remove(src_id);

        toks
    }

    #[test]
    fn lexes_regular_command_tokens() {
        let kinds = token_kinds("echo hello world");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "hello"));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "world"));
        assert!(matches!(&kinds[3], TokenKind::EOF));
    }

    #[test]
    fn lexes_pipeline_command_tokens() {
        let kinds = token_kinds("cat file.txt | grep txt");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "cat"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "file.txt"));
        assert!(matches!(&kinds[2], TokenKind::Pipe));
        assert!(matches!(&kinds[3], TokenKind::String(s) if s == "grep"));
        assert!(matches!(&kinds[4], TokenKind::String(s) if s == "txt"));
    }

    #[test]
    fn keeps_colons_inside_command_path_tokens() {
        let kinds = token_kinds("cat /sys/bus/pci/devices/0000:64:00.0/power/runtime_status");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "cat"));
        assert!(
            matches!(&kinds[1], TokenKind::String(s) if s == "/sys/bus/pci/devices/0000:64:00.0/power/runtime_status")
        );
        assert!(matches!(&kinds[2], TokenKind::EOF));
    }

    #[test]
    fn keeps_object_key_colon_as_colon_token() {
        let kinds = token_kinds("let x = { key: value }");

        assert!(matches!(&kinds[0], TokenKind::Let));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "x"));
        assert!(matches!(&kinds[2], TokenKind::Equals));
        assert!(matches!(&kinds[3], TokenKind::LeftCurly));
        assert!(matches!(&kinds[4], TokenKind::String(s) if s == "key"));
        assert!(matches!(&kinds[5], TokenKind::Colon));
        assert!(matches!(&kinds[6], TokenKind::String(s) if s == "value"));
        assert!(matches!(&kinds[7], TokenKind::RightCurly));
    }

    #[test]
    fn lexes_command_substitution_tokens() {
        let kinds = token_kinds("echo $(cat /etc/hosts)");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::DollarLeftParen));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "cat"));
        assert!(matches!(&kinds[3], TokenKind::String(s) if s == "/etc/hosts"));
        assert!(matches!(&kinds[4], TokenKind::RightParen));
    }

    #[test]
    fn hash_starts_comment() {
        let kinds = token_kinds("echo hello # this is a comment");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "hello"));
        assert!(matches!(&kinds[2], TokenKind::EOF));
    }

    #[test]
    fn double_slash_is_not_a_comment() {
        let kinds = token_kinds("echo https://example.com");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "https://example.com"));
        assert!(matches!(&kinds[2], TokenKind::EOF));
    }

    #[test]
    fn leading_hash_comment_does_not_lex_comment_text() {
        let kinds = token_kinds("# If this is a comment\necho ok");

        assert!(matches!(&kinds[0], TokenKind::NewLine));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "ok"));
        assert!(matches!(&kinds[3], TokenKind::EOF));
    }

    #[test]
    fn inline_comment_preserves_newline_separator() {
        let kinds = token_kinds("echo a # comment\necho b");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "a"));
        assert!(matches!(&kinds[2], TokenKind::NewLine));
        assert!(matches!(&kinds[3], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[4], TokenKind::String(s) if s == "b"));
        assert!(matches!(&kinds[5], TokenKind::EOF));
    }

    #[test]
    fn keeps_star_inside_word_when_unspaced() {
        let kinds = token_kinds("echo a*b");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "a*b"));
        assert!(matches!(&kinds[2], TokenKind::EOF));
    }

    #[test]
    fn decodes_common_string_escapes() {
        let kinds = token_kinds("echo \"line1\\nline2\\t\\\\\"");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::QuotedString(s) if s == "line1\nline2\t\\"));
        assert!(matches!(&kinds[2], TokenKind::EOF));
    }

    #[test]
    fn decodes_single_quoted_escape_sequences_too() {
        let kinds = token_kinds("echo 'a\\nb\\\\c'");

        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::QuotedString(s) if s == "a\nb\\c"));
        assert!(matches!(&kinds[2], TokenKind::EOF));
    }

    #[test]
    fn lexes_fat_arrow_for_fat_arrow() {
        let kinds = token_kinds("x => x");
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "x"));
        assert!(matches!(&kinds[1], TokenKind::FatArrow));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "x"));
    }

    #[test]
    fn lexes_thin_arrow_for_thin_arrow() {
        let kinds = token_kinds("a -> b");
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "a"));
        assert!(matches!(&kinds[1], TokenKind::ThinArrow));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "b"));
    }

    #[test]
    fn lexes_escaped_spaces_in_bare_words() {
        let mut src_map = SourceMap::default();
        let src_id = src_map.insert("<test>", "cd My\\ Documents");
        let tokens = lex_input(src_id, "cd My\\ Documents").expect("lex succeeds");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[0].kind, TokenKind::String(s) if s == "cd"));
        assert_eq!(tokens[0].offset, 0);
        assert_eq!(tokens[0].len, 2);

        assert!(matches!(&tokens[1].kind, TokenKind::String(s) if s == "My Documents"));
        assert_eq!(tokens[1].offset, 3);
        assert_eq!(tokens[1].len, 13);
        assert!(matches!(&tokens[2].kind, TokenKind::EOF));
    }

    #[test]
    fn lexes_escaped_special_characters_in_bare_words() {
        let kinds = token_kinds("find . -exec echo {} \\;");
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "find"));
        assert!(matches!(&kinds[1], TokenKind::Dot));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "-exec"));
        assert!(matches!(&kinds[3], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[4], TokenKind::LeftCurly));
        assert!(matches!(&kinds[5], TokenKind::RightCurly));
        assert!(matches!(&kinds[6], TokenKind::String(s) if s == ";"));
        assert!(matches!(&kinds[7], TokenKind::EOF));
    }

    #[test]
    fn lexes_escaped_variable_and_comment_and_quotes() {
        let kinds = token_kinds("echo \\$FOO \\#tag \\\"hello\\\" foo\\\\bar");
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "$FOO"));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "#tag"));
        assert!(matches!(&kinds[3], TokenKind::String(s) if s == "\"hello\""));
        assert!(matches!(&kinds[4], TokenKind::String(s) if s == "foo\\bar"));
    }

    #[test]
    fn lexes_escaped_keywords_and_numbers_as_strings() {
        let kinds = token_kinds("\\let \\123 \\true");
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "let"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "123"));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "true"));
    }

    #[test]
    fn lexes_line_continuation() {
        let kinds = token_kinds("echo hello \\\n world");
        assert_eq!(kinds.len(), 4);
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "hello"));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "world"));
        assert!(matches!(&kinds[3], TokenKind::EOF));
    }

    #[test]
    fn lexes_line_continuation_with_crlf() {
        let kinds = token_kinds("echo hello \\\r\n world");
        assert_eq!(kinds.len(), 4);
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "echo"));
        assert!(matches!(&kinds[1], TokenKind::String(s) if s == "hello"));
        assert!(matches!(&kinds[2], TokenKind::String(s) if s == "world"));
        assert!(matches!(&kinds[3], TokenKind::EOF));
    }

    #[test]
    fn lexes_line_continuation_inside_word() {
        let kinds = token_kinds("hel\\\nlo");
        assert_eq!(kinds.len(), 2);
        assert!(matches!(&kinds[0], TokenKind::String(s) if s == "hello"));
        assert!(matches!(&kinds[1], TokenKind::EOF));
    }
}
