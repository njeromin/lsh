use std::fmt;

#[derive(Debug, Clone)]
pub enum TokenKind {
    Variable(String),
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
    Semicolon,
    EOF,
}

// NOTE: This is pretty ugly with how unreachable is used.
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TokenKind::{}",
            match self {
                Self::Variable(var) => format!("Variable({var})"),
                Self::String(str) => format!("String({str})"),
                Self::Number(num) => format!("Number({num})"),
                Self::Bool(bool) => format!("Bool({bool})"),
                Self::Plus => "Plus".to_string(),
                Self::Minus => "Minus".to_string(),
                Self::PlusEquals => "PlusEquals".to_string(),
                Self::MinusEquals => "MinusEquals".to_string(),
                Self::Star => "Star".to_string(),
                Self::Slash => "Slash".to_string(),
                Self::StarEquals => "StarEquals".to_string(),
                Self::SlashEquals => "SlashEquals".to_string(),
                Self::Pipe => "Pipe".to_string(),
                Self::Ampersand => "Ampersand".to_string(),
                Self::And => "And".to_string(),
                Self::Or => "Or".to_string(),
                Self::Equals => "Equals".to_string(),
                Self::EqualsEquals => "EqualsEquals".to_string(),
                Self::GreaterThan => "GreaterThan".to_string(),
                Self::GtEquals => "GtEquals".to_string(),
                Self::LessThan => "LessThan".to_string(),
                Self::LtEquals => "LtEquals".to_string(),
                Self::Not => "Not".to_string(),
                Self::NotEquals => "NotEquals".to_string(),
                Self::Let => "Let".to_string(),
                Self::If => "If".to_string(),
                Self::Else => "Else".to_string(),
                Self::For => "For".to_string(),
                Self::While => "While".to_string(),
                Self::Fn => "Fn".to_string(),
                Self::LeftCurly => "LeftCurly".to_string(),
                Self::RightCurly => "RightCurly".to_string(),
                Self::LeftParen => "LeftParen".to_string(),
                Self::RightParen => "RightParen".to_string(),
                Self::Semicolon => "Semicolon".to_string(),
                Self::EOF => "EOF".to_string(),
            },
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

impl Token {
    pub fn new(kind: TokenKind, offset: usize) -> Self {
        Token { kind, offset }
    }
}
