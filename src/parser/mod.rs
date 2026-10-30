use std::collections::HashMap;

use crate::{
    diagnostics::{Diagnostic, DiagnosticKind},
    lexer::token::{Token, TokenKind},
    parser::ast::{BinaryOp, CommandExpr, Expr, Program, Statement},
};

pub mod ast;

pub struct Parser {
    src_id: usize,
    tokens: Vec<Token>,
    index: usize,
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

    fn parse_program_unchecked(&mut self) -> Result<Program, Diagnostic> {
        let mut body = vec![];

        self.skip_statement_separators();
        while !self.check_current(&TokenKind::EOF) {
            body.push(self.parse_statement()?);
            self.skip_statement_separators();
        }

        Ok(Program { body })
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        self.skip_statement_separators();

        match &self.current().kind {
            TokenKind::Let => self.parse_let(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Fn => self.parse_function(),
            TokenKind::Return => self.parse_return(),
            TokenKind::LeftCurly => {
                // Handle blocks.

                self.advance(); // Skip past '{'.

                let mut statements = vec![];

                while !self.check_current(&TokenKind::RightCurly) {
                    while self.check_current(&TokenKind::NewLine)
                        || self.check_current(&TokenKind::Semicolon)
                    {
                        self.advance();
                    }

                    if self.check_current(&TokenKind::RightCurly) {
                        break;
                    }

                    statements.push(self.parse_statement()?);
                }

                self.consume(&TokenKind::RightCurly, "Expected '}' after block.")?;

                Ok(Statement::Block(statements))
            }
            TokenKind::Dot | TokenKind::Slash => Ok(Statement::Command(self.parse_command_expr()?)),
            TokenKind::String(_) | TokenKind::QuotedString(_) | TokenKind::Variable(_) => {
                let initial_index = self.index;
                let expr = match self.parse_postfix() {
                    Ok(expr) => expr,
                    Err(diag) if diag.message.starts_with("Expected identifier after '.'") => {
                        self.index = initial_index;
                        return Ok(Statement::Command(self.parse_command_expr()?));
                    }
                    Err(diag) => return Err(diag),
                };

                // Assignment is only valid for assignable targets.
                if self.check_current(&TokenKind::Equals)
                    && matches!(expr, Expr::Variable(_) | Expr::Index { .. })
                {
                    self.advance(); // Skip '='.
                    let value = self.parse_expr()?;
                    return Ok(Statement::Assign {
                        target: expr,
                        value,
                    });
                }

                // If it's a Call and the statement ends here, treat it as an expression statement.
                // Otherwise, backtrack and parse command operators (&&, ||, |, ...).
                let statement_ends = self.check_current(&TokenKind::Semicolon)
                    || self.check_current(&TokenKind::NewLine)
                    || self.check_current(&TokenKind::EOF)
                    || self.check_current(&TokenKind::RightCurly);

                if matches!(expr, Expr::Call { .. }) && statement_ends {
                    self.match_token(&[TokenKind::Semicolon]);
                    return Ok(Statement::Expr(expr));
                }

                // Otherwise, backtrack and parse as command.
                self.index = initial_index;
                Ok(Statement::Command(self.parse_command_expr()?))
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(Statement::Expr(expr))
            }
        }
    }

    fn parse_command_expr(&mut self) -> Result<CommandExpr, Diagnostic> {
        self.parse_command_expr_until(&[
            TokenKind::NewLine,
            TokenKind::Semicolon,
            TokenKind::EOF,
            TokenKind::RightCurly,
        ])
    }

    fn parse_command_expr_until(
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
            && !self.check_current(&TokenKind::ThinArrow)
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
            Some(target)
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
        // Values for prefix assignments must be contiguous with '='. `$VAR` is a
        // special case where the variable token starts one byte after '$'.
        let contiguous = current_offset == eq_end_offset
            || (matches!(self.current().kind, TokenKind::Variable(_))
                && current_offset == eq_end_offset + 1);
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

    fn parse_let(&mut self) -> Result<Statement, Diagnostic> {
        let name = if let TokenKind::String(name) = self.advance().kind.clone() {
            name
        } else {
            return self.error_here("Expected identifier after 'let'.");
        };

        // Advance to next token which should be an '=' and try to consume it.
        self.advance();
        self.consume(&TokenKind::Equals, "Expected '=' after variable name.")?;

        let expr = self.parse_expr()?;
        Ok(Statement::Let { name, value: expr })
    }

    fn parse_if(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Advance past 'if'.

        let condition = self.parse_expr()?;

        let then_branch = match self.parse_statement()? {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        let else_branch = if self.match_token(&[TokenKind::Else]) {
            let branch = match self.parse_statement()? {
                Statement::Block(stmts) => stmts,
                stmt => vec![stmt],
            };
            Some(branch)
        } else {
            None
        };

        Ok(Statement::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while(&mut self) -> Result<Statement, Diagnostic> {
        self.advance();

        let condition = self.parse_expr()?;

        let body = match self.parse_statement()? {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        Ok(Statement::While { condition, body })
    }

    fn parse_function(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Skip 'fn'.

        let target = self.parse_target()?;

        self.consume(&TokenKind::LeftParen, "Expected '(' after function target")?;

        let mut params = vec![];

        if !self.check_current(&TokenKind::RightParen) {
            loop {
                match &self.current().kind {
                    TokenKind::String(p) => params.push(p.clone()),
                    other => {
                        return self.error_here(format!(
                            "Expected parameter name, found {}.",
                            Self::token_kind_label(other)
                        ));
                    }
                }
                self.advance();

                if self.check_current(&TokenKind::Comma) {
                    self.advance(); // Skip comma and continue.
                } else {
                    break;
                }
            }
        }

        self.consume(
            &TokenKind::RightParen,
            "Expected ')' after function parameters",
        )?;

        let body = match self.parse_statement()? {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        Ok(Statement::FunctionDeclaration {
            target,
            params,
            body,
        })
    }

    fn parse_return(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Skip `return`.

        let value = if self.check_current(&TokenKind::Semicolon)
            || self.check_current(&TokenKind::NewLine)
            || self.check_current(&TokenKind::RightCurly)
            || self.check_current(&TokenKind::EOF)
        {
            None
        } else {
            Some(self.parse_expr()?)
        };

        // Optional semicolon after return value (or bare return).
        if self.check_current(&TokenKind::Semicolon) {
            self.advance();
        }

        Ok(Statement::Return(value))
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_and()?;

        while self.match_token(&[TokenKind::Or]) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_equality()?;

        while self.match_token(&[TokenKind::And]) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_comparison()?;
        while let Some(op) = match self.current().kind {
            TokenKind::EqualsEquals => Some(BinaryOp::Eq),
            TokenKind::NotEquals => Some(BinaryOp::Neq),
            _ => None,
        } {
            self.advance();
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_term()?;
        while let Some(op) = match self.current().kind {
            TokenKind::LessThan => Some(BinaryOp::Lt),
            TokenKind::GreaterThan => Some(BinaryOp::Gt),
            TokenKind::LtEquals => Some(BinaryOp::Le),
            TokenKind::GtEquals => Some(BinaryOp::Ge),
            _ => None,
        } {
            self.advance();
            let right = self.parse_term()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_factor()?;
        while let Some(op) = match self.current().kind {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        } {
            self.advance();
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_postfix()?;
        while let Some(op) = match self.current().kind {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            _ => None,
        } {
            self.advance();
            let right = self.parse_postfix()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.parse_primary()?;
        self.parse_postfix_on_current(expr)
    }

    fn parse_target(&mut self) -> Result<Expr, Diagnostic> {
        let expr = match self.parse_primary()? {
            Expr::Word(name) => Expr::Variable(name),
            other => other,
        };
        self.parse_target_on_current(expr)
    }

    fn parse_target_on_current(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if self.check_current(&TokenKind::LeftBracket) {
                self.advance(); // Skip '['.

                let index_expr = self.parse_expr()?;

                self.consume(
                    &TokenKind::RightBracket,
                    "Expected ']' after object index expression.",
                )?;

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index_expr),
                };
            } else if self.check_current(&TokenKind::Dot) {
                self.advance(); // Skip '.'.
                let key = match &self.current().kind {
                    TokenKind::String(s) => s.clone(),
                    other => {
                        return self.error_here(format!(
                            "Expected identifier after '.', found {}.",
                            Self::token_kind_label(other)
                        ));
                    }
                };
                self.advance();

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(Expr::String(key)),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_postfix_on_current(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if self.check_current(&TokenKind::LeftBracket) {
                self.advance(); // Skip '['.

                let index_expr = self.parse_expr()?;

                self.consume(
                    &TokenKind::RightBracket,
                    "Expected ']' after object index expression.",
                )?;

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index_expr),
                };
            } else if self.check_current(&TokenKind::LeftParen) {
                // Ensure no whitespace between identifier and '(' for it to be a call.
                let prev_token = &self.tokens[self.index - 1];
                let current_token = self.current();

                if current_token.offset == prev_token.offset + Self::token_source_len(prev_token) {
                    self.advance(); // Skip '('.

                    let mut args = vec![];
                    if !self.check_current(&TokenKind::RightParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.check_current(&TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }

                    self.consume(&TokenKind::RightParen, "Expected ')' after arguments.")?;

                    expr = Expr::Call {
                        target: Box::new(expr),
                        args,
                    };
                } else {
                    break;
                }
            } else if self.check_current(&TokenKind::Dot) {
                self.advance(); // Skip '.'.
                let key = match &self.current().kind {
                    TokenKind::String(s) => s.clone(),
                    other => {
                        return self.error_here(format!(
                            "Expected identifier after '.', found {}.",
                            Self::token_kind_label(other)
                        ));
                    }
                };
                self.advance();

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(Expr::String(key)),
                };
            } else if let TokenKind::String(member_access) = &self.current().kind {
                if member_access.starts_with('.') {
                    // `.foo.bar` is syntax sugar for `["foo"]["bar"]`.
                    let member_access = member_access.clone();
                    self.advance();

                    for key in member_access
                        .split('.')
                        .skip(1)
                        .filter(|part| !part.is_empty())
                    {
                        expr = Expr::Index {
                            object: Box::new(expr),
                            index: Box::new(Expr::String(key.to_string())),
                        };
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let mut should_advance = true;

        let expr = match &self.current().kind {
            TokenKind::Number(n) => Expr::Number(*n),
            TokenKind::Colon => Expr::String(":".to_string()),
            TokenKind::QuotedString(s) => Expr::QuotedString(s.clone()),
            TokenKind::String(s) => {
                // Check if this is the start of an anonymous function: x => ...
                if self.check_peek(1, &TokenKind::FatArrow) {
                    should_advance = false;
                    self.parse_anonymous_function()?
                } else {
                    let call_ident = s.to_owned();
                    let mut args = vec![];

                    while let Some(next_token) = self.tokens.get(self.index + 1) {
                        let current = self.current().to_owned();
                        let is_contiguous = next_token.offset
                            == current.offset + Self::token_source_len(&current);

                        if matches!(next_token.kind, TokenKind::Star | TokenKind::Slash)
                            && !is_contiguous
                        {
                            break;
                        }

                        // This should really just use some form of parse_primary/parse_expr.
                        if let Some(arg_expr) =
                            Self::token_kind_to_primary_call_arg_literal(&next_token.kind)
                        {
                            self.advance(); // Move to literal.

                            if is_contiguous && !args.is_empty() {
                                let previous = args
                                    .pop()
                                    .expect("Expected previous argument while concatenating.");
                                args.push(Expr::Binary {
                                    op: BinaryOp::Add,
                                    left: Box::new(previous),
                                    right: Box::new(arg_expr),
                                });
                            } else {
                                args.push(arg_expr);
                            }
                        } else if let TokenKind::Variable(var) = &next_token.kind {
                            let var_name = var.clone();
                            self.advance(); // Move to variable.

                            let sub_expr =
                                self.parse_postfix_from_seed(Expr::Variable(var_name))?;
                            args.push(sub_expr);

                            // Keep command-like expression args contiguous:
                            // `echo $HOME/.config` should remain one argument.
                            while let Some(next_part) = self.tokens.get(self.index + 1).cloned() {
                                let current_token = self.current().to_owned();
                                let current_end =
                                    current_token.offset + Self::token_source_len(&current_token);
                                if next_part.offset != current_end {
                                    break;
                                }

                                let Some(part_expr) =
                                    Self::token_kind_to_primary_call_arg_literal(&next_part.kind)
                                else {
                                    break;
                                };

                                self.advance(); // Consume contiguous part token.
                                let previous = args
                                    .pop()
                                    .expect("Expected previous argument while concatenating.");
                                args.push(Expr::Binary {
                                    op: BinaryOp::Add,
                                    left: Box::new(previous),
                                    right: Box::new(part_expr),
                                });
                            }
                        } else if let TokenKind::DollarLeftParen = &next_token.kind {
                            self.advance(); // Move to '$('.
                            let sub_expr = self.parse_primary()?;
                            let sub_expr = self.parse_postfix_on_current(sub_expr)?;
                            args.push(sub_expr);

                            loop {
                                if self.check_current(&TokenKind::NewLine)
                                    || self.check_current(&TokenKind::Semicolon)
                                    || self.check_current(&TokenKind::EOF)
                                    || self.check_current(&TokenKind::RightCurly)
                                    || self.check_current(&TokenKind::Pipe)
                                    || self.check_current(&TokenKind::And)
                                    || self.check_current(&TokenKind::Or)
                                    || self.check_current(&TokenKind::ThinArrow)
                                {
                                    should_advance = false;
                                    break;
                                }

                                if let Some(arg_expr) =
                                    Self::token_kind_to_primary_call_arg_literal(&self.current().kind)
                                {
                                    args.push(arg_expr);
                                    break;
                                } else if let TokenKind::Variable(var) = &self.current().kind {
                                    let var_name = var.clone();
                                    let sub_expr =
                                        self.parse_postfix_from_seed(Expr::Variable(var_name))?;
                                    args.push(sub_expr);
                                    break;
                                } else if self.check_current(&TokenKind::DollarLeftParen) {
                                    self.advance(); // Move past '$('.
                                    let sub = self.parse_primary()?;
                                    let sub = self.parse_postfix_on_current(sub)?;
                                    args.push(sub);
                                } else {
                                    should_advance = false;
                                    break;
                                }
                            }

                            if !should_advance {
                                break;
                            }
                        } else if matches!(next_token.kind, TokenKind::Dot) {
                            // Keep dotted literals correct:
                            // - spaced: `git add .` => "." as separate arg
                            // - contiguous: `..`, `file.txt` => concatenate
                            let current = self.current().to_owned();
                            let is_contiguous = next_token.offset
                                == current.offset + Self::token_source_len(&current);

                            if !is_contiguous {
                                self.advance(); // Move to '.'
                                args.push(Expr::String(".".to_string()));
                                continue;
                            }

                            self.advance(); // Move to '.'

                            let mut suffix = Expr::String(".".to_string());
                            if let Some(after_dot) = self.tokens.get(self.index + 1)
                                && let Some(after_dot_expr) =
                                    Self::token_kind_to_primary_call_arg_literal(&after_dot.kind)
                            {
                                self.advance(); // Move to token after '.'
                                suffix = Expr::Binary {
                                    op: BinaryOp::Add,
                                    left: Box::new(suffix),
                                    right: Box::new(after_dot_expr),
                                };
                            }

                            if let Some(last_arg) = args.pop() {
                                args.push(Expr::Binary {
                                    op: BinaryOp::Add,
                                    left: Box::new(last_arg),
                                    right: Box::new(suffix),
                                });
                            } else {
                                args.push(suffix);
                            }
                        } else {
                            break;
                        }
                    }

                    if args.is_empty() {
                        Expr::Word(call_ident)
                    } else {
                        Expr::Call {
                            target: Box::new(Expr::Word(call_ident)),
                            args,
                        }
                    }
                }
            }
            TokenKind::Variable(var) => Expr::Variable(var.to_owned()),
            TokenKind::Bool(value) => Expr::Bool(*value),
            TokenKind::TypeOf => {
                self.advance(); // Skip past 'typeof'.

                let expr = self.parse_expr()?;

                should_advance = false;
                Expr::TypeOf(Box::new(expr))
            }
            TokenKind::LeftParen => {
                // Check for (a, b) => { ... } or () => { ... }
                let mut arrow_found = false;
                let mut nesting = 0;
                let mut i = self.index;
                while i < self.tokens.len() {
                    match &self.tokens[i].kind {
                        TokenKind::LeftParen => nesting += 1,
                        TokenKind::RightParen => {
                            nesting -= 1;
                            if nesting == 0 {
                                if let Some(next) = self.tokens.get(i + 1)
                                    && next.kind.compare(&TokenKind::FatArrow)
                                {
                                    arrow_found = true;
                                }
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }

                if arrow_found {
                    should_advance = false;
                    self.parse_anonymous_function()?
                } else {
                    self.advance(); // Skip past '('.

                    let expr = self.parse_expr()?;

                    if !self.check_current(&TokenKind::RightParen) {
                        return self.error_here(format!(
                            "Expected ')' after expression, found {}.",
                            self.current_token_label()
                        ));
                    }

                    expr
                }
            }
            TokenKind::DollarLeftParen => {
                self.advance(); // Skip past '$('.

                if matches!(self.current().kind, TokenKind::String(_) | TokenKind::QuotedString(_)) {
                    let cmd_expr = self.parse_command_expr_until(&[TokenKind::RightParen])?;
                    self.consume(
                        &TokenKind::RightParen,
                        "Expected ')' after command substitution.",
                    )?;
                    should_advance = false;
                    Expr::CommandSubCommand(Box::new(cmd_expr))
                } else {
                    let expr = self.parse_expr()?;

                    if !self.check_current(&TokenKind::RightParen) {
                        return self.error_here(format!(
                            "Expected ')' after expression, found {}.",
                            self.current_token_label()
                        ));
                    }

                    Expr::CommandSub(Box::new(expr))
                }
            }
            TokenKind::LeftCurly => {
                self.advance(); // Skip past '{'.

                let mut expr_map = HashMap::new();
                while !self.check_current(&TokenKind::RightCurly) {
                    while self.check_current(&TokenKind::NewLine)
                        || self.check_current(&TokenKind::Semicolon)
                    {
                        self.advance();
                    }

                    if self.check_current(&TokenKind::RightCurly) {
                        break;
                    }

                    let key = match &self.current().kind {
                        TokenKind::String(s) | TokenKind::QuotedString(s) => s.clone(),
                        TokenKind::Variable(v) => v.clone(),
                        TokenKind::Number(n) => n.to_string(),
                        _ => return self.error_here("Expected object key"),
                    };

                    self.advance();
                    self.consume(&TokenKind::Colon, "Expected ':' after object key")?;

                    let value = self.parse_expr()?;

                    expr_map.insert(key, value);

                    if !self.check_current(&TokenKind::RightCurly) {
                        if self.check_current(&TokenKind::Comma) {
                            self.advance();
                        } else if self.check_current(&TokenKind::NewLine)
                            || self.check_current(&TokenKind::Semicolon)
                        {
                            // Newline/semicolon separators are allowed in object literals.
                        } else {
                            self.consume(&TokenKind::Comma, "Expected ',' between object fields.")?;
                        }
                    }
                }
                self.advance(); // Skip past '}'.

                Expr::Object(expr_map)
            }
            TokenKind::ThinArrow => {
                return self.error_here(
                    "Unexpected '->' here. Redirection is only valid after a command.",
                );
            }
            TokenKind::FatArrow => {
                return self.error_here(
                    "Unexpected '=>'. Anonymous function arrows must follow a parameter list.",
                );
            }
            TokenKind::Equals => {
                return self.error_here_with_hint(
                    "Unexpected '=' in expression.",
                    "If you meant comparison, use '=='.",
                );
            }
            _ => {
                return self.error_here(format!(
                    "Unexpected token {} while parsing an expression.",
                    self.current_token_label()
                ));
            }
        };

        if should_advance {
            self.advance();
        }

        Ok(expr)
    }

    fn token_kind_to_command_literal(kind: &TokenKind) -> Option<Expr> {
        match kind {
            TokenKind::String(s) => Some(Expr::String(s.clone())),
            TokenKind::QuotedString(s) => Some(Expr::QuotedString(s.clone())),
            TokenKind::Number(n) => Some(Expr::Number(*n)),
            TokenKind::Bool(b) => Some(Expr::Bool(*b)),
            // Treat punctuation/operator tokens as literal string arguments in command context.
            TokenKind::Comma => Some(Expr::String(",".to_string())),
            TokenKind::Dot => Some(Expr::String(".".to_string())),
            TokenKind::Colon => Some(Expr::String(":".to_string())),
            TokenKind::Minus => Some(Expr::String("-".to_string())),
            TokenKind::Plus => Some(Expr::String("+".to_string())),
            TokenKind::Star => Some(Expr::String("*".to_string())),
            TokenKind::Slash => Some(Expr::String("/".to_string())),
            TokenKind::Equals => Some(Expr::String("=".to_string())),
            TokenKind::Not => Some(Expr::String("!".to_string())),
            TokenKind::GreaterThan => Some(Expr::String(">".to_string())),
            TokenKind::LessThan => Some(Expr::String("<".to_string())),
            TokenKind::Ampersand => Some(Expr::String("&".to_string())),
            _ => None,
        }
    }

    fn token_kind_to_compound_command_part(kind: &TokenKind) -> Option<String> {
        match kind {
            TokenKind::String(s) | TokenKind::QuotedString(s) => Some(s.clone()),
            TokenKind::Number(n) => Some(n.to_string()),
            TokenKind::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    fn token_source_len(token: &Token) -> usize {
        match &token.kind {
            TokenKind::Variable(name) => name.len(),
            TokenKind::String(s) => s.len(),
            TokenKind::QuotedString(s) => s.len() + 2,
            TokenKind::Number(n) => n.to_string().len(),
            TokenKind::Bool(b) => b.to_string().len(),
            TokenKind::And
            | TokenKind::Or
            | TokenKind::EqualsEquals
            | TokenKind::GtEquals
            | TokenKind::LtEquals
            | TokenKind::NotEquals
            | TokenKind::PlusEquals
            | TokenKind::MinusEquals
            | TokenKind::StarEquals
            | TokenKind::SlashEquals
            | TokenKind::DollarLeftParen
            | TokenKind::FatArrow
            | TokenKind::ThinArrow => 2,
            TokenKind::EOF => 0,
            _ => 1,
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

    fn token_kind_to_primary_call_arg_literal(kind: &TokenKind) -> Option<Expr> {
        match kind {
            TokenKind::String(s) => Some(Expr::String(s.clone())),
            TokenKind::QuotedString(s) => Some(Expr::QuotedString(s.clone())),
            TokenKind::Number(n) => Some(Expr::Number(*n)),
            TokenKind::Bool(b) => Some(Expr::Bool(*b)),
            // Keep these for command-like expression args only.
            TokenKind::Star => Some(Expr::String("*".to_string())),
            TokenKind::Slash => Some(Expr::String("/".to_string())),
            TokenKind::Colon => Some(Expr::String(":".to_string())),
            _ => None,
        }
    }

    fn skip_statement_separators(&mut self) {
        while self.check_current(&TokenKind::NewLine) || self.check_current(&TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn parse_postfix_from_seed(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if self
                .tokens
                .get(self.index + 1)
                .is_some_and(|tok| tok.kind.compare(&TokenKind::LeftBracket))
            {
                self.advance(); // Move to '['.
                self.advance(); // Skip '['.

                let index_expr = self.parse_expr()?;

                self.consume(
                    &TokenKind::RightBracket,
                    "Expected ']' after object index expression.",
                )?;

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index_expr),
                };
            } else if self
                .tokens
                .get(self.index + 1)
                .is_some_and(|tok| tok.kind.compare(&TokenKind::Dot))
            {
                self.advance(); // Move to '.'.
                self.advance(); // Skip '.'.

                let key = match &self.current().kind {
                    TokenKind::String(s) => s.clone(),
                    other => {
                        return self.error_here(format!(
                            "Expected identifier after '.', found {}.",
                            Self::token_kind_label(other)
                        ));
                    }
                };

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(Expr::String(key)),
                };
            } else if let Some(Token {
                kind: TokenKind::String(member_access),
                ..
            }) = self.tokens.get(self.index + 1)
            {
                if member_access.starts_with('.') {
                    let member_access = member_access.to_owned();
                    self.advance(); // Move to `.foo` token.

                    for key in member_access
                        .split('.')
                        .skip(1)
                        .filter(|part| !part.is_empty())
                    {
                        expr = Expr::Index {
                            object: Box::new(expr),
                            index: Box::new(Expr::String(key.to_string())),
                        };
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("Failed to get current token."))
    }

    fn peek_token(&self, distance: usize) -> &Token {
        self.tokens
            .get(self.index + distance)
            .unwrap_or_else(|| self.tokens.last().expect("Failed to get last token."))
    }

    fn advance(&mut self) -> &Token {
        // Only index if it is possible.
        if self.index < self.tokens.len() {
            self.index += 1;
        }

        self.current()
    }

    fn check_current(&self, kind_y: &TokenKind) -> bool {
        self.current().kind.compare(kind_y)
    }

    fn check_peek(&self, distance: usize, kind_y: &TokenKind) -> bool {
        self.peek_token(distance).kind.compare(kind_y)
    }

    fn parse_anonymous_function(&mut self) -> Result<Expr, Diagnostic> {
        let mut params = vec![];

        if self.check_current(&TokenKind::LeftParen) {
            self.advance(); // Skip '('.
            if !self.check_current(&TokenKind::RightParen) {
                loop {
                    match &self.current().kind {
                        TokenKind::String(p) => params.push(p.clone()),
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
            self.consume(&TokenKind::RightParen, "Expected ')' after parameters.")?;
        } else {
            // Single parameter without parens: x => ...
            match &self.current().kind {
                TokenKind::String(p) => params.push(p.clone()),
                other => {
                    return self.error_here(format!(
                        "Expected parameter name, found {}.",
                        Self::token_kind_label(other)
                    ));
                }
            }
            self.advance();
        }

        self.consume(&TokenKind::FatArrow, "Expected '=>' after parameters.")?;

        if self.check_current(&TokenKind::EOF) {
            return self.error_here(
                "Expected function body after '=>'. Anonymous functions need an expression or block.",
            );
        }

        let body = match self.parse_statement()? {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        Ok(Expr::AnonymousFunction { params, body })
    }

    fn match_token(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check_current(kind) {
                self.advance();
                return true;
            }
        }

        false
    }

    fn consume(&mut self, kind: &TokenKind, err_msg: &str) -> Result<Token, Diagnostic> {
        if self.check_current(kind) {
            return Ok(self.advance().to_owned());
        }

        let expected = Self::token_kind_label(kind);
        let found = self.current_token_label();
        let msg = format!("{err_msg} Found {found}, expected {expected}.");
        self.error_here(msg)
    }

    fn token_kind_label(kind: &TokenKind) -> String {
        match kind {
            TokenKind::String(s) => format!("'{s}'"),
            TokenKind::QuotedString(s) => format!("\"{s}\""),
            TokenKind::Variable(v) => format!("'${v}'"),
            TokenKind::Number(n) => format!("'{n}'"),
            TokenKind::Bool(b) => format!("'{b}'"),
            TokenKind::Let => "'let'".to_string(),
            TokenKind::If => "'if'".to_string(),
            TokenKind::Else => "'else'".to_string(),
            TokenKind::While => "'while'".to_string(),
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
            TokenKind::Colon => "':'".to_string(),
            TokenKind::NewLine => "newline".to_string(),
            TokenKind::EOF => "end of input".to_string(),
            TokenKind::Ampersand => "'&'".to_string(),
        }
    }

    fn current_token_label(&self) -> String {
        Self::token_kind_label(&self.current().kind)
    }

    fn diagnostic_here(&self, message: impl Into<String>) -> Diagnostic {
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

    fn diagnostic_here_with_hint(
        &self,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Diagnostic {
        let mut diag = self.diagnostic_here(message);
        diag.hint = Some(hint.into());
        diag
    }

    fn error_here<T>(&self, message: impl Into<String>) -> Result<T, Diagnostic> {
        Err(self.diagnostic_here(message))
    }

    fn error_here_with_hint<T>(
        &self,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Result<T, Diagnostic> {
        Err(self.diagnostic_here_with_hint(message, hint))
    }
}

#[cfg(test)]
mod tests {
    use crate::{diagnostics::DiagnosticKind, lexer::lex_input};

    use super::{
        Parser,
        ast::{BinaryOp, CommandExpr, Expr, Statement},
    };

    fn parse_program(input: &str) -> Vec<Statement> {
        let tokens = lex_input(0, input).expect("lexer should succeed in parser test");
        let mut parser = Parser::new(0, tokens);
        parser
            .parse_program()
            .expect("parser should succeed in parser test")
            .body
    }

    fn assert_command_identifier(identifier: &Expr, expected: &str) {
        assert!(
            matches!(identifier, Expr::String(actual) | Expr::QuotedString(actual) if actual == expected)
                || matches!(identifier, Expr::Word(actual) if actual == expected),
            "Expected command identifier {expected:?}, got {identifier:?}"
        );
    }

    fn extract_call_parts(statement: &Statement) -> (&Expr, &Vec<Expr>) {
        match statement {
            Statement::Command(CommandExpr::Call {
                identifier, args, ..
            }) => (identifier, args),
            Statement::Expr(Expr::Call { target, args }) => (target.as_ref(), args),
            _ => panic!("Expected call-like statement"),
        }
    }

    #[test]
    fn returns_parse_diagnostic_for_invalid_let_identifier() {
        let tokens = lex_input(0, "let = 1").expect("lexer should succeed");
        let mut parser = Parser::new(0, tokens);

        let err = parser
            .parse_program()
            .expect_err("parser should return parse diagnostic");
        assert!(matches!(err.kind, DiagnosticKind::ParseError));
        assert!(err.message.contains("Expected identifier after 'let'."));
    }

    #[test]
    fn returns_parse_diagnostic_for_unclosed_command_sub_expr() {
        let tokens = lex_input(0, "echo $(1 + 2").expect("lexer should succeed");
        let mut parser = Parser::new(0, tokens);

        let err = parser
            .parse_program()
            .expect_err("parser should return parse diagnostic");
        assert!(matches!(err.kind, DiagnosticKind::ParseError));
        assert!(err.message.contains("Expected ')' after expression"));
    }

    #[test]
    fn returns_parse_diagnostic_for_missing_redirect_target() {
        let tokens = lex_input(0, "echo hi ->").expect("lexer should succeed");
        let mut parser = Parser::new(0, tokens);

        let err = parser
            .parse_program()
            .expect_err("parser should return parse diagnostic");
        assert!(matches!(err.kind, DiagnosticKind::ParseError));
        assert!(err.message.contains("Expected redirect target after '->'."));
    }

    #[test]
    fn returns_parse_diagnostic_with_hint_for_assignment_in_expression() {
        let tokens = lex_input(0, "if 1 = 1 {}").expect("lexer should succeed");
        let mut parser = Parser::new(0, tokens);

        let err = parser
            .parse_program()
            .expect_err("parser should return parse diagnostic");
        assert!(matches!(err.kind, DiagnosticKind::ParseError));
        assert!(err.message.contains("Unexpected '=' in expression."));
        assert!(matches!(err.hint.as_deref(), Some("If you meant comparison, use '=='.")));
    }

    #[test]
    fn parses_regular_command_call() {
        let body = parse_program("echo hello world");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 2);
        assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
        assert!(matches!(&args[1], Expr::String(s) if s == "world"));
    }

    #[test]
    fn parses_pipeline_command() {
        let body = parse_program("cat file.txt | grep txt");

        let Statement::Command(CommandExpr::Pipeline(stages)) = &body[0] else {
            panic!("Expected command pipeline statement");
        };

        assert_eq!(stages.len(), 2);

        let CommandExpr::Call {
            identifier: first_id,
            args: first_args,
            ..
        } = &stages[0]
        else {
            panic!("Expected first pipeline stage to be a call");
        };
        assert_command_identifier(first_id, "cat");
        assert!(matches!(&first_args[0], Expr::String(s) if s == "file.txt"));

        let CommandExpr::Call {
            identifier: second_id,
            args: second_args,
            ..
        } = &stages[1]
        else {
            panic!("Expected second pipeline stage to be a call");
        };
        assert_command_identifier(second_id, "grep");
        assert!(matches!(&second_args[0], Expr::String(s) if s == "txt"));
    }

    #[test]
    fn parses_and_command_chain() {
        let body = parse_program("echo ok && echo done");

        let Statement::Command(CommandExpr::And { left, right }) = &body[0] else {
            panic!("Expected command and-chain");
        };

        let CommandExpr::Call {
            identifier: left_id,
            ..
        } = left.as_ref()
        else {
            panic!("Expected left side of and-chain to be a command call");
        };
        let CommandExpr::Call {
            identifier: right_id,
            ..
        } = right.as_ref()
        else {
            panic!("Expected right side of and-chain to be a command call");
        };

        assert_command_identifier(left_id, "echo");
        assert_command_identifier(right_id, "echo");
    }

    #[test]
    fn parses_command_with_env_prefix_assignment() {
        let body = parse_program("ENV_VAR=1 echo hello");

        let Statement::Command(CommandExpr::Call {
            env_assignments,
            identifier,
            args,
            ..
        }) = &body[0]
        else {
            panic!("Expected command call statement with env prefix");
        };

        assert_eq!(env_assignments.len(), 1);
        assert!(matches!(
            &env_assignments[0],
            (name, Expr::Number(n)) if name == "ENV_VAR" && (*n - 1.0).abs() < f64::EPSILON
        ));
        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
    }

    #[test]
    fn parses_command_with_multiple_env_prefix_assignments() {
        let body = parse_program("A=1 B=2 echo ok");

        let Statement::Command(CommandExpr::Call {
            env_assignments,
            identifier,
            args,
            ..
        }) = &body[0]
        else {
            panic!("Expected command call statement with env prefix");
        };

        assert_eq!(env_assignments.len(), 2);
        assert!(matches!(&env_assignments[0], (name, Expr::Number(_)) if name == "A"));
        assert!(matches!(&env_assignments[1], (name, Expr::Number(_)) if name == "B"));
        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "ok"));
    }

    #[test]
    fn parses_command_with_env_prefix_variable_value() {
        let body = parse_program("ENV_VAR=$HOME echo ok");

        let Statement::Command(CommandExpr::Call {
            env_assignments,
            identifier,
            args,
            ..
        }) = &body[0]
        else {
            panic!("Expected command call statement with env prefix");
        };

        assert_eq!(env_assignments.len(), 1);
        assert!(matches!(
            &env_assignments[0],
            (name, Expr::Variable(var)) if name == "ENV_VAR" && var == "HOME"
        ));
        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "ok"));
    }

    #[test]
    fn parses_or_command_chain() {
        let body = parse_program("missing || echo fallback");

        let Statement::Command(CommandExpr::Or { left, right }) = &body[0] else {
            panic!("Expected command or-chain");
        };

        let CommandExpr::Call {
            identifier: left_id,
            ..
        } = left.as_ref()
        else {
            panic!("Expected left side of or-chain to be a command call");
        };
        let CommandExpr::Call {
            identifier: right_id,
            ..
        } = right.as_ref()
        else {
            panic!("Expected right side of or-chain to be a command call");
        };

        assert_command_identifier(left_id, "missing");
        assert_command_identifier(right_id, "echo");
    }

    #[test]
    fn parses_pipeline_before_and_chain() {
        let body = parse_program("cat file.txt | grep txt && echo done");

        let Statement::Command(CommandExpr::And { left, right }) = &body[0] else {
            panic!("Expected top-level and-chain");
        };

        let CommandExpr::Pipeline(stages) = left.as_ref() else {
            panic!("Expected left side to be parsed as pipeline");
        };
        assert_eq!(stages.len(), 2);

        let CommandExpr::Call { identifier, .. } = right.as_ref() else {
            panic!("Expected right side to be a command call");
        };
        assert_command_identifier(identifier, "echo");
    }

    #[test]
    fn parses_command_with_colon_path_argument() {
        let body = parse_program("cat /sys/bus/pci/devices/0000:64:00.0/power/runtime_status");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "cat");
        assert_eq!(args.len(), 1);
        assert!(
            matches!(&args[0], Expr::String(s) if s == "/sys/bus/pci/devices/0000:64:00.0/power/runtime_status")
        );
    }

    #[test]
    fn parses_command_with_command_substitution_argument() {
        let body = parse_program("echo $(cat /etc/hosts)");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);

        let Expr::CommandSubCommand(sub_command) = &args[0] else {
            panic!("Expected command substitution command argument");
        };

        let CommandExpr::Call {
            identifier: sub_id,
            args: sub_args,
            ..
        } = sub_command.as_ref()
        else {
            panic!("Expected command substitution to contain a command call");
        };
        assert_command_identifier(sub_id, "cat");
        assert!(matches!(&sub_args[0], Expr::String(s) if s == "/etc/hosts"));
    }

    #[test]
    fn parses_member_access_on_command_substitution_in_expr_call_arg() {
        let body = parse_program("echo $(cat /etc/hosts).out");

        let Statement::Expr(Expr::Call { target, args }) = &body[0] else {
            panic!("Expected expression call statement");
        };

        assert!(matches!(target.as_ref(), Expr::Word(word) if word == "echo"));
        assert_eq!(args.len(), 1);

        let Expr::Index { object, index } = &args[0] else {
            panic!("Expected member access on command substitution");
        };
        assert!(matches!(index.as_ref(), Expr::String(s) if s == "out"));

        let Expr::CommandSubCommand(sub_command) = object.as_ref() else {
            panic!("Expected command substitution expression");
        };
        let CommandExpr::Call {
            identifier: sub_id,
            args: sub_args,
            ..
        } = sub_command.as_ref()
        else {
            panic!("Expected command substitution to contain a command call");
        };
        assert_command_identifier(sub_id, "cat");
        assert!(matches!(&sub_args[0], Expr::String(s) if s == "/etc/hosts"));
    }

    #[test]
    fn parses_member_access_on_command_sub_with_following_arg() {
        let body = parse_program("echo $(fmt-cwd $PWD).out $git_info");

        let (identifier, args) = extract_call_parts(&body[0]);
        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 2);

        let Expr::Index { object, index } = &args[0] else {
            panic!("Expected first arg to be member access on command substitution");
        };
        assert!(matches!(index.as_ref(), Expr::String(s) if s == "out"));
        assert!(matches!(object.as_ref(), Expr::CommandSubCommand(_)));
        assert!(matches!(&args[1], Expr::Variable(name) if name == "git_info"));
    }

    #[test]
    fn parses_command_with_dotted_path_argument() {
        let body = parse_program("source config/config.lsh");

        let (identifier, args) = extract_call_parts(&body[0]);
        assert_command_identifier(identifier, "source");
        assert_eq!(args.len(), 1);
        assert!(
            matches!(&args[0], Expr::String(s) if s == "config/config.lsh")
                || matches!(
                    &args[0],
                    Expr::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
        );
    }

    #[test]
    fn parses_command_with_spaced_dot_argument() {
        let body = parse_program("git add .");

        let (identifier, args) = extract_call_parts(&body[0]);
        assert_command_identifier(identifier, "git");
        assert_eq!(args.len(), 2);
        assert!(matches!(&args[0], Expr::String(s) if s == "add"));
        assert!(matches!(&args[1], Expr::String(s) if s == "."));
    }

    #[test]
    fn parses_command_with_contiguous_dot_path_argument() {
        let body = parse_program("ls ./examples/");

        let (identifier, args) = extract_call_parts(&body[0]);
        assert_command_identifier(identifier, "ls");
        assert_eq!(args.len(), 1);
        assert!(
            matches!(&args[0], Expr::String(s) if s == "./examples/")
                || matches!(
                    &args[0],
                    Expr::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
        );
    }

    #[test]
    fn parses_command_with_contiguous_path_identifier() {
        let body = parse_program("./examples/anonymous-functions.lsh");

        let (identifier, args) = extract_call_parts(&body[0]);
        assert_eq!(args.len(), 0);
        assert!(
            matches!(identifier, Expr::String(s) if s == "./examples/anonymous-functions.lsh")
                || matches!(
                    identifier,
                    Expr::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
        );
    }

    #[test]
    fn parses_command_with_stdout_redirection() {
        let body = parse_program("echo hello -> out.txt");

        let Statement::Command(CommandExpr::Call {
            identifier,
            args,
            stdout_redirect,
            ..
        }) = &body[0]
        else {
            panic!("Expected command call statement with redirection");
        };

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
        assert!(matches!(stdout_redirect, Some(Expr::String(s)) if s == "out.txt"));
    }

    #[test]
    fn parses_pipeline_with_last_stage_stdout_redirection() {
        let body = parse_program("echo hello | cat -> out.txt");

        let Statement::Command(CommandExpr::Pipeline(stages)) = &body[0] else {
            panic!("Expected command pipeline statement");
        };
        assert_eq!(stages.len(), 2);

        let CommandExpr::Call {
            stdout_redirect: first_redirect,
            ..
        } = &stages[0]
        else {
            panic!("Expected first stage call");
        };
        assert!(first_redirect.is_none());

        let CommandExpr::Call {
            identifier,
            stdout_redirect,
            ..
        } = &stages[1]
        else {
            panic!("Expected second stage call");
        };
        assert_command_identifier(identifier, "cat");
        assert!(matches!(stdout_redirect, Some(Expr::String(s)) if s == "out.txt"));
    }

    #[test]
    fn parses_object_literal_with_colon_separator() {
        let body = parse_program("let x = { key: value }");

        let Statement::Let { name, value } = &body[0] else {
            panic!("Expected let statement");
        };
        assert_eq!(name, "x");

        let Expr::Object(map) = value else {
            panic!("Expected object literal expression");
        };
        assert!(matches!(map.get("key"), Some(Expr::Word(value)) if value == "value"));
    }

    #[test]
    fn parses_command_with_comma_arguments_without_panicking() {
        let body = parse_program("echo hello, world");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 2);
        assert!(matches!(
            &args[0],
            Expr::Binary {
                op: BinaryOp::Add,
                left,
                right
            } if matches!(left.as_ref(), Expr::String(s) if s == "hello")
                && matches!(right.as_ref(), Expr::String(s) if s == ",")
        ));
        assert!(matches!(&args[1], Expr::String(s) if s == "world"));
    }

    #[test]
    fn parses_command_with_dash_argument_without_panicking() {
        let body = parse_program("cd -");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "cd");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "-"));
    }

    #[test]
    fn parses_command_with_url_argument() {
        let body = parse_program("echo https://example.com");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(
            matches!(&args[0], Expr::String(s) if s == "https://example.com")
                || matches!(
                    &args[0],
                    Expr::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
        );
    }

    #[test]
    fn parses_command_with_key_value_argument_tokens() {
        let body = parse_program("echo key=value");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "key=value"));
    }

    #[test]
    fn parses_command_with_multiple_equals_in_argument() {
        let body = parse_program("echo key=value=next");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "key=value=next"));
    }

    #[test]
    fn parses_command_with_unspaced_star_word_argument() {
        let body = parse_program("echo a*b");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "a*b"));
    }

    #[test]
    fn parses_command_with_variable_path_suffix_as_single_argument() {
        let body = parse_program("echo $HOME/.config");

        let (identifier, args) = extract_call_parts(&body[0]);

        assert_command_identifier(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(
            &args[0],
            Expr::Binary {
                op: BinaryOp::Add,
                left,
                right
            } if matches!(left.as_ref(), Expr::Variable(name) if name == "HOME")
                && matches!(right.as_ref(), Expr::String(s) if s == "/.config")
        ));
    }

    #[test]
    fn parses_string_concatenation_in_assignment_expression() {
        let body = parse_program("let x = \"a\" + \"b\" + \"c\"");

        let Statement::Let { value, .. } = &body[0] else {
            panic!("Expected let statement");
        };

        let Expr::Binary {
            op: BinaryOp::Add, ..
        } = value
        else {
            panic!("Expected top-level binary add expression");
        };
    }

    #[test]
    fn parses_function_declaration_target_as_variable() {
        let body = parse_program("fn add(a, b) { return $a + $b; }");

        let Statement::FunctionDeclaration { target, params, .. } = &body[0] else {
            panic!("Expected function declaration");
        };

        assert!(matches!(target, Expr::Variable(name) if name == "add"));
        assert_eq!(params, &vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parses_block_with_trailing_separator_before_right_curly() {
        let body = parse_program("if true {\n    echo ok\n}\n");

        let Statement::If { then_branch, .. } = &body[0] else {
            panic!("Expected if statement");
        };
        assert_eq!(then_branch.len(), 1);
        let (identifier, _args) = extract_call_parts(&then_branch[0]);
        assert_command_identifier(identifier, "echo");
    }

    #[test]
    fn parses_program_with_only_separators_as_empty() {
        let body = parse_program("\n;\n;;\n");
        assert!(body.is_empty());
    }

    #[test]
    fn parses_multiline_commands_with_command_substitution_separately() {
        let body = parse_program("echo \"first\" $(add 2 3).out\necho \"second\" $(add 10 5).out");
        assert_eq!(body.len(), 2);
    }

    #[test]
    fn parses_multiline_commands_with_inline_comments_separately() {
        let body = parse_program("echo hello # first comment\necho world # second comment");
        assert_eq!(body.len(), 2);
    }

    #[test]
    fn parses_command_with_multiple_command_substitutions() {
        let body = parse_program("echo $(add 1 2).out $(add 3 4).out");
        assert_eq!(body.len(), 1);
        let (_, args) = extract_call_parts(&body[0]);
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn parses_functions_example_script_into_expected_number_of_statements() {
        let input = "fn add(a, b) {\n    return $a + $b;\n}\n\nfn greet(name) {\n    return \"Hello, \" + $name\n}\n\necho \"add 2 3 ->\" $(add 2 3).out\necho \"add 10 5 ->\" $(add 10 5).out\necho $(greet \"lsh\").out\n";
        let body = parse_program(input);
        assert_eq!(body.len(), 5);
    }
}
