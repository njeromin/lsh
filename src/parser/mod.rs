use std::collections::HashMap;

use crate::{
    lexer::token::{Token, TokenKind},
    parser::ast::{BinaryOp, Expr, Program, Statement},
};

pub mod ast;

pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    /// Parses all provided tokens and returns a program which can be evaluated by an Evaluator.
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
            TokenKind::Fn => self.parse_function(),
            TokenKind::Return => self.parse_return(),
            TokenKind::LeftCurly => {
                // Handle blocks.

                self.advance(); // Skip past '{'.

                let mut statements = vec![];

                while !self.check_current(&TokenKind::RightCurly) {
                    statements.push(self.parse_statement());
                }

                self.consume(&TokenKind::RightCurly, "Expected '}' after block.");

                Statement::Block(statements)
            }
            TokenKind::NewLine | TokenKind::Semicolon => {
                self.advance(); // Skip newline.
                self.parse_statement()
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
            panic!("Expected identifier after 'let'.");
        };

        // Advance to next token which should be an '=' and try to consume it.
        self.advance();
        self.consume(&TokenKind::Equals, "Expected '=' after variable name.");

        let expr = self.parse_expr();
        Statement::Let { name, value: expr }
    }

    fn parse_if(&mut self) -> Statement {
        self.advance(); // Advance past 'if'.

        let condition = self.parse_expr();

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

        let condition = self.parse_expr();

        let body = match self.parse_statement() {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        Statement::While { condition, body }
    }

    fn parse_function(&mut self) -> Statement {
        self.advance(); // Skip 'fn'.

        let name = match &self.current().kind {
            TokenKind::String(s) => s.clone(),
            other => panic!("Expected function name, got {:?}", other),
        };
        self.advance(); // Move past the function name.

        self.consume(&TokenKind::LeftParen, "Expected '(' after function name");

        let mut params = vec![];

        if !self.check_current(&TokenKind::RightParen) {
            loop {
                match &self.current().kind {
                    TokenKind::String(p) => params.push(p.clone()),
                    other => panic!("Expected parameter name, found {other}"),
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
        );

        let body = match self.parse_statement() {
            Statement::Block(stmts) => stmts,
            stmt => vec![stmt],
        };

        Statement::FunctionDeclaration { name, params, body }
    }

    fn parse_return(&mut self) -> Statement {
        self.advance(); // Skip `return`.

        let value = if self.check_current(&TokenKind::Semicolon)
            || self.check_current(&TokenKind::NewLine)
            || self.check_current(&TokenKind::RightCurly)
            || self.check_current(&TokenKind::EOF)
        {
            None
        } else {
            Some(self.parse_expr())
        };

        // Optional semicolon after return value (or bare return).
        if self.check_current(&TokenKind::Semicolon) {
            self.advance();
        }

        Statement::Return(value)
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
        let mut expr = self.parse_postfix();
        while let Some(op) = match self.current().kind {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            _ => None,
        } {
            self.advance();
            let right = self.parse_postfix();
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        expr
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();

        loop {
            if self.check_current(&TokenKind::LeftBracket) {
                self.advance(); // Skip '['.

                let index_expr = self.parse_expr();

                self.consume(
                    &TokenKind::RightBracket,
                    "Expected ']' after object index expression.",
                );

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index_expr),
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

        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let expr = match &self.current().kind {
            TokenKind::Number(n) => Expr::Number(*n),
            TokenKind::String(s) => {
                let call_ident = s.to_owned();
                let mut args = vec![];

                while let Some(next_kind) = self.tokens.get(self.index + 1).map(|t| t.kind.clone())
                {
                    // This should really just use some form of parse_primary/parse_expr.
                    match next_kind {
                        TokenKind::String(s) => {
                            args.push(Expr::String(s));
                            self.advance();
                        }
                        TokenKind::Number(n) => {
                            args.push(Expr::Number(n));
                            self.advance();
                        }
                        TokenKind::Bool(b) => {
                            args.push(Expr::Bool(b));
                            self.advance();
                        }
                        TokenKind::Star => {
                            args.push(Expr::String("*".to_string()));
                            self.advance();
                        }
                        TokenKind::Slash => {
                            args.push(Expr::String("/".to_string()));
                            self.advance();
                        }
                        TokenKind::Variable(var) => {
                            self.advance(); // Move to variable.

                            let sub_expr = self.parse_postfix_from_seed(Expr::Variable(var));
                            args.push(sub_expr);
                        }
                        TokenKind::DollarLeftParen => {
                            self.advance(); // Move to '$('.
                            args.push(self.parse_primary());
                        }
                        _ => break,
                    }
                }

                Expr::CallOrString {
                    identifier: call_ident,
                    args,
                }
            }
            TokenKind::Variable(var) => Expr::Variable(var.to_owned()),
            TokenKind::Bool(value) => Expr::Bool(*value),
            TokenKind::TypeOf => {
                self.advance(); // Skip past 'typeof'.

                let expr = self.parse_expr();

                Expr::TypeOf(Box::new(expr))
            }
            TokenKind::LeftParen => {
                self.advance(); // Skip past '('.

                let expr = self.parse_expr();

                if !self.check_current(&TokenKind::RightParen) {
                    panic!("Parser error: expected ')', found {}", self.current().kind);
                }

                expr
            }
            TokenKind::DollarLeftParen => {
                self.advance(); // Skip past '$('.

                let expr = self.parse_expr();

                if !self.check_current(&TokenKind::RightParen) {
                    panic!(
                        "Parser error: expected ')' after expression, found {}",
                        self.current().kind
                    );
                }

                Expr::CommandSub(Box::new(expr))
            }
            TokenKind::LeftCurly => {
                self.advance(); // Skip past '{'.

                let mut expr_map = HashMap::new();
                while !self.check_current(&TokenKind::RightCurly) {
                    let key = match &self.current().kind {
                        TokenKind::String(s) => s.clone(),
                        TokenKind::Variable(v) => v.clone(),
                        TokenKind::Number(n) => n.to_string(),
                        _ => panic!("Expected object key"),
                    };

                    self.advance();
                    self.consume(&TokenKind::Colon, "Expected ':' after object key");

                    let value = self.parse_expr();

                    expr_map.insert(key, value);

                    if !self.check_current(&TokenKind::RightCurly) {
                        self.consume(&TokenKind::Comma, "Expected ',' between object fields.");
                    }
                }
                self.advance(); // Skip past '}'.

                Expr::Object(expr_map)
            }
            TokenKind::NewLine | TokenKind::Semicolon => {
                let expr = self.parse_expr();

                self.advance();

                expr
            }
            other => panic!("Unexpected token in primary: {other}"),
        };

        self.advance();

        expr
    }

    fn parse_postfix_from_seed(&mut self, mut expr: Expr) -> Expr {
        loop {
            if self
                .tokens
                .get(self.index + 1)
                .is_some_and(|tok| tok.kind.compare(&TokenKind::LeftBracket))
            {
                self.advance(); // Move to '['.
                self.advance(); // Skip '['.

                let index_expr = self.parse_expr();

                self.consume(
                    &TokenKind::RightBracket,
                    "Expected ']' after object index expression.",
                );

                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index_expr),
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
        self.current().kind.compare(kind_y)
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

    fn consume(&mut self, kind: &TokenKind, panic_msg: &str) -> Token {
        if self.check_current(kind) {
            return self.advance().to_owned();
        }

        let current_kind = &self.current().kind;

        panic!("Parser error: expected {kind}, found {current_kind} - {panic_msg}");
    }
}
