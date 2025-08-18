#[derive(Debug)]
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

#[derive(Debug)]
pub enum Expr {
    // Data,
    Number(f64),
    Bool(bool),
    String(String),
    Variable(String),

    // a + b, a - b, ...
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    // cmd arg1 arg2...
    Call {
        identifier: String,
        args: Vec<Expr>,
    },
    Typeof(Box<Expr>),
}

#[derive(Debug)]
pub enum Statement {
    Expr(Expr),
    Let {
        name: String,
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
    For {
        condition: Expr,
        body: Vec<Statement>,
    },
    Block(Vec<Statement>),
}

#[derive(Debug)]
pub struct Program {
    pub body: Vec<Statement>,
}
