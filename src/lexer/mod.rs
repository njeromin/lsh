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
            '-' => {
                if let Some(next_char) = chars.peek() {
                    match *next_char {
                        '=' => {
                            tokens.push(Token::new(TokenKind::MinusEquals, 0));
                            chars.next();
                        }
                        ' ' => tokens.push(Token::new(TokenKind::Minus, 0)),
                        _ => {
                            if lex_number(&mut tokens, char, &mut chars).is_none()
                                && lex_identifier(&mut tokens, '-', &mut chars).is_none()
                            {
                                tokens.push(Token::new(TokenKind::Minus, 0))
                            }
                        }
                    }
                }
            }
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
                if let Some(next_char) = chars.peek() {
                    match *next_char {
                        '=' => {
                            chars.next();
                            tokens.push(Token::new(TokenKind::SlashEquals, 0))
                        }
                        '/' => {
                            // Skip single line comments.
                            while let Some(char) = chars.next() {
                                if char == '\n' {
                                    break;
                                }
                            }
                        }
                        _ => {
                            if lex_identifier(&mut tokens, '/', &mut chars).is_none() {
                                tokens.push(Token::new(TokenKind::Slash, 0))
                            }
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
                if lex_number(&mut tokens, char, &mut chars).is_some()
                    || lex_identifier(&mut tokens, char, &mut chars).is_some()
                {
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
    // First, check if starting character is a valid for the start of a string.
    if matches!(start_char, '~' | '.' | '/' | '_' | '-') || start_char.is_ascii_alphabetic() {
        let mut ident_string = String::from(start_char);

        while let Some(ident_char) = chars.peek() {
            if matches!(ident_char, '~' | '.' | '/' | '_' | '-')
                || ident_char.is_ascii_alphanumeric()
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
            tokens.push(Token::new(keyword_kind, 0));
        } else {
            tokens.push(Token::new(TokenKind::String(ident_string), 0));
        }

        return Some(());
    }

    None
}

fn lex_number(tokens: &mut Vec<Token>, start_char: char, chars: &mut CharIter) -> Option<()> {
    if start_char.is_ascii_digit() || start_char == '-' {
        // Character is a valid number starting character.

        let mut num_str = String::from(start_char);
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

        tokens.push(Token::new(TokenKind::Number(num), 0));

        return Some(());
    }

    None
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
        "typeof" => Some(TokenKind::Typeof),
        "true" => Some(TokenKind::Bool(true)),
        "false" => Some(TokenKind::Bool(false)),
        _ => None,
    }
}
