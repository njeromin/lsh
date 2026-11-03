use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};

use lsh_syntax::lexer::{
    self,
    token::{Token, TokenKind},
};

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

    pub fn highlight_line(&self, line: &str) -> StyledText {
        let mut styled = StyledText::new();
        let Ok(tokens) = lexer::lex_input(0, line) else {
            styled.push((Style::default(), line.to_string()));
            return styled;
        };

        if tokens
            .iter()
            .all(|token| matches!(token.kind, TokenKind::EOF))
        {
            styled.push((Style::default(), line.to_string()));
            return styled;
        }

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

            if start > cursor {
                styled.push((Style::default(), line[cursor..start].to_string()));
            }

            let span = &line[start..end];
            let style = token_style(token, span, is_command_sub_close).unwrap_or_default();
            styled.push((style, span.to_string()));

            cursor = end;
        }

        if cursor < line.len() {
            styled.push((Style::default(), line[cursor..].to_string()));
        }

        styled
    }
}

impl Highlighter for SyntaxHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        self.highlight_line(line)
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

fn token_style(token: &Token, _span: &str, is_command_sub_close: bool) -> Option<Style> {
    if matches!(token.kind, TokenKind::RightParen) && is_command_sub_close {
        return Some(Style::new().fg(Color::Fixed(208)));
    }

    match token.kind {
        TokenKind::Let
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::While
        | TokenKind::For
        | TokenKind::In
        | TokenKind::Break
        | TokenKind::Continue
        | TokenKind::Fn
        | TokenKind::TypeOf
        | TokenKind::Return => Some(Style::new().bold().fg(Color::Magenta)),
        TokenKind::Variable(_) => Some(Style::new().fg(Color::Cyan)),
        TokenKind::Number(_) | TokenKind::Bool(_) => Some(Style::new().fg(Color::Yellow)),
        TokenKind::QuotedString(_) | TokenKind::DoubleQuotedString(_) => {
            Some(Style::new().fg(Color::Green))
        }
        TokenKind::Pipe
        | TokenKind::And
        | TokenKind::Or
        | TokenKind::DollarLeftParen
        | TokenKind::FatArrow
        | TokenKind::ThinArrow => Some(Style::new().fg(Color::Fixed(208))),
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
        | TokenKind::NotEquals => Some(Style::new().fg(Color::White)),
        TokenKind::LeftCurly
        | TokenKind::RightCurly
        | TokenKind::LeftParen
        | TokenKind::RightParen
        | TokenKind::LeftBracket
        | TokenKind::RightBracket
        | TokenKind::Comma
        | TokenKind::Dot
        | TokenKind::Semicolon => Some(Style::new().dimmed()),
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

        let sub_style = Style::new().fg(Color::Fixed(208));
        assert!(output.buffer.contains(&(sub_style, "$(".to_string())));
        assert!(output.buffer.contains(&(sub_style, ")".to_string())));
    }

    #[test]
    fn regular_parens_retain_dim_punctuation_style() {
        let highlighter = SyntaxHighlighter::new();
        let input = "fn greet(who) {}";
        let output = highlighter.highlight(input, 0);

        let dim_style = Style::new().dimmed();
        let sub_style = Style::new().fg(Color::Fixed(208));
        assert!(output.buffer.contains(&(dim_style, "(".to_string())));
        assert!(output.buffer.contains(&(dim_style, ")".to_string())));
        assert!(!output.buffer.contains(&(sub_style, ")".to_string())));
    }

    #[test]
    fn nested_command_substitutions_match_correctly() {
        let highlighter = SyntaxHighlighter::new();
        let input = "echo $(cat $(get-path))";
        let output = highlighter.highlight(input, 0);

        let sub_style = Style::new().fg(Color::Fixed(208));
        let occurrences = output
            .buffer
            .iter()
            .filter(|(s, t)| *s == sub_style && t == ")")
            .count();
        assert_eq!(occurrences, 2);
    }
}
