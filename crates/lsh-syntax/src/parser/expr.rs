use std::{collections::HashMap, ops::ControlFlow};

use crate::{
    diagnostics::Diagnostic,
    lexer::token::TokenKind,
    parser::ast::{BinaryOp, Expr, UnaryOp},
};

use super::Parser;

impl Parser<'_> {
    pub(crate) fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
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
        let mut expr = self.parse_unary()?;
        while let Some(op) = match self.current().kind {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            _ => None,
        } {
            self.advance();
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.match_token(&[TokenKind::Not]) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.match_token(&[TokenKind::Minus]) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            });
        }

        self.parse_postfix()
    }

    pub(crate) fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.parse_primary()?;
        self.parse_postfix_on_current(expr)
    }

    pub(crate) fn parse_target(&mut self) -> Result<Expr, Diagnostic> {
        let expr = match &self.current().kind {
            TokenKind::String(name) => {
                let name = name.as_ref().to_owned();
                self.advance();
                Expr::Variable(name)
            }
            _ => match self.parse_primary()? {
                Expr::Word(name) => Expr::Variable(name),
                other => other,
            },
        };
        self.parse_target_on_current(expr)
    }

    fn parse_index_or_dot_postfix(
        &mut self,
        expr: Expr,
        require_contiguous_bracket: bool,
    ) -> Result<ControlFlow<Expr, Expr>, Diagnostic> {
        if self.check_current(&TokenKind::LeftBracket) {
            if require_contiguous_bracket && !self.is_contiguous_with_prev() {
                return Ok(ControlFlow::Break(expr));
            }

            self.advance(); // Skip '['.
            let index_expr = self.parse_expr()?;
            self.consume(
                &TokenKind::RightBracket,
                "Expected ']' after object index expression.",
            )?;

            return Ok(ControlFlow::Continue(Expr::Index {
                object: Box::new(expr),
                index: Box::new(index_expr),
            }));
        }

        if self.check_current(&TokenKind::Dot) {
            self.advance(); // Skip '.'.
            let key = match &self.current().kind {
                TokenKind::String(s) => s.as_ref().to_owned(),
                other => {
                    return self.error_here(format!(
                        "Expected identifier after '.', found {}.",
                        Self::token_kind_label(other)
                    ));
                }
            };
            self.advance();

            return Ok(ControlFlow::Continue(Expr::Index {
                object: Box::new(expr),
                index: Box::new(Expr::String(key)),
            }));
        }

        Ok(ControlFlow::Break(expr))
    }

    fn parse_target_on_current(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            match self.parse_index_or_dot_postfix(expr, false)? {
                ControlFlow::Continue(next_expr) => expr = next_expr,
                ControlFlow::Break(done_expr) => return Ok(done_expr),
            }
        }
    }

    pub(crate) fn parse_postfix_on_current(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if self.check_current(&TokenKind::LeftParen) {
                // Ensure no whitespace between identifier and '(' for it to be a call.
                if !self.is_contiguous_with_prev() {
                    break;
                }

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
                match self.parse_index_or_dot_postfix(expr, true)? {
                    ControlFlow::Continue(next_expr) => expr = next_expr,
                    ControlFlow::Break(done_expr) => return Ok(done_expr),
                }
            }
        }

        Ok(expr)
    }

    pub(crate) fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let mut should_advance = true;

        let expr = match &self.current().kind {
            TokenKind::Number(n) => Expr::Number(*n),
            TokenKind::QuotedString(s) => Expr::QuotedString(s.as_ref().to_owned()),
            TokenKind::DoubleQuotedString(s) => {
                Self::parse_interpolated_string(self.src_id, s, self.current().offset)?
            }
            TokenKind::String(s) => {
                // Check if this is the start of an anonymous function: x => ...
                if self.check_peek(1, &TokenKind::FatArrow) {
                    should_advance = false;
                    self.parse_anonymous_function()?
                } else {
                    Expr::Word(s.as_ref().to_owned())
                }
            }
            TokenKind::Variable(var) => Expr::Variable(var.as_ref().to_owned()),
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

                if matches!(
                    self.current().kind,
                    TokenKind::String(_)
                        | TokenKind::QuotedString(_)
                        | TokenKind::DoubleQuotedString(_)
                ) {
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
                        || self.check_current(&TokenKind::Ampersand)
                    {
                        self.advance();
                    }

                    if self.check_current(&TokenKind::RightCurly) {
                        break;
                    }

                    let key = match &self.current().kind {
                        TokenKind::String(s)
                        | TokenKind::QuotedString(s)
                        | TokenKind::DoubleQuotedString(s) => s.as_ref().to_owned(),
                        TokenKind::Variable(v) => v.as_ref().to_owned(),
                        TokenKind::Number(n) => n.to_string(),
                        _ => return self.error_here("Expected object key"),
                    };

                    self.advance();
                    self.consume(&TokenKind::Equals, "Expected '=' after object key")?;

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
                self.consume(&TokenKind::RightCurly, "Expected '}' after object.")?;
                should_advance = false;

                Expr::Object(expr_map)
            }
            TokenKind::LeftBracket => {
                self.advance(); // Skip past '['.

                let mut elements = Vec::new();
                while !self.check_current(&TokenKind::RightBracket) {
                    while self.check_current(&TokenKind::NewLine)
                        || self.check_current(&TokenKind::Semicolon)
                    {
                        self.advance();
                    }

                    if self.check_current(&TokenKind::RightBracket) {
                        break;
                    }

                    let elem = self.parse_expr()?;
                    elements.push(elem);

                    while self.check_current(&TokenKind::NewLine)
                        || self.check_current(&TokenKind::Semicolon)
                    {
                        self.advance();
                    }

                    if !self.check_current(&TokenKind::RightBracket) {
                        if self.check_current(&TokenKind::Comma) {
                            self.advance();
                        } else if self.check_current(&TokenKind::NewLine)
                            || self.check_current(&TokenKind::Semicolon)
                        {
                            // Newline/semicolon separators are allowed in array literals.
                        } else {
                            self.consume(
                                &TokenKind::Comma,
                                "Expected ',' between array elements.",
                            )?;
                        }
                    }
                }
                self.consume(
                    &TokenKind::RightBracket,
                    "Expected ']' after array elements.",
                )?;
                should_advance = false;

                Expr::Array(elements)
            }
            TokenKind::ThinArrow => {
                return self.error_here(
                    "Unexpected '->' here. Redirection is only valid after a command.",
                );
            }
            TokenKind::ThinArrowAppend => {
                return self.error_here(
                    "Unexpected '->>' here. Redirection is only valid after a command.",
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

    fn parse_anonymous_function(&mut self) -> Result<Expr, Diagnostic> {
        let params = if self.check_current(&TokenKind::LeftParen) {
            self.advance(); // Skip '('.
            self.parse_param_list("Expected ')' after parameters.")?
        } else {
            // Single parameter without parens: x => ...
            let param = match &self.current().kind {
                TokenKind::String(p) => p.as_ref().to_owned(),
                other => {
                    return self.error_here(format!(
                        "Expected parameter name, found {}.",
                        Self::token_kind_label(other)
                    ));
                }
            };
            self.advance();
            vec![param]
        };

        self.consume(&TokenKind::FatArrow, "Expected '=>' after parameters.")?;

        if self.check_current(&TokenKind::EOF) {
            return self.error_here(
                "Expected function body after '=>'. Anonymous functions need an expression or block.",
            );
        }

        let body = self.parse_block_or_single_statement()?;

        Ok(Expr::AnonymousFunction { params, body })
    }
}
