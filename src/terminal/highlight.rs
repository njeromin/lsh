use std::borrow::Cow;

use rustyline::highlight::{CmdKind, Highlighter};

use crate::lexer::{
    self,
    token::{Token, TokenKind},
};

const COMMAND_SUB_STYLE: &str = "\x1b[38;5;208m";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Delimiter {
    DollarParen,
    Paren,
    Bracket,
    Brace,
}

#[derive(Default)]
pub struct SyntaxHighlighter;

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self
    }

    fn highlight_line<'a>(&self, line: &'a str) -> Cow<'a, str> {
        let Ok(tokens) = lexer::lex_input(0, line) else {
            return Cow::Borrowed(line);
        };

        if tokens
            .iter()
            .all(|token| matches!(token.kind, TokenKind::EOF))
        {
            return Cow::Borrowed(line);
        }

        let mut highlighted = String::with_capacity(line.len() + tokens.len() * 12);
        let mut cursor = 0;
        let mut delim_stack = Vec::new();

        for (index, token) in tokens.iter().enumerate() {
            if matches!(token.kind, TokenKind::EOF) {
                continue;
            }

            let start = token.offset.min(line.len());
            let end = token_end(line, &tokens, index);

            if start < cursor || start >= end {
                continue;
            }

            let mut is_command_sub_close = false;
            match token.kind {
                TokenKind::DollarLeftParen => delim_stack.push(Delimiter::DollarParen),
                TokenKind::LeftParen => delim_stack.push(Delimiter::Paren),
                TokenKind::LeftBracket => delim_stack.push(Delimiter::Bracket),
                TokenKind::LeftCurly => delim_stack.push(Delimiter::Brace),
                TokenKind::RightBracket => {
                    while let Some(d) = delim_stack.pop() {
                        if d == Delimiter::Bracket {
                            break;
                        }
                    }
                }
                TokenKind::RightCurly => {
                    while let Some(d) = delim_stack.pop() {
                        if d == Delimiter::Brace {
                            break;
                        }
                    }
                }
                TokenKind::RightParen => {
                    while let Some(d) = delim_stack.pop() {
                        match d {
                            Delimiter::DollarParen => {
                                is_command_sub_close = true;
                                break;
                            }
                            Delimiter::Paren => {
                                is_command_sub_close = false;
                                break;
                            }
                            _ => continue,
                        }
                    }
                }
                _ => {}
            }

            highlighted.push_str(&line[cursor..start]);

            let span = &line[start..end];
            if let Some(style) = token_style(token, span, is_command_sub_close) {
                highlighted.push_str(style);
                highlighted.push_str(span);
                highlighted.push_str("\x1b[0m");
            } else {
                highlighted.push_str(span);
            }

            cursor = end;
        }

        highlighted.push_str(&line[cursor..]);
        Cow::Owned(highlighted)
    }
}

impl Highlighter for SyntaxHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        self.highlight_line(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        true
    }
}

fn token_end(line: &str, tokens: &[Token], index: usize) -> usize {
    let start = tokens[index].offset.min(line.len());
    let mut end = tokens
        .iter()
        .skip(index + 1)
        .find(|token| !matches!(token.kind, TokenKind::EOF))
        .map_or(line.len(), |token| token.offset.min(line.len()));

    while end > start {
        let Some(ch) = line[..end].chars().next_back() else {
            break;
        };

        if !ch.is_whitespace() {
            break;
        }

        end -= ch.len_utf8();
    }

    end
}

fn token_style(token: &Token, _span: &str, is_command_sub_close: bool) -> Option<&'static str> {
    if matches!(token.kind, TokenKind::RightParen) && is_command_sub_close {
        return Some(COMMAND_SUB_STYLE);
    }

    match token.kind {
        TokenKind::Let
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::While
        | TokenKind::Fn
        | TokenKind::TypeOf
        | TokenKind::Return => Some("\x1b[1;35m"),
        TokenKind::Variable(_) => Some("\x1b[36m"),
        TokenKind::Number(_) | TokenKind::Bool(_) => Some("\x1b[33m"),
        TokenKind::QuotedString(_) => Some("\x1b[32m"),
        TokenKind::Pipe
        | TokenKind::And
        | TokenKind::Or
        | TokenKind::DollarLeftParen
        | TokenKind::FatArrow
        | TokenKind::ThinArrow => Some(COMMAND_SUB_STYLE),
        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::PlusEquals
        | TokenKind::MinusEquals
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::StarEquals
        | TokenKind::SlashEquals
        | TokenKind::Ampersand
        | TokenKind::Equals
        | TokenKind::EqualsEquals
        | TokenKind::GreaterThan
        | TokenKind::GtEquals
        | TokenKind::LessThan
        | TokenKind::LtEquals
        | TokenKind::Not
        | TokenKind::NotEquals => Some("\x1b[37m"),
        TokenKind::LeftCurly
        | TokenKind::RightCurly
        | TokenKind::LeftParen
        | TokenKind::RightParen
        | TokenKind::LeftBracket
        | TokenKind::RightBracket
        | TokenKind::Comma
        | TokenKind::Dot
        | TokenKind::Semicolon
        | TokenKind::Colon => Some("\x1b[2m"),
        TokenKind::String(_) | TokenKind::NewLine | TokenKind::EOF => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_sub_closing_paren_matches_dollar_left_paren_style() {
        let highlighter = SyntaxHighlighter::new();
        let input = "echo $(greet \"text\")";
        let output = highlighter.highlight(input, 0);

        assert!(output.contains(&format!("{COMMAND_SUB_STYLE}$(\x1b[0m")));
        assert!(output.contains(&format!("{COMMAND_SUB_STYLE})\x1b[0m")));
    }

    #[test]
    fn regular_parens_retain_dim_punctuation_style() {
        let highlighter = SyntaxHighlighter::new();
        let input = "fn greet(who) {}";
        let output = highlighter.highlight(input, 0);

        assert!(output.contains("\x1b[2m(\x1b[0m"));
        assert!(output.contains("\x1b[2m)\x1b[0m"));
        assert!(!output.contains(&format!("{COMMAND_SUB_STYLE})")));
    }

    #[test]
    fn nested_command_substitutions_match_correctly() {
        let highlighter = SyntaxHighlighter::new();
        let input = "echo $(cat $(get-path))";
        let output = highlighter.highlight(input, 0);

        let occurrences = output
            .matches(&format!("{COMMAND_SUB_STYLE})\x1b[0m"))
            .count();
        assert_eq!(occurrences, 2);
    }
}
