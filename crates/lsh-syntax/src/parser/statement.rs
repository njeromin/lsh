use crate::{
    diagnostics::{Diagnostic, SourceInfo},
    lexer::token::TokenKind,
    parser::ast::{BinaryOp, Expr, Program, SpannedStatement, Statement},
};

use super::Parser;

impl Parser<'_> {
    pub(crate) fn parse_program_unchecked(&mut self) -> Result<Program, Diagnostic> {
        let mut body = vec![];

        self.skip_statement_separators();
        while !self.check_current(&TokenKind::EOF) {
            body.push(self.parse_statement()?);
            self.skip_statement_separators();
        }

        Ok(Program { body })
    }

    pub(crate) fn parse_statement(&mut self) -> Result<SpannedStatement, Diagnostic> {
        self.skip_statement_separators();

        let start_index = self.index;
        let start = self.current().offset;
        let statement = self.parse_statement_inner()?;

        let mut last_idx = self.index.saturating_sub(1).max(start_index);
        if last_idx > start_index
            && matches!(
                self.tokens.get(last_idx).map(|t| &t.kind),
                Some(TokenKind::Semicolon)
            )
        {
            last_idx -= 1;
        }
        let end = self
            .tokens
            .get(last_idx)
            .map_or(start + 1, |t| (t.offset + t.len).max(start + 1));

        Ok(SpannedStatement {
            statement,
            span: SourceInfo {
                source_id: self.src_id,
                start,
                end,
            },
        })
    }

    fn parse_statement_inner(&mut self) -> Result<Statement, Diagnostic> {
        match &self.current().kind {
            TokenKind::Let => self.parse_let(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Break => {
                self.advance();
                self.match_token(&[TokenKind::Semicolon]);
                Ok(Statement::Break)
            }
            TokenKind::Continue => {
                self.advance();
                self.match_token(&[TokenKind::Semicolon]);
                Ok(Statement::Continue)
            }
            TokenKind::Fn => self.parse_function(),
            TokenKind::Return => self.parse_return(),
            TokenKind::LeftCurly => {
                // Handle blocks.

                self.advance(); // Skip past '{'.

                let mut statements = vec![];

                while !self.check_current(&TokenKind::RightCurly) {
                    while self.check_current(&TokenKind::NewLine)
                        || self.check_current(&TokenKind::Semicolon)
                        || self.check_current(&TokenKind::Ampersand)
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
            TokenKind::String(_)
            | TokenKind::QuotedString(_)
            | TokenKind::DoubleQuotedString(_)
            | TokenKind::Variable(_) => {
                if !self.can_start_postfix_or_assign() {
                    return Ok(Statement::Command(self.parse_command_expr()?));
                }

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
                let is_assignable = Self::is_assignable_target(&expr);

                let compound_op = match self.current().kind {
                    TokenKind::PlusEquals => Some(BinaryOp::Add),
                    TokenKind::MinusEquals => Some(BinaryOp::Sub),
                    TokenKind::StarEquals => Some(BinaryOp::Mul),
                    TokenKind::SlashEquals => Some(BinaryOp::Div),
                    _ => None,
                };

                if is_assignable
                    && (self.check_current(&TokenKind::Equals) || compound_op.is_some())
                {
                    let pre_assign_index = self.index;
                    self.advance(); // Skip '=' or compound assignment operator.
                    if let Ok(value) = self.parse_expr()
                        && self.is_statement_end()
                    {
                        return Ok(match compound_op {
                            Some(op) => Statement::CompoundAssign {
                                target: expr,
                                op,
                                value,
                            },
                            None => Statement::Assign {
                                target: expr,
                                value,
                            },
                        });
                    }
                    self.index = pre_assign_index;
                }

                // If it's a Call and the statement ends here, treat it as an expression statement.
                // Otherwise, backtrack and parse command operators (&&, ||, |, ...).
                if matches!(expr, Expr::Call { .. }) && self.is_statement_end() {
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

    fn parse_let(&mut self) -> Result<Statement, Diagnostic> {
        let name = if let TokenKind::String(name) = &self.advance().kind {
            name.as_ref().to_owned()
        } else {
            return self.error_here("Expected identifier after 'let'.");
        };

        // Advance to next token which should be an '=' and try to consume it.
        self.advance();
        self.consume(&TokenKind::Equals, "Expected '=' after variable name.")?;

        let expr = self.parse_expr()?;
        Ok(Statement::Let { name, value: expr })
    }

    pub(crate) fn parse_block_or_single_statement(
        &mut self,
    ) -> Result<Vec<SpannedStatement>, Diagnostic> {
        let spanned = self.parse_statement()?;
        Ok(match spanned.statement {
            Statement::Block(stmts) => stmts,
            statement => vec![SpannedStatement {
                statement,
                span: spanned.span,
            }],
        })
    }

    fn parse_if(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Advance past 'if'.

        let condition = self.parse_expr()?;
        let then_branch = self.parse_block_or_single_statement()?;
        let else_branch = if self.match_token(&[TokenKind::Else]) {
            Some(self.parse_block_or_single_statement()?)
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
        let body = self.parse_block_or_single_statement()?;

        Ok(Statement::While { condition, body })
    }

    fn parse_for(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Skip 'for'.

        let has_paren = self.match_token(&[TokenKind::LeftParen]);

        let variable = match &self.current().kind {
            TokenKind::String(name) | TokenKind::Variable(name) => {
                let name = name.as_ref().to_owned();
                self.advance();
                name
            }
            _ => {
                return self.error_here("Expected variable name after 'for'.");
            }
        };

        self.consume(&TokenKind::In, "Expected 'in' after loop variable.")?;

        let iterable = self.parse_expr()?;

        if has_paren {
            self.consume(&TokenKind::RightParen, "Expected ')' after for condition.")?;
        }

        let body = self.parse_block_or_single_statement()?;

        Ok(Statement::For {
            variable,
            iterable,
            body,
        })
    }

    fn parse_function(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Skip 'fn'.

        let target = self.parse_target()?;

        self.consume(&TokenKind::LeftParen, "Expected '(' after function target")?;
        let params = self.parse_param_list("Expected ')' after function parameters")?;
        let body = self.parse_block_or_single_statement()?;

        Ok(Statement::FunctionDeclaration {
            target,
            params,
            body,
        })
    }

    fn parse_return(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Skip `return`.

        let value = if self.is_statement_end() {
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

    fn is_assignable_target(expr: &Expr) -> bool {
        match expr {
            Expr::Variable(_) => true,
            Expr::Index { object, .. } => Self::is_assignable_target(object),
            _ => false,
        }
    }

    fn can_start_postfix_or_assign(&self) -> bool {
        let Some(current) = self.tokens.get(self.index) else {
            return false;
        };
        let Some(next) = self.tokens.get(self.index + 1) else {
            return false;
        };
        let contiguous = next.offset == current.offset + Self::token_source_len(current);

        match next.kind {
            TokenKind::Dot => matches!(
                self.tokens.get(self.index + 2).map(|t| &t.kind),
                Some(TokenKind::String(_))
            ),
            TokenKind::LeftParen | TokenKind::LeftBracket => contiguous,
            TokenKind::Equals
            | TokenKind::PlusEquals
            | TokenKind::MinusEquals
            | TokenKind::StarEquals
            | TokenKind::SlashEquals => matches!(current.kind, TokenKind::Variable(_)),
            _ => false,
        }
    }
}
