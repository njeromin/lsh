use std::char;

use crate::lexer::token::{Token, TokenKind, make_token};

pub mod token;

pub fn lex_input(input: &str) -> Vec<Token> {
    let mut tokens = vec![];

    let mut chars = input.chars().peekable();

    while let Some(char) = chars.next() {
        match char {
            '|' => {
                if chars.peek().is_some_and(|next_char| next_char == &'|') {
                    tokens.push(make_token(TokenKind::Or, 0));
                } else {
                    tokens.push(make_token(TokenKind::Pipe, 0));
                }
            }
            '&' => {
                if chars.peek().is_some_and(|next_char| next_char == &'&') {
                    tokens.push(make_token(TokenKind::And, 0));
                } else {
                    tokens.push(make_token(TokenKind::Ampersand, 0));
                }
            }
            ';' => tokens.push(make_token(TokenKind::ExprSeparator, 0)),
            '=' => {
                if chars.peek().is_some_and(|next_char| next_char == &'=') {
                    tokens.push(make_token(TokenKind::EqualsEquals, 0));
                    chars.next();
                } else {
                    tokens.push(make_token(TokenKind::Equals, 0));
                }
            }
            '>' => {
                if chars.peek().is_some() {
                    tokens.push(make_token(TokenKind::GtEquals, 0));
                    chars.next();
                } else {
                    tokens.push(make_token(TokenKind::GreaterThan, 0));
                }
            }
            '<' => {
                if chars.peek().is_some() {
                    tokens.push(make_token(TokenKind::LtEquals, 0));
                    chars.next();
                } else {
                    tokens.push(make_token(TokenKind::LessThan, 0));
                }
            }
            '!' => {
                if chars.peek().is_some_and(|next_char| next_char == &'=') {
                    tokens.push(make_token(TokenKind::NotEquals, 0));
                } else {
                    tokens.push(make_token(TokenKind::Not, 0));
                }
            }
            '+' => {
                if let Some(next_char) = chars.peek() {
                    match next_char.to_owned() {
                        '=' => tokens.push(make_token(TokenKind::PlusEquals, 0)),
                        ' ' => tokens.push(make_token(TokenKind::Plus, 0)),
                        _ => (),
                    }
                }
            }
            '-' => {
                if let Some(next_char) = chars.peek() {
                    match next_char.to_owned() {
                        '=' => tokens.push(make_token(TokenKind::MinusEquals, 0)),
                        ' ' => tokens.push(make_token(TokenKind::Minus, 0)),
                        _ => (),
                    }
                }
            }
            '*' => {
                if chars.peek().is_some_and(|next_char| next_char == &'=') {
                    tokens.push(make_token(TokenKind::StarEquals, 0));
                } else {
                    tokens.push(make_token(TokenKind::Star, 0));
                }
            }
            '$' => {
                if chars
                    .peek()
                    .is_some_and(|next_char| matches!(next_char, '_' | 'A'..='z'))
                {
                    let mut var_string = String::from(
                        // Using expect here but this should never fail.
                        chars
                            .next()
                            .expect("Failed to get starting character of environment variable."),
                    );

                    while let Some(var_char) = chars.peek() {
                        if matches!(var_char, '_' | 'A'..='z') {
                            // Character is a valid identifier character.

                            var_string.push(var_char.to_owned());
                            // Skip forward one character as only peek was used
                            // previously.
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    tokens.push(make_token(TokenKind::EnvVar(var_string), 0));
                } else {
                    panic!("Unexpected token");
                }
            }
            // TODO: Fix as this could also be part of a string/command
            // '/' => {
            //     if chars.peek().is_some_and(|next_char| next_char == &'=') {
            //         tokens.push(make_token(TokenKind::SlashEquals, 0));
            //     } else {
            //         tokens.push(make_token(TokenKind::Slash, 0));
            //     }
            // }
            '{' => tokens.push(make_token(TokenKind::LeftCurly, 0)),
            '}' => tokens.push(make_token(TokenKind::RightCurly, 0)),
            '(' => tokens.push(make_token(TokenKind::LeftParen, 0)),
            ')' => tokens.push(make_token(TokenKind::RightParen, 0)),
            '"' | '\'' => {
                let mut string = String::new();

                for str_char in chars.by_ref() {
                    if str_char != char {
                        string.push(str_char);
                    } else {
                        break;
                    }
                }

                tokens.push(make_token(TokenKind::String(string), 0));
            }
            '\n' => tokens.push(make_token(TokenKind::ExprSeparator, 0)),
            _ => {
                if matches!(char, '~' | '.' | '/' | '_' | '-' | 'A'..='z') {
                    // Character is a valid identifier starting character.

                    let mut ident_string = String::from(char);

                    while let Some(ident_char) = chars.peek() {
                        if matches!(ident_char, '~' | '.' | '/' | '_' | '-' | 'A'..='z' | '0'..='9')
                        {
                            // Character is a valid identifier character.

                            ident_string.push(ident_char.to_owned());
                            // Skip forward one character as only peek was used
                            // previously.
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    if let Some(keyword_kind) = match_keyword(ident_string.as_str()) {
                        tokens.push(make_token(keyword_kind, 0));
                    } else {
                        tokens.push(make_token(TokenKind::String(ident_string), 0));
                    }
                } else if char.is_ascii_digit() {
                    // Character is a valid number starting character.

                    let mut num_str = String::from(char);
                    let mut has_decimal_separator = false;

                    while let Some(ident_char) = chars.peek() {
                        if ident_char.is_ascii_digit() {
                            // Character is a valid digit.

                            num_str.push(ident_char.to_owned());
                            chars.next(); // Increment chars iter.
                        } else if ident_char == &'.' {
                            if !has_decimal_separator {
                                has_decimal_separator = true;

                                num_str.push('.');
                                chars.next(); // Increment chars iter.
                            } else {
                                panic!("A number cannot have more than one decimal separator.");
                            }
                        } else {
                            break;
                        }
                    }

                    let num: f64 = num_str.parse().expect("Failed to parse number.");

                    tokens.push(make_token(TokenKind::Number(num), 0));
                } else if !is_whitespace(&char) {
                    // This is the fallback case so if it is not whitespace, then it is an
                    // unexpected token.
                    panic!("Unexpected token");
                }
            }
        }
    }

    tokens.push(make_token(TokenKind::EOF, 0));

    tokens
}

fn is_whitespace(char: &char) -> bool {
    matches!(char, ' ' | '\n' | '\t')
}

fn match_keyword(keyword: &str) -> Option<TokenKind> {
    match keyword {
        "let" => Some(TokenKind::Let),
        "if" => Some(TokenKind::If),
        "else" => Some(TokenKind::Else),
        "for" => Some(TokenKind::For),
        "while" => Some(TokenKind::While),
        "fn" => Some(TokenKind::Fn),
        "true" => Some(TokenKind::Bool(true)),
        "false" => Some(TokenKind::Bool(false)),
        _ => None,
    }
}
