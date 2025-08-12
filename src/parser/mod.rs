use std::mem;

use crate::{
    lexer::token::{Token, TokenKind},
    parser::ast::{BinaryOp, Expr, Program, Statement},
};

pub mod ast;

fn cmp_kinds(kind_x: &TokenKind, kind_y: &TokenKind) -> bool {
    mem::discriminant(kind_x) == mem::discriminant(kind_y)
}

pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    pub fn parse_program(&mut self) -> Program {
        let mut body = vec![];

        while !self.check_current(&TokenKind::EOF) {
            body.push(self.parse_statement());
        }

        Program { body }
    }

    fn parse_statement(&mut self) -> Statement {
        match &self.current().kind {
            TokenKind::Let => self.parse_let(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::LeftCurly => {
                // Handle blocks.

                let mut statements = vec![];

                while !self.check_current(&TokenKind::RightCurly) {
                    statements.push(self.parse_statement());
                }

                self.consume(&TokenKind::RightCurly, "Expected '}' after block.");

                Statement::Block(statements)
            }
            _ => {
                let expr = self.parse_expr();

                Statement::Expr(expr)
            }
        }
    }

    fn parse_let(&mut self) -> Statement {
        let name = if let TokenKind::String(name) = self.advance().kind.clone() {
            name
        } else {
            panic!("Expected identifier after 'let'");
        };

        // Advance to next token which should be an '=' and try to consume it.
        self.advance();
        self.consume(&TokenKind::Equals, "Expected '=' after variable name.");

        let expr = self.parse_expr();
        Statement::Let { name, value: expr }
    }

    fn parse_if(&mut self) -> Statement {
        self.advance();

        self.consume(&TokenKind::LeftParen, "Expected '(' after 'if'");
        let condition = self.parse_expr();
        self.consume(&TokenKind::RightParen, "Expected ')' after condition");

        let then_branch = match self.parse_statement() {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        let else_branch = if self.match_token(&[TokenKind::Else]) {
            let branch = match self.parse_statement() {
                Statement::Block(stmts) => stmts,
                stmt => vec![stmt],
            };
            Some(branch)
        } else {
            None
        };

        Statement::If {
            condition,
            then_branch,
            else_branch,
        }
    }

    fn parse_while(&mut self) -> Statement {
        self.advance();

        self.consume(&TokenKind::LeftParen, "Expected '(' after 'while'");
        let condition = self.parse_expr();
        self.consume(&TokenKind::RightParen, "Expected ')' after condition");

        let body = match self.parse_statement() {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        Statement::While { condition, body }
    }

    fn parse_for(&mut self) -> Statement {
        self.advance();

        self.consume(&TokenKind::LeftParen, "Expected '(' after 'for'");
        let condition = self.parse_expr();
        self.consume(&TokenKind::RightParen, "Expected ')' after condition");

        let body = match self.parse_statement() {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        Statement::For { condition, body }
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Expr {
        let mut expr = self.parse_and();

        while self.match_token(&[TokenKind::Or]) {
            let right = self.parse_and();
            expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }

        expr
    }

    fn parse_and(&mut self) -> Expr {
        let mut expr = self.parse_equality();

        while self.match_token(&[TokenKind::And]) {
            let right = self.parse_equality();
            expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }

        expr
    }

    fn parse_equality(&mut self) -> Expr {
        let mut expr = self.parse_comparison();
        while let Some(op) = match self.current().kind {
            TokenKind::EqualsEquals => Some(BinaryOp::Eq),
            TokenKind::NotEquals => Some(BinaryOp::Neq),
            _ => None,
        } {
            self.advance();
            let right = self.parse_comparison();
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        expr
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut expr = self.parse_term();
        while let Some(op) = match self.current().kind {
            TokenKind::LessThan => Some(BinaryOp::Lt),
            TokenKind::GreaterThan => Some(BinaryOp::Gt),
            TokenKind::LtEquals => Some(BinaryOp::Le),
            TokenKind::GtEquals => Some(BinaryOp::Ge),
            _ => None,
        } {
            self.advance();
            let right = self.parse_term();
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        expr
    }

    fn parse_term(&mut self) -> Expr {
        let mut expr = self.parse_factor();
        while let Some(op) = match self.current().kind {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        } {
            self.advance();
            let right = self.parse_factor();
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        expr
    }

    fn parse_factor(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        while let Some(op) = match self.current().kind {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            _ => None,
        } {
            self.advance();
            let right = self.parse_primary();
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let expr = match &self.current().kind {
            TokenKind::Number(n) => Expr::Number(*n),
            // NOTE: Do strings actually have to be handled separately to calls?
            TokenKind::String(s) => {
                let call_ident = s.to_owned();
                let mut args = vec![];

                // NOTE: This could just be peek?
                while let Some(next_tok) = self.tokens.get(self.index + 1)
                    && !cmp_kinds(&next_tok.kind, &TokenKind::EOF)
                {
                    let sub_expr = match &next_tok.kind {
                        TokenKind::String(s) => Expr::String(s.to_owned()),
                        TokenKind::Number(n) => Expr::Number(*n),
                        TokenKind::Bool(b) => Expr::Bool(*b),
                        _ => break,
                    };

                    args.push(sub_expr);

                    self.advance();
                }

                Expr::Call {
                    identifier: call_ident,
                    args,
                }
            }
            TokenKind::Bool(value) => Expr::Bool(*value),
            TokenKind::LeftParen => {
                let expr = self.parse_expr();
                self.consume(&TokenKind::RightParen, "Expected ')' after expression.");
                expr
            }
            other => panic!("Unexpected token in primary: {other}"),
        };

        self.advance();

        expr
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("Failed to get current token."))
    }

    fn advance(&mut self) -> &Token {
        // Only index if it is possible.
        if self.index < self.tokens.len() {
            self.index += 1;
        }

        self.current()
    }

    fn check_current(&self, kind_y: &TokenKind) -> bool {
        cmp_kinds(&self.current().kind, kind_y)
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

    fn is_valid_arg(&mut self, kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::String(_) | TokenKind::Number(_) | TokenKind::Bool(_)
        )
    }

    fn consume(&mut self, kind: &TokenKind, panic_msg: &str) -> Token {
        if self.check_current(kind) {
            return self.advance().to_owned();
        }

        let current_kind = &self.current().kind;

        panic!("Parser error: expected {kind}, found {current_kind} - {panic_msg}");
    }
}
