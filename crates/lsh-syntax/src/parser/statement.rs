use crate::{
    diagnostics::Diagnostic,
    lexer::token::TokenKind,
    parser::ast::{Expr, Program, Statement},
};

use super::Parser;

impl Parser {
    pub(crate) fn parse_program_unchecked(&mut self) -> Result<Program, Diagnostic> {
        let mut body = vec![];

        self.skip_statement_separators();
        while !self.check_current(&TokenKind::EOF) {
            body.push(self.parse_statement()?);
            self.skip_statement_separators();
        }

        Ok(Program { body })
    }

    pub(crate) fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        self.skip_statement_separators();

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
                let is_assignable = match &expr {
                    Expr::Variable(_) | Expr::Index { .. } => true,
                    Expr::Call { target, args } if args.is_empty() => {
                        matches!(target.as_ref(), Expr::Word(_))
                    }
                    _ => false,
                };

                if self.check_current(&TokenKind::Equals) && is_assignable {
                    let pre_assign_index = self.index;
                    self.advance(); // Skip '='.
                    if let Ok(value) = self.parse_expr() {
                        let statement_ends = self.check_current(&TokenKind::Semicolon)
                            || self.check_current(&TokenKind::NewLine)
                            || self.check_current(&TokenKind::EOF)
                            || self.check_current(&TokenKind::RightCurly);

                        if statement_ends {
                            let target = match expr {
                                Expr::Call { target, args } if args.is_empty() => match *target {
                                    Expr::Word(name) => Expr::Variable(name),
                                    other => other,
                                },
                                other => other,
                            };
                            return Ok(Statement::Assign { target, value });
                        }
                    }
                    self.index = pre_assign_index;
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

    fn parse_for(&mut self) -> Result<Statement, Diagnostic> {
        self.advance(); // Skip 'for'.

        let has_paren = self.match_token(&[TokenKind::LeftParen]);

        let variable = match &self.current().kind {
            TokenKind::String(name) => {
                let name = name.clone();
                self.advance();
                name
            }
            TokenKind::Variable(name) => {
                let name = name.clone();
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

        let body = match self.parse_statement()? {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

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
}
