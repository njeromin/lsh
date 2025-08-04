use std::fmt;

#[derive(Debug, Clone)]
pub enum TokenKind {
    String(String),
    Number(f64),
    Bool(bool),
    Plus,
    Minus,
    PlusEquals,
    MinusEquals,
    Star,
    Slash,
    StarEquals,
    SlashEquals,
    Pipe,
    Ampersand,
    And,
    Or,
    Semicolon,
    Equals,
    EqualsEquals,
    GreaterThan,
    GtEquals,
    LessThan,
    LtEquals,
    Not,
    NotEquals,
    Let,
    If,
    Else,
    For,
    While,
    Fn,
    LeftCurly,
    RightCurly,
    LeftParen,
    RightParen,
    EOF,
}

// NOTE: This is pretty ugly with how unreachable is used.
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TokenKind::{}",
            match self {
                Self::String(str) => format!("String({str})"),
                Self::Number(num) => format!("Number({num})"),
                Self::Bool(bool) => format!("Bool({bool})"),
                _ => match self {
                    Self::Pipe => "Pipe",
                    Self::And => "And",
                    Self::Ampersand => "Ampersand",
                    Self::Semicolon => "Semicolon",
                    Self::Equals => "Equals",
                    Self::EqualsEquals => "EqualsEquals",
                    Self::GreaterThan => "GreaterThan",
                    Self::GtEquals => "GtEquals",
                    Self::LessThan => "LessThan",
                    Self::LtEquals => "LtEquals",
                    Self::Not => "Not",
                    Self::NotEquals => "NotEquals",
                    Self::Let => "Let",
                    Self::If => "If",
                    Self::Else => "Else",
                    Self::For => "For",
                    Self::While => "While",
                    Self::Fn => "Fn",
                    Self::LeftCurly => "LeftCurly",
                    Self::RightCurly => "RightCurly",
                    Self::LeftParen => "LeftParen",
                    Self::RightParen => "RightParen",
                    Self::EOF => "EOF",
                    _ => unreachable!(),
                }
                .to_string(),
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub offset: usize,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Token{{ kind: {}, offset: {} }}", self.kind, self.offset)
    }
}

pub fn make_token(kind: TokenKind, offset: usize) -> Token {
    Token { kind, offset }
}
