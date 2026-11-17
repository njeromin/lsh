use std::borrow::Cow;

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

pub struct Parser<'a> {
    pub(crate) tokens: Vec<Token<'a>>,
    pub(crate) index: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens, index: 0 }
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

    pub(crate) fn current(&self) -> &Token<'a> {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("Failed to get current token."))
    }

    pub(crate) fn peek_token(&self, distance: usize) -> &Token<'a> {
        self.tokens
            .get(self.index + distance)
            .unwrap_or_else(|| self.tokens.last().expect("Failed to get last token."))
    }

    pub(crate) fn advance(&mut self) -> &Token<'a> {
        // Only index if it is possible.
        if self.index < self.tokens.len() {
            self.index += 1;
        }

        self.current()
    }

    pub(crate) fn check_current(&self, kind_y: &TokenKind<'_>) -> bool {
        self.current().kind.compare(kind_y)
    }

    pub(crate) fn check_peek(&self, distance: usize, kind_y: &TokenKind<'_>) -> bool {
        self.peek_token(distance).kind.compare(kind_y)
    }

    pub(crate) fn match_token(&mut self, kinds: &[TokenKind<'_>]) -> bool {
        for kind in kinds {
            if self.check_current(kind) {
                self.advance();
                return true;
            }
        }

        false
    }

    pub(crate) fn consume(
        &mut self,
        kind: &TokenKind<'_>,
        err_msg: &str,
    ) -> Result<&Token<'a>, Diagnostic> {
        if self.check_current(kind) {
            return Ok(self.advance());
        }

        let expected = Self::token_kind_label(kind);
        let found = self.current_token_label();
        let msg = format!("{err_msg} Found {found}, expected {expected}.");
        self.error_here(msg)
    }

    pub(crate) fn token_kind_label(kind: &TokenKind<'_>) -> Cow<'static, str> {
        match kind {
            TokenKind::String(s) => Cow::Owned(format!("'{s}'")),
            TokenKind::QuotedString(s) => Cow::Owned(format!("'{s}'")),
            TokenKind::DoubleQuotedString(s) => Cow::Owned(format!("\"{s}\"")),
            TokenKind::Variable(v) => Cow::Owned(format!("'${v}'")),
            TokenKind::Number(n) => Cow::Owned(format!("'{n}'")),
            TokenKind::Bool(b) => Cow::Owned(format!("'{b}'")),
            TokenKind::Let => Cow::Borrowed("'let'"),
            TokenKind::If => Cow::Borrowed("'if'"),
            TokenKind::Else => Cow::Borrowed("'else'"),
            TokenKind::While => Cow::Borrowed("'while'"),
            TokenKind::For => Cow::Borrowed("'for'"),
            TokenKind::In => Cow::Borrowed("'in'"),
            TokenKind::Break => Cow::Borrowed("'break'"),
            TokenKind::Continue => Cow::Borrowed("'continue'"),
            TokenKind::Fn => Cow::Borrowed("'fn'"),
            TokenKind::TypeOf => Cow::Borrowed("'typeof'"),
            TokenKind::Return => Cow::Borrowed("'return'"),
            TokenKind::Pipe => Cow::Borrowed("'|'"),
            TokenKind::And => Cow::Borrowed("'&&'"),
            TokenKind::Or => Cow::Borrowed("'||'"),
            TokenKind::Plus => Cow::Borrowed("'+'"),
            TokenKind::Minus => Cow::Borrowed("'-'"),
            TokenKind::Star => Cow::Borrowed("'*'"),
            TokenKind::Slash => Cow::Borrowed("'/'"),
            TokenKind::Equals => Cow::Borrowed("'='"),
            TokenKind::EqualsEquals => Cow::Borrowed("'=='"),
            TokenKind::Not => Cow::Borrowed("'!'"),
            TokenKind::NotEquals => Cow::Borrowed("'!='"),
            TokenKind::LessThan => Cow::Borrowed("'<'"),
            TokenKind::LtEquals => Cow::Borrowed("'<='"),
            TokenKind::GreaterThan => Cow::Borrowed("'>'"),
            TokenKind::GtEquals => Cow::Borrowed("'>='"),
            TokenKind::PlusEquals => Cow::Borrowed("'+='"),
            TokenKind::MinusEquals => Cow::Borrowed("'-='"),
            TokenKind::StarEquals => Cow::Borrowed("'*='"),
            TokenKind::SlashEquals => Cow::Borrowed("'/='"),
            TokenKind::FatArrow => Cow::Borrowed("'=>'"),
            TokenKind::ThinArrow => Cow::Borrowed("'->'"),
            TokenKind::ThinArrowAppend => Cow::Borrowed("'->>'"),
            TokenKind::DollarLeftParen => Cow::Borrowed("'$('"),
            TokenKind::LeftCurly => Cow::Borrowed("'{'"),
            TokenKind::RightCurly => Cow::Borrowed("'}'"),
            TokenKind::LeftParen => Cow::Borrowed("'('"),
            TokenKind::RightParen => Cow::Borrowed("')'"),
            TokenKind::LeftBracket => Cow::Borrowed("'['"),
            TokenKind::RightBracket => Cow::Borrowed("']'"),
            TokenKind::Comma => Cow::Borrowed("','"),
            TokenKind::Dot => Cow::Borrowed("'.'"),
            TokenKind::Semicolon => Cow::Borrowed("';'"),
            TokenKind::NewLine => Cow::Borrowed("newline"),
            TokenKind::EOF => Cow::Borrowed("end of input"),
            TokenKind::Ampersand => Cow::Borrowed("'&'"),
        }
    }

    pub(crate) fn current_token_label(&self) -> Cow<'static, str> {
        Self::token_kind_label(&self.current().kind)
    }

    pub(crate) fn diagnostic_here(&self, message: impl Into<String>) -> Diagnostic {
        let token = self.current();
        let start = token.offset;
        let end = start + Self::token_source_len(token).max(1);

        Diagnostic::new(DiagnosticKind::ParseError, start, end, message)
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

    pub(crate) fn token_source_len(token: &Token<'_>) -> usize {
        token.len
    }

    pub(crate) fn is_contiguous_with_prev(&self) -> bool {
        let Some(prev_idx) = self.index.checked_sub(1) else {
            return false;
        };
        let Some(current) = self.tokens.get(self.index) else {
            return false;
        };
        let prev = &self.tokens[prev_idx];
        current.offset == prev.offset + Self::token_source_len(prev)
    }

    pub(crate) fn is_statement_end(&self) -> bool {
        self.check_current(&TokenKind::Semicolon)
            || self.check_current(&TokenKind::NewLine)
            || self.check_current(&TokenKind::RightCurly)
            || self.check_current(&TokenKind::EOF)
    }

    pub(crate) fn parse_param_list(
        &mut self,
        missing_rparen_msg: &str,
    ) -> Result<Vec<String>, Diagnostic> {
        let mut params = vec![];
        if !self.check_current(&TokenKind::RightParen) {
            loop {
                match &self.current().kind {
                    TokenKind::String(p) => params.push(p.as_ref().to_owned()),
                    other => {
                        return self.error_here(format!(
                            "Expected parameter name, found {}.",
                            Self::token_kind_label(other)
                        ));
                    }
                }
                self.advance();

                if self.check_current(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RightParen, missing_rparen_msg)?;
        Ok(params)
    }
}

#[cfg(test)]
mod tests;
