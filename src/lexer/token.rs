use std::fmt;

pub enum TokenKind {
    String,
    Number,
    Bool,
    Pipe,
    And,
    Ampersand,
    Semicolon,
    Equals,
    DoubleEquals,
    GreaterThan,
    GtEquals,
    LessThan,
    LtEquals,
    If,
    LeftCurly,
    RightCurly,
    LeftParen,
    RightParen,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TokenKind::{}",
            match self {
                Self::String => "String",
                Self::Number => "Number",
                Self::Bool => "Bool",
                Self::Pipe => "Pipe",
                Self::And => "And",
                Self::Ampersand => "Ampersand",
                Self::Semicolon => "Semicolon",
                Self::Equals => "Equals",
                Self::DoubleEquals => "DoubleEquals",
                Self::GreaterThan => "GreaterThan",
                Self::GtEquals => "GtEquals",
                Self::LessThan => "LessThan",
                Self::LtEquals => "LtEquals",
                Self::If => "If",
                Self::LeftCurly => "LeftCurly",
                Self::RightCurly => "RightCurly",
                Self::LeftParen => "LeftParen",
                Self::RightParen => "RightParen",
            }
        )
    }
}

pub struct Token {
    pub kind: TokenKind,
    pub input: String,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Token{{ kind: {}, input: {} }}", self.kind, self.input)
    }
}

pub fn make_token(kind: TokenKind, input: String) -> Token {
    Token { kind, input }
}
