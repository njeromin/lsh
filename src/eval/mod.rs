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

    pub fn resolve_command(&self, cmd: String) -> (String, Vec<String>) {
        if let Some(resolved_alias) = self.aliases.get(cmd.as_str()) {
            // Split the unsplit command by spaces, take out the command itself, and return the rest of the
            // parts as arguments.
            let mut split_cmd = resolved_alias.split(" ");
            let resolved_cmd = split_cmd
                .next()
                .expect("Failed to get first command.")
                .to_string();
            let args: Vec<String> = split_cmd.map(|arg| arg.to_string()).collect();

            (resolved_cmd, args)
        } else {
            (cmd, vec![])
        }
    }

    pub fn set_alias(&mut self, name: String, value: String) -> Option<String> {
        self.aliases.insert(name, value)
    }

    pub fn evaluate(&mut self, prog: Program) {
        self.evaluate_statements(&prog.body, None);
    }

    fn evaluate_statements(&mut self, statements: &[Statement], scope_index: Option<usize>) {
        for statement in statements {
            match statement {
                Statement::Expr(expr) => {
                    self.evaluate_expr(expr, scope_index);
                }
                Statement::Let { name, value } => {
                    // Handle let statement

                    if let Some(some_scope_index) = scope_index {
                        let expr = self.evaluate_expr(value, Some(some_scope_index));

                        let scope = self
                            .scopes
                            .get_mut(some_scope_index)
                            .expect("Failed to get scope at index {some_scope_index}.");

                        scope.vars.insert(name.to_owned(), expr);
                    } else {
                        let expr_res = self.evaluate_expr(value, None);
                        self.global_scope.vars.insert(name.to_owned(), expr_res);
                    }
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let cond_res = self.evaluate_expr(condition, scope_index);

                    if cond_res.is_truthy() {
                        self.evaluate_statements(then_branch, scope_index);
                    } else if let Some(else_branch_statements) = else_branch {
                        self.evaluate_statements(else_branch_statements, scope_index);
                    }
                }
                Statement::While { condition, body } => {
                    while self.evaluate_expr(condition, scope_index).is_truthy() {
                        self.evaluate_statements(body, scope_index);
                    }
                }
                Statement::For {
                    condition: _,
                    body: _,
                } => {
                    todo!("Implement"); // Why does this even take an expr as an input like that
                    // how is that even going to work.
                }
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
            Expr::CallOrString { identifier, args } => {
                let (res_cmd, res_args) = self.resolve_command(identifier.to_owned());

                if res_args.is_empty() && !cmds::is_valid_cmd(&res_cmd) {
                    return Value::String(identifier.to_owned());
                }

                let string_args = args
                    .iter()
                    .map(|i| self.evaluate_expr(i, scope_index).to_string())
                    .collect();

                // Concatenate arguments from the input cmd with the provided arguments from the evaluator.
                let args = [res_args, string_args].concat();

                cmds::exec_cmd(self, &res_cmd, args);

                Value::Null
            }
            Expr::Variable(var_name) => {
                // Try to find local variable.
                if let Some(some_scope_index) = scope_index
                    && let Some(scope) = self.scopes.get(some_scope_index)
                    && let Some(some_value) = scope.vars.get(var_name)
                {
                    return some_value.to_owned();
                }

                // Try to find global variable.
                if let Some(some_value) = self.global_scope.vars.get(var_name) {
                    return some_value.to_owned();
                }

                // Try to find environment variable.
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

                let left_type = left_res.get_type();

                match *op {
                    BinaryOp::Add => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => Value::Number(l + r),
                        (Value::String(l), Value::String(r)) => Value::String(l + &r),
                        _ => Value::Null, // or error if you prefer
                    },

                    BinaryOp::Sub => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => Value::Number(l - r),
                        _ => Value::Null,
                    },

                    BinaryOp::Mul => match (left_res, right_res) {
                        (Value::Number(l), Value::Number(r)) => Value::Number(l * r),
                        (Value::String(l), Value::Number(r)) => {
                            Value::String(l.repeat(r.round() as usize))
                        }
                        _ => Value::Null,
                    },

                    BinaryOp::Div => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => Value::Number(l / r),
                        _ => Value::Null,
                    },

                    BinaryOp::Eq => Value::Bool(left_res.check_equality(&right_res)),

                    BinaryOp::Neq => Value::Bool(!left_res.check_equality(&right_res)),

                    BinaryOp::Gt => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => Value::Bool(l > r),
                        (Value::String(l), Value::String(r)) => Value::Bool(l > r),
                        _ => Value::Null,
                    },

                    BinaryOp::Lt => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => Value::Bool(l < r),
                        (Value::String(l), Value::String(r)) => Value::Bool(l < r),
                        _ => Value::Null,
                    },

                    BinaryOp::Ge => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => Value::Bool(l >= r),
                        (Value::String(l), Value::String(r)) => Value::Bool(l >= r),
                        _ => Value::Null,
                    },

                    BinaryOp::Le => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => Value::Bool(l <= r),
                        (Value::String(l), Value::String(r)) => Value::Bool(l <= r),
                        _ => Value::Null,
                    },

                    BinaryOp::And => Value::Bool(left_res.is_truthy() && right_res.is_truthy()),

                    BinaryOp::Or => Value::Bool(left_res.is_truthy() || right_res.is_truthy()),
                }
            }
            Expr::Typeof(expr) => {
                // Evaluate the expression and return its type as a string.
                let evaluated_value = self.evaluate_expr(expr, scope_index);
                Value::String(evaluated_value.get_type().to_string())
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
