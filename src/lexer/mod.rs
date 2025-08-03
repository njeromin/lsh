use std::char;

use crate::lexer::token::{Token, TokenKind, make_token};

pub mod token;

pub fn lex_input(input: &str) -> Vec<Token> {
    let mut tokens = vec![];

    let mut chars = input.chars().peekable();

    while let Some(char) = chars.next() {
        match char {
            '|' => tokens.push(make_token(TokenKind::Pipe, "|".to_string())),
            '&' => {
                if chars.next().is_some() {
                    tokens.push(make_token(TokenKind::And, "&&".to_string()));
                } else {
                    tokens.push(make_token(TokenKind::Ampersand, "&".to_string()));
                }
            }
            ';' => tokens.push(make_token(TokenKind::Semicolon, ';'.to_string())),
            '=' => {
                if chars.peek().is_some() {
                    tokens.push(make_token(TokenKind::DoubleEquals, "==".to_string()));
                    chars.next();
                } else {
                    tokens.push(make_token(TokenKind::Equals, "=".to_string()));
                }
            }
            '{' => tokens.push(make_token(TokenKind::LeftCurly, "{".to_string())),
            '}' => tokens.push(make_token(TokenKind::RightCurly, "}".to_string())),
            '(' => tokens.push(make_token(TokenKind::LeftParen, "{".to_string())),
            ')' => tokens.push(make_token(TokenKind::RightParen, "}".to_string())),
            '"' => {
                // NOTE: This is duplicated for ', pretty stupid fix eventually.
                let mut string = String::new();

                while let Some(str_char) = chars.next() {
                    if str_char != '"' {
                        string.push(str_char);
                    } else {
                        break;
                    }
                }

                tokens.push(make_token(TokenKind::String, string));
            }
            '\'' => {
                let mut string = String::new();

                while let Some(str_char) = chars.next() {
                    if str_char != '\'' {
                        string.push(str_char);
                    } else {
                        break;
                    }
                }

                tokens.push(make_token(TokenKind::String, string));
            }
            _ => {
                if matches!(char, '_' | '-' | 'A'..='z') {
                    // Character is a valid identifier starting character.

                    let mut ident_string = String::from(char);

                    while let Some(ident_char) = chars.peek() {
                        if matches!(ident_char, '_' | '-' | 'A'..='z' | '0'..='9') {
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
                        tokens.push(make_token(keyword_kind, ident_string));
                    } else {
                        tokens.push(make_token(TokenKind::String, ident_string));
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

                    tokens.push(make_token(TokenKind::Number, num_str));
                } else if !is_whitespace(&char) {
                    // This is the fallback case so if it is not whitespace, then it is an
                    // unexpected token.
                    panic!("Unexpected token");
                }
            }
        }
    }

    tokens
}

fn is_whitespace(char: &char) -> bool {
    matches!(char, ' ' | '\n' | '\t')
}

fn match_keyword(keyword: &str) -> Option<TokenKind> {
    match keyword {
        "if" => Some(TokenKind::If),
        _ => None,
    }
}
