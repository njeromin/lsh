use std::collections::HashMap;

use crate::{
    diagnostics::Diagnostic,
    lexer::token::{Token, TokenKind},
    parser::ast::{BinaryOp, Expr, Statement},
};

use super::Parser;

impl Parser {
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

    pub(crate) fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.parse_primary()?;
        self.parse_postfix_on_current(expr)
    }

    pub(crate) fn parse_target(&mut self) -> Result<Expr, Diagnostic> {
        let expr = match &self.current().kind {
            TokenKind::String(name) => {
                let name = name.clone();
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

    pub(crate) fn parse_postfix_on_current(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if self.check_current(&TokenKind::LeftBracket) {
                // Ensure no whitespace between expression and '[' for it to be an index.
                let prev_token = &self.tokens[self.index - 1];
                let current_token = self.current();

                if current_token.offset != prev_token.offset + Self::token_source_len(prev_token) {
                    break;
                }

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

    pub(crate) fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let mut should_advance = true;

        let expr = match &self.current().kind {
            TokenKind::Number(n) => Expr::Number(*n),
            TokenKind::QuotedString(s) => Expr::QuotedString(s.clone()),
            TokenKind::DoubleQuotedString(s) => {
                Self::parse_interpolated_string(self.src_id, s, self.current().offset)?
            }
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
                        let is_contiguous =
                            next_token.offset == current.offset + Self::token_source_len(&current);

                        if matches!(next_token.kind, TokenKind::Star | TokenKind::Slash)
                            && !is_contiguous
                        {
                            break;
                        }

                        // This should really just use some form of parse_primary/parse_expr.
                        if let TokenKind::DoubleQuotedString(raw) = &next_token.kind {
                            let parsed = Self::parse_interpolated_string(
                                self.src_id,
                                raw,
                                next_token.offset,
                            )?;
                            self.advance(); // Move to literal.

                            if is_contiguous && !args.is_empty() {
                                let previous = args
                                    .pop()
                                    .expect("Expected previous argument while concatenating.");
                                args.push(Expr::Binary {
                                    op: BinaryOp::Add,
                                    left: Box::new(previous),
                                    right: Box::new(parsed),
                                });
                            } else {
                                args.push(parsed);
                            }
                        } else if let Some(arg_expr) =
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

                                let part_expr = if let TokenKind::DoubleQuotedString(raw) =
                                    &next_part.kind
                                {
                                    Self::parse_interpolated_string(
                                        self.src_id,
                                        raw,
                                        next_part.offset,
                                    )?
                                } else if let Some(part_expr) =
                                    Self::token_kind_to_primary_call_arg_literal(&next_part.kind)
                                {
                                    part_expr
                                } else {
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

                                if let TokenKind::DoubleQuotedString(raw) = &self.current().kind {
                                    let parsed = Self::parse_interpolated_string(
                                        self.src_id,
                                        raw,
                                        self.current().offset,
                                    )?;
                                    args.push(parsed);
                                    break;
                                } else if let Some(arg_expr) =
                                    Self::token_kind_to_primary_call_arg_literal(
                                        &self.current().kind,
                                    )
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
                            let current = self.current().to_owned();
                            let is_contiguous = next_token.offset
                                == current.offset + Self::token_source_len(&current);

                            if is_contiguous && args.is_empty() {
                                break;
                            }

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
                        | TokenKind::DoubleQuotedString(s) => s.clone(),
                        TokenKind::Variable(v) => v.clone(),
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

    fn token_kind_to_primary_call_arg_literal(kind: &TokenKind) -> Option<Expr> {
        match kind {
            TokenKind::String(s) => Some(Expr::String(s.clone())),
            TokenKind::QuotedString(s) => Some(Expr::QuotedString(s.clone())),
            TokenKind::Number(n) => Some(Expr::Number(*n)),
            TokenKind::Bool(b) => Some(Expr::Bool(*b)),
            // Keep these for command-like expression args only.
            TokenKind::Star => Some(Expr::String("*".to_string())),
            TokenKind::Slash => Some(Expr::String("/".to_string())),
            _ => None,
        }
    }

    fn parse_postfix_from_seed(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if let Some(next_tok) = self.tokens.get(self.index + 1)
                && next_tok.kind.compare(&TokenKind::LeftBracket)
            {
                let cur_tok = &self.tokens[self.index];
                if next_tok.offset != cur_tok.offset + Self::token_source_len(cur_tok) {
                    break;
                }

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

}
