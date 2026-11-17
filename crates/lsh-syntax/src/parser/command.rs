use std::borrow::Cow;

use crate::{
    diagnostics::Diagnostic,
    lexer::token::TokenKind,
    parser::ast::{BinaryOp, CommandExpr, Expr, Redirect},
};

use super::Parser;

impl Parser<'_> {
    pub(crate) fn parse_command_expr(&mut self) -> Result<CommandExpr, Diagnostic> {
        self.parse_command_expr_until(&[
            TokenKind::NewLine,
            TokenKind::Semicolon,
            TokenKind::EOF,
            TokenKind::RightCurly,
        ])
    }

    pub(crate) fn parse_command_expr_until(
        &mut self,
        terminators: &[TokenKind<'_>],
    ) -> Result<CommandExpr, Diagnostic> {
        let mut expr = self.parse_command_pipeline(terminators)?;

        while self.check_current(&TokenKind::And) || self.check_current(&TokenKind::Or) {
            if self.check_current(&TokenKind::And) {
                self.advance(); // Skip '&&'
                let right = self.parse_command_pipeline(terminators)?;
                expr = CommandExpr::And {
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            } else {
                self.advance(); // Skip '||'
                let right = self.parse_command_pipeline(terminators)?;
                expr = CommandExpr::Or {
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            }
        }

        if self.check_current(&TokenKind::Ampersand) {
            self.advance(); // Skip '&'
            expr = CommandExpr::Background(Box::new(expr));
        }

        Ok(expr)
    }

    fn parse_command_pipeline(
        &mut self,
        terminators: &[TokenKind<'_>],
    ) -> Result<CommandExpr, Diagnostic> {
        let first = self.parse_command_call(terminators)?;
        if !self.check_current(&TokenKind::Pipe) {
            return Ok(first);
        }

        let mut stages = vec![first];
        while self.check_current(&TokenKind::Pipe) {
            self.advance(); // Skip '|'
            stages.push(self.parse_command_call(terminators)?);
        }

        Ok(CommandExpr::Pipeline(stages))
    }

    fn parse_command_call(
        &mut self,
        terminators: &[TokenKind<'_>],
    ) -> Result<CommandExpr, Diagnostic> {
        let mut env_assignments = vec![];
        while let Some(assignment) = self.try_parse_env_assignment_prefix(terminators)? {
            env_assignments.push(assignment);
        }

        let Some(identifier) = self.parse_command_argument()? else {
            return self.error_here(format!(
                "Expected command name, found {}.",
                Self::token_kind_label(&self.current().kind)
            ));
        };

        if self.check_current(&TokenKind::LeftParen) && self.is_contiguous_with_prev() {
            return self.error_here(
                "Command names cannot be immediately followed by '(' - did you mean a function call?",
            );
        }

        let mut args = vec![];
        while !self.check_current(&TokenKind::Pipe)
            && !self.check_current(&TokenKind::And)
            && !self.check_current(&TokenKind::Or)
            && !self.check_current(&TokenKind::Ampersand)
            && !self.check_current(&TokenKind::ThinArrow)
            && !self.check_current(&TokenKind::ThinArrowAppend)
            && !terminators.iter().any(|term| self.check_current(term))
        {
            if let Some(arg) = self.parse_command_argument()? {
                args.push(arg);
                continue;
            }
            break;
        }

        let stdout_redirect = if self.check_current(&TokenKind::ThinArrow)
            || self.check_current(&TokenKind::ThinArrowAppend)
        {
            let append = self.check_current(&TokenKind::ThinArrowAppend);
            let arrow = if append { "->>" } else { "->" };
            self.advance(); // Skip '->' or '->>'.
            let Some(target) = self.parse_command_argument()? else {
                return self.error_here(format!("Expected redirect target after '{arrow}'."));
            };
            Some(Redirect { target, append })
        } else {
            None
        };

        Ok(CommandExpr::Call {
            env_assignments,
            identifier,
            args,
            stdout_redirect,
        })
    }

    fn try_parse_env_assignment_prefix(
        &mut self,
        terminators: &[TokenKind<'_>],
    ) -> Result<Option<(String, Expr)>, Diagnostic> {
        let start_index = self.index;

        let (name_len, name_offset) = match &self.current().kind {
            TokenKind::String(name) => {
                if !Self::is_valid_env_name(name.as_ref()) {
                    return Ok(None);
                }
                (name.len(), self.current().offset)
            }
            _ => return Ok(None),
        };

        let Some(eq_token) = self.tokens.get(self.index + 1) else {
            return Ok(None);
        };
        if !eq_token.kind.compare(&TokenKind::Equals) {
            return Ok(None);
        }
        let eq_offset = eq_token.offset;
        let eq_len = Self::token_source_len(eq_token);
        if eq_offset != name_offset + name_len {
            return Ok(None);
        }

        self.advance(); // Move to '='
        self.advance(); // Move to start of value / command identifier

        if self.check_current(&TokenKind::Pipe)
            || self.check_current(&TokenKind::And)
            || self.check_current(&TokenKind::Or)
            || terminators.iter().any(|term| self.check_current(term))
        {
            let name = match &self.tokens[start_index].kind {
                TokenKind::String(name) => name.as_ref().to_owned(),
                _ => unreachable!(),
            };
            return Ok(Some((name, Expr::String(String::new()))));
        }

        let eq_end_offset = eq_offset + eq_len;
        let current_offset = self.current().offset;
        // Values for prefix assignments must be contiguous with '='.
        let contiguous = current_offset == eq_end_offset;
        if !contiguous {
            self.index = start_index;
            return Ok(None);
        }

        let name = match &self.tokens[start_index].kind {
            TokenKind::String(name) => name.as_ref().to_owned(),
            _ => unreachable!(),
        };

        let Some(value_expr) = self.parse_command_argument()? else {
            return Ok(Some((name, Expr::String(String::new()))));
        };

        Ok(Some((name, value_expr)))
    }

    fn parse_command_argument(&mut self) -> Result<Option<Expr>, Diagnostic> {
        let Some(mut expr) = self.parse_command_arg_part()? else {
            return Ok(None);
        };

        // Concatenate adjacent (no-whitespace) command parts into a single argument.
        while self.is_contiguous_with_prev() {
            let Some(right_expr) = self.parse_command_arg_part()? else {
                break;
            };

            expr = Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(expr),
                right: Box::new(right_expr),
            };
        }

        Ok(Some(expr))
    }

    fn parse_command_arg_part(&mut self) -> Result<Option<Expr>, Diagnostic> {
        if self.check_current(&TokenKind::LeftParen) && self.is_contiguous_with_prev() {
            // If it's identifier( it's not a command argument part.
            return Ok(None);
        }

        if let Some(compound_arg) = self.try_parse_command_equals_compound_arg() {
            return Ok(Some(compound_arg));
        }

        if let TokenKind::DoubleQuotedString(s) = &self.current().kind {
            let parsed = Self::parse_interpolated_string(self.src_id, s, self.current().offset)?;
            self.advance();
            return Ok(Some(parsed));
        }

        if let Some(literal) = Self::token_kind_to_command_literal(&self.current().kind) {
            self.advance();
            return Ok(Some(literal));
        }

        if let TokenKind::Variable(var) = &self.current().kind {
            let var_name = var.as_ref().to_owned();
            self.advance(); // Move to token after variable.
            let sub_expr = self.parse_postfix_on_current(Expr::Variable(var_name))?;
            return Ok(Some(sub_expr));
        }

        if self.check_current(&TokenKind::DollarLeftParen) {
            let sub_expr = self.parse_primary()?;
            let sub_expr = self.parse_postfix_on_current(sub_expr)?;
            return Ok(Some(sub_expr));
        }

        Ok(None)
    }

    fn try_parse_command_equals_compound_arg(&mut self) -> Option<Expr> {
        if !self.check_peek(1, &TokenKind::Equals) {
            return None;
        }
        let next_kind = &self.tokens.get(self.index + 2)?.kind;
        Self::token_kind_to_compound_command_part(next_kind)?;

        let start_index = self.index;
        let mut value =
            Self::token_kind_to_compound_command_part(&self.current().kind)?.into_owned();
        self.advance();

        let mut consumed_equals = false;

        loop {
            if !self.check_current(&TokenKind::Equals) {
                break;
            }

            let Some(next_kind) = self.tokens.get(self.index + 1).map(|token| &token.kind) else {
                break;
            };
            let Some(next_value) = Self::token_kind_to_compound_command_part(next_kind) else {
                break;
            };

            consumed_equals = true;
            value.push('=');
            value.push_str(next_value.as_ref());

            self.advance(); // Skip '='.
            self.advance(); // Move to and consume next literal part.
        }

        if consumed_equals {
            Some(Expr::String(value))
        } else {
            self.index = start_index;
            None
        }
    }

    fn token_kind_to_word_str(kind: &TokenKind<'_>) -> Option<&'static str> {
        match kind {
            TokenKind::For => Some("for"),
            TokenKind::In => Some("in"),
            TokenKind::Break => Some("break"),
            TokenKind::Continue => Some("continue"),
            TokenKind::LeftBracket => Some("["),
            TokenKind::RightBracket => Some("]"),
            TokenKind::LeftCurly => Some("{"),
            TokenKind::RightCurly => Some("}"),
            _ => None,
        }
    }

    fn token_kind_to_command_literal(kind: &TokenKind<'_>) -> Option<Expr> {
        match kind {
            TokenKind::String(s) => Some(Expr::String(s.as_ref().to_owned())),
            TokenKind::QuotedString(s) => Some(Expr::QuotedString(s.as_ref().to_owned())),
            TokenKind::Number(n) => Some(Expr::Number(*n)),
            TokenKind::Bool(b) => Some(Expr::Bool(*b)),
            // Treat punctuation/operator tokens as literal string arguments in command context.
            TokenKind::Comma => Some(Expr::String(",".to_string())),
            TokenKind::Dot => Some(Expr::String(".".to_string())),
            TokenKind::Minus => Some(Expr::String("-".to_string())),
            TokenKind::Plus => Some(Expr::String("+".to_string())),
            TokenKind::Star => Some(Expr::String("*".to_string())),
            TokenKind::Slash => Some(Expr::String("/".to_string())),
            TokenKind::Equals => Some(Expr::String("=".to_string())),
            TokenKind::Not => Some(Expr::String("!".to_string())),
            TokenKind::GreaterThan => Some(Expr::String(">".to_string())),
            TokenKind::LessThan => Some(Expr::String("<".to_string())),
            other => Self::token_kind_to_word_str(other).map(|s| Expr::String(s.to_owned())),
        }
    }

    fn token_kind_to_compound_command_part<'b>(kind: &'b TokenKind<'_>) -> Option<Cow<'b, str>> {
        match kind {
            TokenKind::String(s) | TokenKind::QuotedString(s) => Some(Cow::Borrowed(s.as_ref())),
            TokenKind::Number(n) => Some(Cow::Owned(n.to_string())),
            TokenKind::Bool(b) => Some(Cow::Borrowed(if *b { "true" } else { "false" })),
            other => Self::token_kind_to_word_str(other).map(Cow::Borrowed),
        }
    }

    fn is_valid_env_name(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };

        if !(first == '_' || first.is_ascii_alphabetic()) {
            return false;
        }

        chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
    }
}
