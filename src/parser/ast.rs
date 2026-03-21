use std::collections::HashMap;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum CommandExpr {
    Call { identifier: String, args: Vec<Expr> },
    Pipeline(Vec<CommandExpr>),
}

#[derive(Debug, Clone)]
pub enum Expr {
    // Data,
    Number(f64),
    Bool(bool),
    String(String),
    Object(HashMap<String, Expr>),
    Variable(String),

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
    // cmd arg1 arg2...
    CallOrString {
        identifier: String,
        args: Vec<Expr>,
    },
    TypeOf(Box<Expr>),
    CommandSub(Box<Expr>),
    CommandSubCommand(Box<CommandExpr>),
}

#[derive(Debug, Clone)]
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
    If {
        condition: Expr,
        then_branch: Vec<Statement>,
        else_branch: Option<Vec<Statement>>,
    },
    While {
        condition: Expr,
        body: Vec<Statement>,
    },
    FunctionDeclaration {
        target: Expr,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    Return(Option<Expr>),
    Block(Vec<Statement>),
}

#[derive(Debug)]
pub struct Program {
    pub body: Vec<Statement>,
}
