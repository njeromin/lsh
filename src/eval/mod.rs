pub mod proc;
pub mod value;

use std::{collections::HashMap, process};

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

    pub fn evaluate(&mut self, prog: Program) {
        self.evaluate_statements(prog.body, None);
    }

    fn evaluate_statements(&mut self, statements: Vec<Statement>, scope_index: Option<usize>) {
        for statement in statements {
            match statement {
                Statement::Expr(expr) => {
                    self.evaluate_expr(&expr, scope_index);
                }
                Statement::Let { name, value } => {
                    // Handle let statement

                    if let Some(some_scope_index) = scope_index {
                        let expr = self.evaluate_expr(&value, Some(some_scope_index));

                        let scope = self
                            .scopes
                            .get_mut(some_scope_index)
                            .expect("Failed to get scope at index {some_scope_index}.");

                        scope.vars.insert(name, expr);
                    } else {
                        self.global_scope
                            .vars
                            .insert(name, self.evaluate_expr(&value, None));
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
                    let block_scope_index = self.make_scope();
                    self.evaluate_statements(block, Some(block_scope_index));
                }
            }
        }
    }

    fn make_scope(&mut self) -> usize {
        self.scopes.push(Scope::default());

        self.scopes.len()
    }

    fn evaluate_expr(&self, expr: &Expr, scope_index: Option<usize>) -> Value {
        match expr {
            Expr::Call { identifier, args } => {
                let string_args = args
                    .iter()
                    .map(|i| self.evaluate_expr(i, scope_index).to_string())
                    .collect();

                if identifier == "exit" {
                    process::exit(0); // TODO: Make this cleaner
                }

                proc::spawn_proc(identifier, string_args);

                Value::Null
            }
            _ => Value::String("Gurt".to_string()),
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}
