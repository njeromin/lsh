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
    Call {
        env_assignments: Vec<(String, Expr)>,
        identifier: Expr,
        args: Vec<Expr>,
        stdout_redirect: Option<Expr>,
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
}

#[derive(Debug, Clone)]
pub enum Expr {
    // Data,
    Number(f64),
    Bool(bool),
    String(String),
    QuotedString(String),
    Object(HashMap<String, Expr>),
    Array(Vec<Expr>),
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
        body: Vec<Statement>,
    },
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
