pub mod cmds;
pub mod value;

use std::{collections::HashMap, env};

use crate::{
    eval::value::Value,
    parser::ast::{BinaryOp, Expr, Program, Statement},
};

#[derive(Default)]
pub struct Scope {
    vars: HashMap<String, Value>,
}

pub struct Evaluator {
    global_scope: Scope,
    scopes: Vec<Scope>,
    aliases: HashMap<String, String>,
}

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator {
            global_scope: Scope::default(),
            scopes: vec![],
            aliases: HashMap::new(),
        }
    }

    // TODO: Rewrite better
    pub fn resolve_alias(&self, cmd: String) -> String {
        if let Some(resolved_alias) = self.aliases.get(cmd.as_str()) {
            resolved_alias.to_owned()
        } else {
            cmd
        }
    }

    pub fn set_alias(&mut self, name: String, value: String) -> Option<String> {
        self.aliases.insert(name, value)
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
                        let expr_res = self.evaluate_expr(&value, None);
                        self.global_scope.vars.insert(name, expr_res);
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

    fn evaluate_expr(&mut self, expr: &Expr, scope_index: Option<usize>) -> Value {
        match expr {
            Expr::Call { identifier, args } => {
                let string_args = args
                    .iter()
                    .map(|i| self.evaluate_expr(i, scope_index).to_string())
                    .collect();

                cmds::exec_cmd(self, identifier, string_args);

                Value::Null
            }
            Expr::Variable(var_name) => {
                // TODO: Clean up.
                if let Some(some_scope_index) = scope_index
                    && let Some(scope) = self.scopes.get(some_scope_index)
                    && let Some(var) = scope.vars.get(var_name)
                {
                    return var.to_owned();
                }

                if let Some(global_var) = self.global_scope.vars.get(var_name) {
                    return global_var.to_owned();
                }

                panic!("Unknown variable {var_name}.");
            }
            Expr::EnvVar(var_name) => {
                if let Ok(env_var) = env::var(var_name) {
                    // Environment variables are resolved into strings.
                    Value::String(env_var)
                } else {
                    Value::Null
                }
            }
            Expr::Binary {
                op,
                left: left_box,
                right: right_box,
            } => {
                // TODO: Implement.

                let left_res = self.evaluate_expr(left_box, scope_index);
                let right_res = self.evaluate_expr(right_box, scope_index);

                // match op {
                //     BinaryOp::Add => {}
                // }

                Value::Null
            }
            Expr::String(string) => Value::String(string.to_owned()),
            Expr::Number(num) => Value::Number(*num),
            Expr::Bool(bool) => Value::Bool(*bool),
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}
