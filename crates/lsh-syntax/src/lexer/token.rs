use std::{borrow::Cow, fmt, mem};

#[derive(Debug, Clone)]
pub enum TokenKind<'a> {
    Variable(Cow<'a, str>),
    String(Cow<'a, str>),
    QuotedString(Cow<'a, str>),
    DoubleQuotedString(Cow<'a, str>),
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
    Equals,
    EqualsEquals,
    GreaterThan,
    GtEquals,
    LessThan,
    LtEquals,
    Not,
    NotEquals,
    DollarLeftParen,
    Let,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Fn,
    TypeOf,
    Return,
    FatArrow,
    ThinArrow,
    ThinArrowAppend,
    LeftCurly,
    RightCurly,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Semicolon,
    NewLine,
    EOF,
}

impl fmt::Display for TokenKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Variable(var) => write!(f, "TokenKind::Variable({var})"),
            Self::String(str) => write!(f, "TokenKind::String({str})"),
            Self::QuotedString(str) => write!(f, "TokenKind::QuotedString({str})"),
            Self::DoubleQuotedString(str) => write!(f, "TokenKind::DoubleQuotedString({str})"),
            Self::Number(num) => write!(f, "TokenKind::Number({num})"),
            Self::Bool(bool) => write!(f, "TokenKind::Bool({bool})"),
            other => {
                let name = match other {
                    Self::Plus => "Plus",
                    Self::Minus => "Minus",
                    Self::PlusEquals => "PlusEquals",
                    Self::MinusEquals => "MinusEquals",
                    Self::Star => "Star",
                    Self::Slash => "Slash",
                    Self::StarEquals => "StarEquals",
                    Self::SlashEquals => "SlashEquals",
                    Self::Pipe => "Pipe",
                    Self::Ampersand => "Ampersand",
                    Self::And => "And",
                    Self::Or => "Or",
                    Self::Equals => "Equals",
                    Self::EqualsEquals => "EqualsEquals",
                    Self::GreaterThan => "GreaterThan",
                    Self::GtEquals => "GtEquals",
                    Self::LessThan => "LessThan",
                    Self::LtEquals => "LtEquals",
                    Self::Not => "Not",
                    Self::NotEquals => "NotEquals",
                    Self::DollarLeftParen => "DollarLeftParen",
                    Self::Let => "Let",
                    Self::If => "If",
                    Self::Else => "Else",
                    Self::While => "While",
                    Self::For => "For",
                    Self::In => "In",
                    Self::Break => "Break",
                    Self::Continue => "Continue",
                    Self::Fn => "Fn",
                    Self::TypeOf => "TypeOf",
                    Self::Return => "Return",
                    Self::FatArrow => "FatArrow",
                    Self::ThinArrow => "ThinArrow",
                    Self::ThinArrowAppend => "ThinArrowAppend",
                    Self::LeftCurly => "LeftCurly",
                    Self::RightCurly => "RightCurly",
                    Self::LeftParen => "LeftParen",
                    Self::RightParen => "RightParen",
                    Self::LeftBracket => "LeftBracket",
                    Self::RightBracket => "RightBracket",
                    Self::Comma => "Comma",
                    Self::Dot => "Dot",
                    Self::Semicolon => "Semicolon",
                    Self::NewLine => "NewLine",
                    Self::EOF => "EOF",
                    Self::Variable(_)
                    | Self::String(_)
                    | Self::QuotedString(_)
                    | Self::DoubleQuotedString(_)
                    | Self::Number(_)
                    | Self::Bool(_) => unreachable!(),
                };
                write!(f, "TokenKind::{name}")
            }
        }
    }
}

impl TokenKind<'_> {
    /// Compares two TokenKind instances for equality based on their discriminants.
    pub fn compare(&self, other: &TokenKind<'_>) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub offset: usize,
    pub len: usize,
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Token{{ kind: {}, offset: {}, len: {} }}",
            self.kind, self.offset, self.len
        )
    }
}

impl<'a> Token<'a> {
    pub fn new(kind: TokenKind<'a>, offset: usize, len: usize) -> Self {
        Token { kind, offset, len }
    }
}
