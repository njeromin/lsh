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
        write!(f, "TokenKind::{self:?}")
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
