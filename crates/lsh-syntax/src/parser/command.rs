use crate::{
    diagnostics::Diagnostic,
    lexer::token::TokenKind,
    parser::ast::{BinaryOp, CommandExpr, Expr, Redirect},
};

use super::Parser;

impl Parser {
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
        terminators: &[TokenKind],
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
        terminators: &[TokenKind],
    ) -> Result<CommandExpr, Diagnostic> {
        let mut stages = vec![self.parse_command_call(terminators)?];

        while self.check_current(&TokenKind::Pipe) {
            self.advance(); // Skip '|'
            stages.push(self.parse_command_call(terminators)?);
        }

        if stages.len() == 1 {
            Ok(stages.remove(0))
        } else {
            Ok(CommandExpr::Pipeline(stages))
        }
    }

    fn parse_command_call(&mut self, terminators: &[TokenKind]) -> Result<CommandExpr, Diagnostic> {
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

        if self.check_current(&TokenKind::LeftParen) {
            let previous_token = &self.tokens[self.index - 1];
            let current_token = self.current();
            if current_token.offset
                == previous_token.offset + Self::token_source_len(previous_token)
            {
                return self.error_here("Command names cannot be immediately followed by '(' - did you mean a function call?");
            }
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

        let stdout_redirect = if self.check_current(&TokenKind::ThinArrow) {
            self.advance(); // Skip '->'.
            let Some(target) = self.parse_command_argument()? else {
                return self.error_here("Expected redirect target after '->'.");
            };
            Some(Redirect {
                target,
                append: false,
            })
        } else if self.check_current(&TokenKind::ThinArrowAppend) {
            self.advance(); // Skip '->>'.
            let Some(target) = self.parse_command_argument()? else {
                return self.error_here("Expected redirect target after '->>'.");
            };
            Some(Redirect {
                target,
                append: true,
            })
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
        terminators: &[TokenKind],
    ) -> Result<Option<(String, Expr)>, Diagnostic> {
        let start_index = self.index;

        let (name, name_offset) = match &self.current().kind {
            TokenKind::String(name) => (name.clone(), self.current().offset),
            _ => return Ok(None),
        };

        if !Self::is_valid_env_name(name.as_str()) {
            return Ok(None);
        }

        let Some(eq_token) = self.tokens.get(self.index + 1) else {
            return Ok(None);
        };
        if !eq_token.kind.compare(&TokenKind::Equals) {
            return Ok(None);
        }
        let eq_offset = eq_token.offset;
        let eq_len = Self::token_source_len(eq_token);
        if eq_offset != name_offset + name.len() {
            return Ok(None);
        }

        self.advance(); // Move to '='
        self.advance(); // Move to start of value / command identifier

        if self.check_current(&TokenKind::Pipe)
            || self.check_current(&TokenKind::And)
            || self.check_current(&TokenKind::Or)
            || terminators.iter().any(|term| self.check_current(term))
        {
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

        let Some(value_expr) = self.parse_command_argument()? else {
            return Ok(Some((name, Expr::String(String::new()))));
        };

        Ok(Some((name, value_expr)))
    }

    fn parse_command_argument(&mut self) -> Result<Option<Expr>, Diagnostic> {
        let Some((mut expr, mut end_offset)) = self.parse_command_arg_part()? else {
            return Ok(None);
        };

        // Concatenate adjacent (no-whitespace) command parts into a single argument.
        while self
            .tokens
            .get(self.index)
            .is_some_and(|token| token.offset == end_offset)
        {
            let Some((right_expr, right_end_offset)) = self.parse_command_arg_part()? else {
                break;
            };

            expr = Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(expr),
                right: Box::new(right_expr),
            };
            end_offset = right_end_offset;
        }

        Ok(Some(expr))
    }

    fn parse_command_arg_part(&mut self) -> Result<Option<(Expr, usize)>, Diagnostic> {
        if self.check_current(&TokenKind::LeftParen) {
            // If it's identifier( it's not a command argument part.
            let prev_token = &self.tokens[self.index - 1];
            let current_token = self.current();
            if current_token.offset == prev_token.offset + Self::token_source_len(prev_token) {
                return Ok(None);
            }
        }

        if let Some(compound_arg) = self.try_parse_command_equals_compound_arg() {
            let end_offset = self.end_offset_of_last_consumed_token();
            return Ok(Some((compound_arg, end_offset)));
        }

        if let TokenKind::DoubleQuotedString(s) = &self.current().kind {
            let token = self.current().to_owned();
            let parsed = Self::parse_interpolated_string(self.src_id, s, token.offset)?;
            self.advance();
            let end_offset = token.offset + Self::token_source_len(&token);
            return Ok(Some((parsed, end_offset)));
        }

        if let Some(literal) = Self::token_kind_to_command_literal(&self.current().kind) {
            let token = self.current().to_owned();
            self.advance();
            let end_offset = token.offset + Self::token_source_len(&token);
            return Ok(Some((literal, end_offset)));
        }

        if let TokenKind::Variable(var) = &self.current().kind {
            let var_name = var.clone();
            self.advance(); // Move to token after variable.
            let sub_expr = self.parse_postfix_on_current(Expr::Variable(var_name))?;
            let end_offset = self.end_offset_of_last_consumed_token();
            return Ok(Some((sub_expr, end_offset)));
        }

        if self.check_current(&TokenKind::DollarLeftParen) {
            let sub_expr = self.parse_primary()?;
            let sub_expr = self.parse_postfix_on_current(sub_expr)?;
            let end_offset = self.end_offset_of_last_consumed_token();
            return Ok(Some((sub_expr, end_offset)));
        }

        Ok(None)
    }

    fn try_parse_command_equals_compound_arg(&mut self) -> Option<Expr> {
        let start_index = self.index;
        let mut value = Self::token_kind_to_compound_command_part(&self.current().kind)?;
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
            value.push_str(&next_value);

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

    fn token_kind_to_command_literal(kind: &TokenKind) -> Option<Expr> {
        match kind {
            TokenKind::String(s) => Some(Expr::String(s.clone())),
            TokenKind::QuotedString(s) => Some(Expr::QuotedString(s.clone())),
            TokenKind::Number(n) => Some(Expr::Number(*n)),
            TokenKind::Bool(b) => Some(Expr::Bool(*b)),
            TokenKind::For => Some(Expr::String("for".to_string())),
            TokenKind::In => Some(Expr::String("in".to_string())),
            TokenKind::Break => Some(Expr::String("break".to_string())),
            TokenKind::Continue => Some(Expr::String("continue".to_string())),
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
            TokenKind::LeftBracket => Some(Expr::String("[".to_string())),
            TokenKind::RightBracket => Some(Expr::String("]".to_string())),
            TokenKind::LeftCurly => Some(Expr::String("{".to_string())),
            TokenKind::RightCurly => Some(Expr::String("}".to_string())),
            _ => None,
        }
    }

    fn token_kind_to_compound_command_part(kind: &TokenKind) -> Option<String> {
        match kind {
            TokenKind::String(s) | TokenKind::QuotedString(s) => Some(s.clone()),
            TokenKind::Number(n) => Some(n.to_string()),
            TokenKind::Bool(b) => Some(b.to_string()),
            TokenKind::For => Some("for".to_string()),
            TokenKind::In => Some("in".to_string()),
            TokenKind::Break => Some("break".to_string()),
            TokenKind::Continue => Some("continue".to_string()),
            TokenKind::LeftBracket => Some("[".to_string()),
            TokenKind::RightBracket => Some("]".to_string()),
            TokenKind::LeftCurly => Some("{".to_string()),
            TokenKind::RightCurly => Some("}".to_string()),
            _ => None,
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

    fn end_offset_of_last_consumed_token(&self) -> usize {
        if self.index == 0 {
            return self.current().offset;
        }

        let token = &self.tokens[self.index - 1];
        token.offset + Self::token_source_len(token)
    }
}
