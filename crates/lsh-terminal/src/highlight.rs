use crossterm::style::{Color, ContentStyle, Stylize};
use lsh_input::{StyledText, TextFormatter};

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

    pub fn highlight_styled(&self, line: &str) -> StyledText {
        let mut styled = StyledText::new();
        let Ok(tokens) = lexer::lex_input(0, line) else {
            styled.push(ContentStyle::default(), line);
            return styled;
        };

        if tokens
            .iter()
            .all(|token| matches!(token.kind, TokenKind::EOF))
        {
            styled.push(ContentStyle::default(), line);
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
                styled.push(ContentStyle::default(), &line[cursor..start]);
            }

            let span = &line[start..end];
            let style = token_content_style(token, span, is_command_sub_close).unwrap_or_default();
            styled.push(style, span);

            cursor = end;
        }

        if cursor < line.len() {
            styled.push(ContentStyle::default(), &line[cursor..]);
        }

        styled
    }
}

impl TextFormatter for SyntaxHighlighter {
    fn format(&self, line: &str) -> StyledText {
        self.highlight_styled(line)
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

fn token_content_style(
    token: &Token,
    _span: &str,
    is_command_sub_close: bool,
) -> Option<ContentStyle> {
    if matches!(token.kind, TokenKind::RightParen) && is_command_sub_close {
        return Some(ContentStyle::new().with(Color::AnsiValue(208)));
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
        | TokenKind::Return => Some(ContentStyle::new().magenta().bold()),
        TokenKind::Variable(_) => Some(ContentStyle::new().cyan()),
        TokenKind::Number(_) | TokenKind::Bool(_) => Some(ContentStyle::new().yellow()),
        TokenKind::QuotedString(_) | TokenKind::DoubleQuotedString(_) => {
            Some(ContentStyle::new().green())
        }
        TokenKind::Pipe
        | TokenKind::And
        | TokenKind::Or
        | TokenKind::DollarLeftParen
        | TokenKind::FatArrow
        | TokenKind::ThinArrow => Some(ContentStyle::new().with(Color::AnsiValue(208))),
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
        | TokenKind::NotEquals => Some(ContentStyle::new().white()),
        TokenKind::LeftCurly
        | TokenKind::RightCurly
        | TokenKind::LeftParen
        | TokenKind::RightParen
        | TokenKind::LeftBracket
        | TokenKind::RightBracket
        | TokenKind::Comma
        | TokenKind::Dot
        | TokenKind::Semicolon => Some(ContentStyle::new().dim()),
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
        let output = highlighter.highlight_styled(input);

        let sub_style = ContentStyle::new().with(Color::AnsiValue(208));
        assert!(output.spans().contains(&(sub_style, "$(".to_string())));
        assert!(output.spans().contains(&(sub_style, ")".to_string())));
    }

    #[test]
    fn regular_parens_retain_dim_punctuation_style() {
        let highlighter = SyntaxHighlighter::new();
        let input = "fn greet(who) {}";
        let output = highlighter.highlight_styled(input);

        let dim_style = ContentStyle::new().dim();
        let sub_style = ContentStyle::new().with(Color::AnsiValue(208));
        assert!(output.spans().contains(&(dim_style, "(".to_string())));
        assert!(output.spans().contains(&(dim_style, ")".to_string())));
        assert!(!output.spans().contains(&(sub_style, ")".to_string())));
    }

    #[test]
    fn nested_command_substitutions_match_correctly() {
        let highlighter = SyntaxHighlighter::new();
        let input = "echo $(cat $(get-path))";
        let output = highlighter.highlight_styled(input);

        let sub_style = ContentStyle::new().with(Color::AnsiValue(208));
        let occurrences = output
            .spans()
            .iter()
            .filter(|(s, t)| *s == sub_style && t == ")")
            .count();
        assert_eq!(occurrences, 2);
    }
}
