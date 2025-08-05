pub mod value;

use std::collections::HashMap;

use crate::{
    eval::value::Value,
    parser::ast::{Expr, Program, Statement},
};

#[derive(Default)]
pub struct Scope {
    vars: HashMap<String, Value>,
}

pub struct Evaluator {
    global_scope: Scope,
    scopes: Vec<Scope>,
}

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator {
            global_scope: Scope::default(),
            scopes: vec![],
        }
    }

    pub fn execute(&mut self, prog: Program) {
        self.execute_statements(prog.body, None);
    }

    fn execute_statements(&mut self, statements: Vec<Statement>, scope: Option<&mut Scope>) {
        for statement in statements {
            match statement {
                Statement::Expr(expr) => {}
                Statement::Let { name, value } => {
                    if let Some(some_scope) = scope {
                        let val = self.evaluate_expr(value, Some(&some_scope));
                        some_scope.vars.insert(name, val);
                    } else {
                        let val = self.evaluate_expr(value, None);
                        self.global_scope.vars.insert(name, val);
                    }
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {}
                Statement::While { condition, body } => {}
                Statement::For { condition, body } => {}
                Statement::Block(block) => {
                    let mut block_scope = Scope::default();

                    self.execute_statements(block, Some(&mut block_scope));
                }
            }
        }
    }

    fn evaluate_expr(&self, expr: Expr, scope: Option<&Scope>) -> Value {
        Value::String("gurt".to_string())
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}
