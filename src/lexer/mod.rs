use std::{char, iter::Peekable, str::Chars};

use crate::lexer::token::{Token, TokenKind};

pub mod token;

type CharIter<'a> = Peekable<Chars<'a>>;

pub fn lex_input(input: &str) -> Vec<Token> {
    let mut tokens = vec![];

    let mut chars: CharIter = input.chars().peekable();

    // Handle shebang line.
    if chars.next_if(|&char| char == '#').is_some() && chars.next_if(|&char| char == '!').is_some()
    {
        // Go through all characters in shebang line until newline is hit.
        for char in chars.by_ref() {
            if char == '\n' {
                break;
            }
        }
    }

    while let Some(char) = chars.next() {
        match char {
            '|' => {
                if chars.next_if(|next_char| next_char == &'|').is_some() {
                    tokens.push(Token::new(TokenKind::Or, 0));
                } else {
                    tokens.push(Token::new(TokenKind::Pipe, 0));
                }
            }
            '&' => {
                if chars.next_if(|next_char| next_char == &'&').is_some() {
                    tokens.push(Token::new(TokenKind::And, 0));
                } else {
                    tokens.push(Token::new(TokenKind::Ampersand, 0));
                }
            }
            ';' => tokens.push(Token::new(TokenKind::Semicolon, 0)),
            ',' => tokens.push(Token::new(TokenKind::Comma, 0)),
            ':' => tokens.push(Token::new(TokenKind::Colon, 0)),
            '=' => {
                if chars.next_if(|next_char| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::EqualsEquals, 0));
                } else {
                    tokens.push(Token::new(TokenKind::Equals, 0));
                }
            }
            '>' => {
                if chars.next_if(|next_char| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::GtEquals, 0));
                } else {
                    tokens.push(Token::new(TokenKind::GreaterThan, 0));
                }
            }
            '<' => {
                if chars.next_if(|next_char| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::LtEquals, 0));
                } else {
                    tokens.push(Token::new(TokenKind::LessThan, 0));
                }
            }
            '!' => {
                if chars.next_if(|next_char| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::NotEquals, 0));
                } else {
                    tokens.push(Token::new(TokenKind::Not, 0));
                }
            }
            '+' => {
                if let Some(next_char) = chars.peek() {
                    match *next_char {
                        '=' => {
                            tokens.push(Token::new(TokenKind::PlusEquals, 0));
                            chars.next();
                        }
                        ' ' => tokens.push(Token::new(TokenKind::Plus, 0)),
                        _ => {
                            if lex_identifier(&mut tokens, '+', &mut chars).is_none() {
                                tokens.push(Token::new(TokenKind::Plus, 0))
                            }
                        }
                    }
                }
            }
            '-' => match chars.peek() {
                Some(&'=') => {
                    tokens.push(Token::new(TokenKind::MinusEquals, 0));
                    chars.next();
                }
                Some(&' ') => tokens.push(Token::new(TokenKind::Minus, 0)),
                Some(c) if c.is_ascii_digit() => {
                    if lex_identifier(&mut tokens, char, &mut chars).is_none() {
                        tokens.push(Token::new(TokenKind::Minus, 0));
                    }
                }
                Some(_) => {
                    if lex_identifier(&mut tokens, '-', &mut chars).is_none() {
                        tokens.push(Token::new(TokenKind::Minus, 0))
                    }
                }
                None => tokens.push(Token::new(TokenKind::Minus, 0)),
            },
            '*' => {
                if chars.next_if(|next_char| next_char == &'=').is_some() {
                    tokens.push(Token::new(TokenKind::StarEquals, 0));
                } else {
                    tokens.push(Token::new(TokenKind::Star, 0));
                }
            }
            '$' => {
                if let Some(next_char) = chars.peek() {
                    if *next_char == '_' || next_char.is_ascii_alphanumeric() {
                        let mut var_string = String::from(
                            // Using expect here but this should never fail.
                            chars.next().expect(
                                "Failed to get starting character of environment variable.",
                            ),
                        );

                        while let Some(var_char) = chars.peek() {
                            if *var_char == '_' || var_char.is_ascii_alphanumeric() {
                                // Character is a valid identifier character.

                                var_string.push(var_char.to_owned());
                                // Skip forward one character as only peek was used
                                // previously.
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        tokens.push(Token::new(TokenKind::Variable(var_string), 0));
                    } else if next_char == &'(' {
                        tokens.push(Token::new(TokenKind::DollarLeftParen, 0));
                        chars.next();
                    } else {
                        panic!("Unexpected token");
                    }
                } else {
                    panic!("Unexpected token");
                }
            }
            '/' => {
                match chars.peek().copied() {
                    Some('=') => {
                        chars.next();
                        tokens.push(Token::new(TokenKind::SlashEquals, 0))
                    }
                    Some('/') => {
                        // Skip single line comments.
                        while let Some(char) = chars.next() {
                            if char == '\n' {
                                break;
                            }
                        }
                    }
                    Some(_) | None => {
                        if lex_identifier(&mut tokens, '/', &mut chars).is_none() {
                            tokens.push(Token::new(TokenKind::Slash, 0))
                        }
                    }
                }
            }
            '{' => tokens.push(Token::new(TokenKind::LeftCurly, 0)),
            '}' => tokens.push(Token::new(TokenKind::RightCurly, 0)),
            '(' => tokens.push(Token::new(TokenKind::LeftParen, 0)),
            ')' => tokens.push(Token::new(TokenKind::RightParen, 0)),
            '[' => tokens.push(Token::new(TokenKind::LeftBracket, 0)),
            ']' => tokens.push(Token::new(TokenKind::RightBracket, 0)),
            '"' | '\'' => {
                let delim = char;

                let mut string = String::new();

                for str_char in chars.by_ref() {
                    if str_char != delim {
                        string.push(str_char);
                    } else {
                        break;
                    }
                }

                tokens.push(Token::new(TokenKind::String(string), 0));
            }
            '\n' => {
                let is_eof = chars
                    .size_hint()
                    .1
                    .expect("lexer: Failed to retrieve length of input iterator.")
                    == 0;

                // Only add newline token if not at end of file, and the next char is not a '}'.
                if !is_eof && chars.peek().is_none_or(|next_char| *next_char != '}') {
                    tokens.push(Token::new(TokenKind::NewLine, 0))
                }
            }
            _ => {
                if lex_identifier(&mut tokens, char, &mut chars).is_some() {
                    continue;
                } else if !is_whitespace(&char) {
                    // This is the fallback case so if it is not whitespace, then it is an
                    // unexpected token.
                    panic!("Unexpected token {char}.");
                }
            }
        }
    }

    tokens.push(Token::new(TokenKind::EOF, 0));

    tokens
}

fn lex_identifier(tokens: &mut Vec<Token>, start_char: char, chars: &mut CharIter) -> Option<()> {
    let is_ident_char = |c: char| -> bool {
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
                    | '*'
                    | '$'
                    | '{'
                    | '}'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '"'
                    | '\''
            )
    };

    if is_ident_char(start_char) {
        let mut ident_string = String::from(start_char);

        while let Some(&ident_char) = chars.peek() {
            if is_ident_char(ident_char) {
                if ident_char == ':' {
                    chars.next();
                    match chars.peek() {
                        Some(&next_char)
                            if next_char.is_whitespace()
                                || matches!(next_char, '}' | ',' | '\n') =>
                        {
                            // Stop identifier if the colon is followed by whitespace, EOF, or typical object terminators.
                            // This allows `{ key: value }` to keep `:` as a separate token.
                            push_identifier_or_number(tokens, ident_string);
                            tokens.push(Token::new(TokenKind::Colon, 0));
                            return Some(());
                        }
                        _ => {
                            ident_string.push(':');
                        }
                    }
                } else {
                    ident_string.push(ident_char);
                    chars.next();
                }
            } else {
                break;
            }
        }

        push_identifier_or_number(tokens, ident_string);
        return Some(());
    }

    None
}

fn push_identifier_or_number(tokens: &mut Vec<Token>, ident_string: String) {
    if ident_string == ":" {
        tokens.push(Token::new(TokenKind::Colon, 0));
        return;
    }

    if let Some(keyword_kind) = match_keyword(ident_string.as_str()) {
        tokens.push(Token::new(keyword_kind, 0));
        return;
    }

    let is_numeric = ident_string
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-');
    if is_numeric {
        if let Ok(num) = ident_string.parse::<f64>() {
            tokens.push(Token::new(TokenKind::Number(num), 0));
            return;
        }
    }

    tokens.push(Token::new(TokenKind::String(ident_string), 0));
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
    use super::*;

    fn token_kinds(input: &str) -> Vec<TokenKind> {
        lex_input(input)
            .into_iter()
            .map(|token| token.kind)
            .collect()
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
}
