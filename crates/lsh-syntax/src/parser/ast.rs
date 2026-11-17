use std::{collections::HashMap, ops::Deref};

use crate::diagnostics::SourceInfo;

#[derive(Debug, Clone, PartialEq)]
pub struct Redirect {
    pub target: Expr,
    pub append: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandExpr {
    Call {
        env_assignments: Vec<(String, Expr)>,
        identifier: Expr,
        args: Vec<Expr>,
        stdout_redirect: Option<Redirect>,
    },
    Pipeline(Vec<CommandExpr>),
    And {
        left: Box<CommandExpr>,
        right: Box<CommandExpr>,
    },
    Or {
        left: Box<CommandExpr>,
        right: Box<CommandExpr>,
    },
    Background(Box<CommandExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Data,
    Number(f64),
    Bool(bool),
    String(String),
    QuotedString(String),
    InterpolatedString(Vec<Expr>),
    Object(HashMap<String, Expr>),
    Array(Vec<Expr>),
    Variable(String),

    // !a, -a
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    // a + b, a - b, ...
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    // Bare identifier/token used in expression context.
    Word(String),
    // call arg1 arg2...
    Call {
        target: Box<Expr>,
        args: Vec<Expr>,
    },
    TypeOf(Box<Expr>),
    CommandSub(Box<Expr>),
    CommandSubCommand(Box<CommandExpr>),
    AnonymousFunction {
        params: Vec<String>,
        body: Vec<SpannedStatement>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedStatement {
    pub statement: Statement,
    pub span: SourceInfo,
}

impl Deref for SpannedStatement {
    type Target = Statement;

    fn deref(&self) -> &Self::Target {
        &self.statement
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Expr(Expr),
    Command(CommandExpr),
    Let {
        name: String,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    CompoundAssign {
        target: Expr,
        op: BinaryOp,
        value: Expr,
    },
    If {
        condition: Expr,
        then_branch: Vec<SpannedStatement>,
        else_branch: Option<Vec<SpannedStatement>>,
    },
    While {
        condition: Expr,
        body: Vec<SpannedStatement>,
    },
    For {
        variable: String,
        iterable: Expr,
        body: Vec<SpannedStatement>,
    },
    Break,
    Continue,
    FunctionDeclaration {
        target: Expr,
        params: Vec<String>,
        body: Vec<SpannedStatement>,
    },
    Return(Option<Expr>),
    Block(Vec<SpannedStatement>),
}

#[derive(Debug)]
pub struct Program {
    pub body: Vec<SpannedStatement>,
}
