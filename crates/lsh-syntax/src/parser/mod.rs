use crate::{
    diagnostics::{Diagnostic, DiagnosticKind},
    lexer::token::{Token, TokenKind},
    parser::ast::Program,
};

pub mod ast;
mod command;
mod expr;
mod statement;
mod string;

pub struct Parser {
    pub(crate) src_id: usize,
    pub(crate) tokens: Vec<Token>,
    pub(crate) index: usize,
}

impl Parser {
    pub fn new(src_id: usize, tokens: Vec<Token>) -> Self {
        Self {
            src_id,
            tokens,
            index: 0,
        }
    }

    /// Parses all provided tokens and returns a program which can be evaluated by an Evaluator.
    pub fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        self.parse_program_unchecked()
    }

    pub(crate) fn skip_statement_separators(&mut self) {
        while self.check_current(&TokenKind::NewLine)
            || self.check_current(&TokenKind::Semicolon)
            || self.check_current(&TokenKind::Ampersand)
        {
            self.advance();
        }
    }

    pub(crate) fn current(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("Failed to get current token."))
    }

    pub(crate) fn peek_token(&self, distance: usize) -> &Token {
        self.tokens
            .get(self.index + distance)
            .unwrap_or_else(|| self.tokens.last().expect("Failed to get last token."))
    }

    pub(crate) fn advance(&mut self) -> &Token {
        // Only index if it is possible.
        if self.index < self.tokens.len() {
            self.index += 1;
        }

        self.current()
    }

    pub(crate) fn check_current(&self, kind_y: &TokenKind) -> bool {
        self.current().kind.compare(kind_y)
    }

    pub(crate) fn check_peek(&self, distance: usize, kind_y: &TokenKind) -> bool {
        self.peek_token(distance).kind.compare(kind_y)
    }

    pub(crate) fn match_token(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check_current(kind) {
                self.advance();
                return true;
            }
        }

        false
    }

    pub(crate) fn consume(&mut self, kind: &TokenKind, err_msg: &str) -> Result<Token, Diagnostic> {
        if self.check_current(kind) {
            return Ok(self.advance().to_owned());
        }

        let expected = Self::token_kind_label(kind);
        let found = self.current_token_label();
        let msg = format!("{err_msg} Found {found}, expected {expected}.");
        self.error_here(msg)
    }

    pub(crate) fn token_kind_label(kind: &TokenKind) -> String {
        match kind {
            TokenKind::String(s) => format!("'{s}'"),
            TokenKind::QuotedString(s) => format!("'{s}'"),
            TokenKind::DoubleQuotedString(s) => format!("\"{s}\""),
            TokenKind::Variable(v) => format!("'${v}'"),
            TokenKind::Number(n) => format!("'{n}'"),
            TokenKind::Bool(b) => format!("'{b}'"),
            TokenKind::Let => "'let'".to_string(),
            TokenKind::If => "'if'".to_string(),
            TokenKind::Else => "'else'".to_string(),
            TokenKind::While => "'while'".to_string(),
            TokenKind::For => "'for'".to_string(),
            TokenKind::In => "'in'".to_string(),
            TokenKind::Break => "'break'".to_string(),
            TokenKind::Continue => "'continue'".to_string(),
            TokenKind::Fn => "'fn'".to_string(),
            TokenKind::TypeOf => "'typeof'".to_string(),
            TokenKind::Return => "'return'".to_string(),
            TokenKind::Pipe => "'|'".to_string(),
            TokenKind::And => "'&&'".to_string(),
            TokenKind::Or => "'||'".to_string(),
            TokenKind::Plus => "'+'".to_string(),
            TokenKind::Minus => "'-'".to_string(),
            TokenKind::Star => "'*'".to_string(),
            TokenKind::Slash => "'/'".to_string(),
            TokenKind::Equals => "'='".to_string(),
            TokenKind::EqualsEquals => "'=='".to_string(),
            TokenKind::Not => "'!'".to_string(),
            TokenKind::NotEquals => "'!='".to_string(),
            TokenKind::LessThan => "'<'".to_string(),
            TokenKind::LtEquals => "'<='".to_string(),
            TokenKind::GreaterThan => "'>'".to_string(),
            TokenKind::GtEquals => "'>='".to_string(),
            TokenKind::PlusEquals => "'+='".to_string(),
            TokenKind::MinusEquals => "'-='".to_string(),
            TokenKind::StarEquals => "'*='".to_string(),
            TokenKind::SlashEquals => "'/='".to_string(),
            TokenKind::FatArrow => "'=>'".to_string(),
            TokenKind::ThinArrow => "'->'".to_string(),
            TokenKind::DollarLeftParen => "'$('".to_string(),
            TokenKind::LeftCurly => "'{'".to_string(),
            TokenKind::RightCurly => "'}'".to_string(),
            TokenKind::LeftParen => "'('".to_string(),
            TokenKind::RightParen => "')'".to_string(),
            TokenKind::LeftBracket => "'['".to_string(),
            TokenKind::RightBracket => "']'".to_string(),
            TokenKind::Comma => "','".to_string(),
            TokenKind::Dot => "'.'".to_string(),
            TokenKind::Semicolon => "';'".to_string(),
            TokenKind::NewLine => "newline".to_string(),
            TokenKind::EOF => "end of input".to_string(),
            TokenKind::Ampersand => "'&'".to_string(),
        }
    }

    pub(crate) fn current_token_label(&self) -> String {
        Self::token_kind_label(&self.current().kind)
    }

    pub(crate) fn diagnostic_here(&self, message: impl Into<String>) -> Diagnostic {
        let token = self.current();
        let start = token.offset;
        let end = start + Self::token_source_len(token).max(1);

        Diagnostic::new(
            DiagnosticKind::ParseError,
            self.src_id,
            start,
            end,
            message.into().as_str(),
        )
    }

    pub(crate) fn diagnostic_here_with_hint(
        &self,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Diagnostic {
        let mut diag = self.diagnostic_here(message);
        diag.hint = Some(hint.into());
        diag
    }

    pub(crate) fn error_here<T>(&self, message: impl Into<String>) -> Result<T, Diagnostic> {
        Err(self.diagnostic_here(message))
    }

    pub(crate) fn error_here_with_hint<T>(
        &self,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Result<T, Diagnostic> {
        Err(self.diagnostic_here_with_hint(message, hint))
    }

    pub(crate) fn token_source_len(token: &Token) -> usize {
        token.len
    }
}

#[cfg(test)]
mod tests;
