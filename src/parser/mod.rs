use std::collections::HashMap;

use crate::{
    lexer::token::{Token, TokenKind},
    parser::ast::{BinaryOp, CommandExpr, Expr, Program, Statement},
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

        self.skip_statement_separators();
        while !self.check_current(&TokenKind::EOF) {
            body.push(self.parse_statement());
            self.skip_statement_separators();
        }

        Program { body }
    }

    fn parse_statement(&mut self) -> Statement {
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

                    statements.push(self.parse_statement());
                }

                self.consume(&TokenKind::RightCurly, "Expected '}' after block.");

                Statement::Block(statements)
            }
            TokenKind::String(_) => Statement::Command(self.parse_command_expr()),
            _ => {
                if matches!(self.current().kind, TokenKind::Variable(_)) {
                    let initial_index = self.index;
                    let assignment_target = self.parse_postfix();

                    if self.check_current(&TokenKind::Equals) {
                        self.advance(); // Skip '='.
                        let value = self.parse_expr();
                        return Statement::Assign {
                            target: assignment_target,
                            value,
                        };
                    }

                    // Reset state if this is not an assignment.
                    self.index = initial_index;
                }

                let expr = self.parse_expr();

                Statement::Expr(expr)
            }
        }
    }

    fn parse_command_expr(&mut self) -> CommandExpr {
        self.parse_command_expr_until(&[
            TokenKind::NewLine,
            TokenKind::Semicolon,
            TokenKind::EOF,
            TokenKind::RightCurly,
        ])
    }

    fn parse_command_expr_until(&mut self, terminators: &[TokenKind]) -> CommandExpr {
        let mut expr = self.parse_command_pipeline(terminators);

        while self.check_current(&TokenKind::And) || self.check_current(&TokenKind::Or) {
            if self.check_current(&TokenKind::And) {
                self.advance(); // Skip '&&'
                let right = self.parse_command_pipeline(terminators);
                expr = CommandExpr::And {
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            } else {
                self.advance(); // Skip '||'
                let right = self.parse_command_pipeline(terminators);
                expr = CommandExpr::Or {
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            }
        }

        expr
    }

    fn parse_command_pipeline(&mut self, terminators: &[TokenKind]) -> CommandExpr {
        let mut stages = vec![self.parse_command_call(terminators)];

        while self.check_current(&TokenKind::Pipe) {
            self.advance(); // Skip '|'
            stages.push(self.parse_command_call(terminators));
        }

        if stages.len() == 1 {
            stages
                .into_iter()
                .next()
                .expect("Missing command stage in command expression.")
        } else {
            CommandExpr::Pipeline(stages)
        }
    }

    fn parse_command_call(&mut self, terminators: &[TokenKind]) -> CommandExpr {
        let identifier = match &self.current().kind {
            TokenKind::String(s) => s.clone(),
            other => panic!("Expected command identifier, found {other}"),
        };
        self.advance();

        let mut args = vec![];
        while !self.check_current(&TokenKind::Pipe)
            && !self.check_current(&TokenKind::And)
            && !self.check_current(&TokenKind::Or)
            && !terminators.iter().any(|term| self.check_current(term))
        {
            if let Some(compound_arg) = self.try_parse_command_equals_compound_arg() {
                args.push(compound_arg);
                continue;
            }

            if let Some(literal) = Self::token_kind_to_command_literal(&self.current().kind) {
                args.push(literal);
                self.advance();
                continue;
            }
            if let TokenKind::Variable(var) = &self.current().kind {
                let var_name = var.clone();
                self.advance(); // Move to token after variable.
                let sub_expr = self.parse_postfix_on_current(Expr::Variable(var_name));
                args.push(sub_expr);
                continue;
            }
            if self.check_current(&TokenKind::DollarLeftParen) {
                let sub_expr = self.parse_primary();
                let sub_expr = self.parse_postfix_on_current(sub_expr);
                args.push(sub_expr);
                continue;
            }

            break;
        }

        CommandExpr::Call { identifier, args }
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

        let target = match &self.current().kind {
            TokenKind::String(s) => Expr::Variable(s.clone()),
            TokenKind::Variable(var) => {
                let seed = Expr::Variable(var.clone());
                self.parse_postfix_from_seed(seed)
            }
            other => panic!("Expected function name, got {:?}", other),
        };
        self.advance(); // Move past function target expression.

        self.consume(&TokenKind::LeftParen, "Expected '(' after function target");

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

        Statement::FunctionDeclaration {
            target,
            params,
            body,
        }
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
        let expr = self.parse_primary();
        self.parse_postfix_on_current(expr)
    }

    fn parse_postfix_on_current(&mut self, mut expr: Expr) -> Expr {
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
        let mut should_advance = true;

        let expr = match &self.current().kind {
            TokenKind::Number(n) => Expr::Number(*n),
            TokenKind::Colon => Expr::String(":".to_string()),
            TokenKind::String(s) => {
                let call_ident = s.to_owned();
                let mut args = vec![];

                while let Some(next_token) = self.tokens.get(self.index + 1) {
                    // This should really just use some form of parse_primary/parse_expr.
                    match &next_token.kind {
                        _ if Self::token_kind_to_primary_call_arg_literal(&next_token.kind)
                            .is_some() =>
                        {
                            args.push(
                                Self::token_kind_to_primary_call_arg_literal(&next_token.kind)
                                    .expect("Expected command literal token"),
                            );
                            self.advance();
                        }
                        TokenKind::Variable(var) => {
                            let var_name = var.clone();
                            self.advance(); // Move to variable.

                            let sub_expr = self.parse_postfix_from_seed(Expr::Variable(var_name));
                            args.push(sub_expr);
                        }
                        TokenKind::DollarLeftParen => {
                            self.advance(); // Move to '$('.
                            args.push(self.parse_primary());
                        }
                        _ => break,
                    }
                }

                if args.is_empty() {
                    Expr::Word(call_ident)
                } else {
                    Expr::Call {
                        identifier: call_ident,
                        args,
                    }
                }
            }
            TokenKind::Variable(var) => Expr::Variable(var.to_owned()),
            TokenKind::Bool(value) => Expr::Bool(*value),
            TokenKind::TypeOf => {
                self.advance(); // Skip past 'typeof'.

                let expr = self.parse_expr();

                should_advance = false;
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

                if matches!(self.current().kind, TokenKind::String(_)) {
                    let cmd_expr = self.parse_command_expr_until(&[TokenKind::RightParen]);
                    self.consume(
                        &TokenKind::RightParen,
                        "Expected ')' after command substitution.",
                    );
                    should_advance = false;
                    Expr::CommandSubCommand(Box::new(cmd_expr))
                } else {
                    let expr = self.parse_expr();

                    if !self.check_current(&TokenKind::RightParen) {
                        panic!(
                            "Parser error: expected ')' after expression, found {}",
                            self.current().kind
                        );
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
                        if self.check_current(&TokenKind::Comma) {
                            self.advance();
                        } else if self.check_current(&TokenKind::NewLine)
                            || self.check_current(&TokenKind::Semicolon)
                        {
                            // Newline/semicolon separators are allowed in object literals.
                        } else {
                            self.consume(&TokenKind::Comma, "Expected ',' between object fields.");
                        }
                    }
                }
                self.advance(); // Skip past '}'.

                Expr::Object(expr_map)
            }
            other => panic!("Unexpected token in primary: {other}"),
        };

        if should_advance {
            self.advance();
        }

        expr
    }

    fn token_kind_to_command_literal(kind: &TokenKind) -> Option<Expr> {
        match kind {
            TokenKind::String(s) => Some(Expr::String(s.clone())),
            TokenKind::Number(n) => Some(Expr::Number(*n)),
            TokenKind::Bool(b) => Some(Expr::Bool(*b)),
            // Treat punctuation/operator tokens as literal string arguments in command context.
            TokenKind::Comma => Some(Expr::String(",".to_string())),
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
            TokenKind::String(s) => Some(s.clone()),
            TokenKind::Number(n) => Some(n.to_string()),
            TokenKind::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    fn token_kind_to_primary_call_arg_literal(kind: &TokenKind) -> Option<Expr> {
        match kind {
            TokenKind::String(s) => Some(Expr::String(s.clone())),
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

#[cfg(test)]
mod tests {
    use crate::lexer::lex_input;

    use super::{
        Parser,
        ast::{BinaryOp, CommandExpr, Expr, Statement},
    };

    fn parse_program(input: &str) -> Vec<Statement> {
        let tokens = lex_input(input).expect("lexer should succeed in parser test");
        let mut parser = Parser::new(tokens);
        parser.parse_program().body
    }

    #[test]
    fn parses_regular_command_call() {
        let body = parse_program("echo hello world");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "echo");
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
        } = &stages[0]
        else {
            panic!("Expected first pipeline stage to be a call");
        };
        assert_eq!(first_id, "cat");
        assert!(matches!(&first_args[0], Expr::String(s) if s == "file.txt"));

        let CommandExpr::Call {
            identifier: second_id,
            args: second_args,
        } = &stages[1]
        else {
            panic!("Expected second pipeline stage to be a call");
        };
        assert_eq!(second_id, "grep");
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

        assert_eq!(left_id, "echo");
        assert_eq!(right_id, "echo");
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

        assert_eq!(left_id, "missing");
        assert_eq!(right_id, "echo");
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
        assert_eq!(identifier, "echo");
    }

    #[test]
    fn parses_command_with_colon_path_argument() {
        let body = parse_program("cat /sys/bus/pci/devices/0000:64:00.0/power/runtime_status");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "cat");
        assert_eq!(args.len(), 1);
        assert!(
            matches!(&args[0], Expr::String(s) if s == "/sys/bus/pci/devices/0000:64:00.0/power/runtime_status")
        );
    }

    #[test]
    fn parses_command_with_command_substitution_argument() {
        let body = parse_program("echo $(cat /etc/hosts)");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "echo");
        assert_eq!(args.len(), 1);

        let Expr::CommandSubCommand(sub_command) = &args[0] else {
            panic!("Expected command substitution command argument");
        };

        let CommandExpr::Call {
            identifier: sub_id,
            args: sub_args,
        } = sub_command.as_ref()
        else {
            panic!("Expected command substitution to contain a command call");
        };
        assert_eq!(sub_id, "cat");
        assert!(matches!(&sub_args[0], Expr::String(s) if s == "/etc/hosts"));
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

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "echo");
        assert_eq!(args.len(), 3);
        assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
        assert!(matches!(&args[1], Expr::String(s) if s == ","));
        assert!(matches!(&args[2], Expr::String(s) if s == "world"));
    }

    #[test]
    fn parses_command_with_dash_argument_without_panicking() {
        let body = parse_program("cd -");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "cd");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "-"));
    }

    #[test]
    fn parses_command_with_url_argument() {
        let body = parse_program("echo https://example.com");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "https://example.com"));
    }

    #[test]
    fn parses_command_with_key_value_argument_tokens() {
        let body = parse_program("echo key=value");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "key=value"));
    }

    #[test]
    fn parses_command_with_multiple_equals_in_argument() {
        let body = parse_program("echo key=value=next");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "key=value=next"));
    }

    #[test]
    fn parses_command_with_unspaced_star_word_argument() {
        let body = parse_program("echo a*b");

        let Statement::Command(CommandExpr::Call { identifier, args }) = &body[0] else {
            panic!("Expected command call statement");
        };

        assert_eq!(identifier, "echo");
        assert_eq!(args.len(), 1);
        assert!(matches!(&args[0], Expr::String(s) if s == "a*b"));
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
    fn parses_block_with_trailing_separator_before_right_curly() {
        let body = parse_program("if true {\n    echo ok\n}\n");

        let Statement::If { then_branch, .. } = &body[0] else {
            panic!("Expected if statement");
        };
        assert_eq!(then_branch.len(), 1);
        assert!(matches!(
            &then_branch[0],
            Statement::Command(CommandExpr::Call { identifier, .. }) if identifier == "echo"
        ));
    }

    #[test]
    fn parses_program_with_only_separators_as_empty() {
        let body = parse_program("\n;\n;;\n");
        assert!(body.is_empty());
    }
}
