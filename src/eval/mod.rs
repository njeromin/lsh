pub mod cmds;
pub mod exec;
pub mod value;

use std::{
    collections::HashMap,
    env,
    io::{self, Stderr, Stdin, Stdout},
};

use crate::{
    eval::value::Value,
    parser::ast::{BinaryOp, Expr, Program, Statement},
};

#[derive(Default)]
pub struct Scope {
    vars: HashMap<String, Value>,
}

pub struct Evaluator {
    scopes: Vec<Scope>,
    aliases: HashMap<String, String>,
}

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator {
            scopes: vec![Scope::default()],
            aliases: HashMap::new(),
        }
    }

    pub fn push_var(&mut self, scope_index: usize, name: String, value: Value) {
        if let Some(scope) = self.scopes.get_mut(scope_index) {
            scope.vars.insert(name, value);
        } else {
            panic!("Failed to retrieve scope {scope_index}.");
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

    pub fn get_aliases(&self) -> &HashMap<String, String> {
        &self.aliases
    }

    pub fn evaluate(&mut self, prog: Program) {
        self.evaluate_statements(&prog.body, &mut vec![0]);
    }

    fn evaluate_statements(&mut self, statements: &[Statement], scope_indices: &mut Vec<usize>) {
        for statement in statements {
            match statement {
                Statement::Expr(expr) => {
                    self.evaluate_expr(expr, false, scope_indices);
                }
                Statement::Let { name, value } => {
                    let expr = self.evaluate_expr(value, true, scope_indices);

                    if let Some(last_scope_index) = scope_indices.last()
                        && let Some(scope) = self.scopes.get_mut(*last_scope_index)
                    {
                        scope.vars.insert(name.to_owned(), expr);
                    } else {
                        panic!("Failed to get current scope.");
                    }
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let cond_res = self.evaluate_expr(condition, true, scope_indices);

                    if cond_res.is_truthy() {
                        self.evaluate_statements(then_branch, scope_indices);
                    } else if let Some(else_branch_statements) = else_branch {
                        self.evaluate_statements(else_branch_statements, scope_indices);
                    }
                }
                Statement::While { condition, body } => {
                    while self
                        .evaluate_expr(condition, true, scope_indices)
                        .is_truthy()
                    {
                        self.evaluate_statements(body, scope_indices);
                    }
                }
                Statement::Block(block) => {
                    let block_scope_index = self.make_scope();
                    scope_indices.push(block_scope_index);

                    self.evaluate_statements(block, scope_indices);

                    scope_indices.pop();
                }
            }
        }
    }

    fn make_scope(&mut self) -> usize {
        self.scopes.push(Scope::default());

        self.scopes.len() - 1
    }

    fn evaluate_expr(
        &mut self,
        expr: &Expr,
        is_secondary: bool,
        scope_indices: &mut Vec<usize>,
    ) -> Value {
        match expr {
            Expr::CallOrString { identifier, args } => {
                let (res_cmd, res_args) = self.resolve_command(identifier.to_owned());

                // What if someone wants to set a value to the result of a command?
                if is_secondary {
                    return Value::String(identifier.to_owned());
                }

                let string_args = args
                    .iter()
                    .map(|i| self.evaluate_expr(i, true, scope_indices).to_string())
                    .collect();

                // Concatenate arguments from the input cmd with the provided arguments from the evaluator.
                let args = [res_args, string_args].concat();

                let mut io = lsh::IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();

                // TODO: Use a shared io::Stdout reference.
                cmds::exec_cmd(self, &res_cmd, args, &mut io);

                Value::Null
            }
            Expr::Variable(var_name) => {
                // Try to find variable in scopes.
                for scope_indices in scope_indices.iter().rev() {
                    if let Some(scope) = self.scopes.get(*scope_indices) {
                        if let Some(value) = scope.vars.get(var_name) {
                            return value.to_owned();
                        }
                    } else {
                        panic!("Failed to retrieve scope {scope_indices}.");
                    }
                }

                // Try to find variable in environment.
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
                let left_res = self.evaluate_expr(left_box, true, scope_indices);
                let right_res = self.evaluate_expr(right_box, true, scope_indices);

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
            Expr::TypeOf(expr) => {
                // Evaluate the expression and return its type as a string.
                let evaluated_value = self.evaluate_expr(expr, true, scope_indices);
                Value::String(evaluated_value.get_type().to_string())
            }
            Expr::String(string) => Value::String(string.to_owned()),
            Expr::Number(num) => Value::Number(*num),
            Expr::Bool(bool) => Value::Bool(*bool),
            Expr::CommandSub(expr) => {
                match expr.as_ref() {
                    Expr::CallOrString { identifier, args } => {
                        let (res_cmd, res_args) = self.resolve_command(identifier.to_owned());

                        let string_args: Vec<String> = args
                            .iter()
                            .map(|arg| self.evaluate_expr(arg, true, scope_indices).to_string())
                            .collect();

                        let all_args = [res_args, string_args].concat();

                        let mut io = lsh::IoCtx {
                            input: io::empty(),
                            out: Vec::<u8>::new(),
                            err: Vec::<u8>::new(),
                            input_mode: lsh::IoStreamMode::Pipe,
                            out_mode: lsh::IoStreamMode::Pipe,
                            err_mode: lsh::IoStreamMode::Pipe,
                        };

                        cmds::exec_cmd(self, &res_cmd, all_args, &mut io);

                        let output = String::from_utf8(io.out).unwrap_or_default();

                        Value::String(output)
                    }
                    other => {
                        // Fallback: allow non-command expressions inside $(...)
                        let value = self.evaluate_expr(other, true, scope_indices);
                        Value::String(value.to_string())
                    }
                }
            }
            Expr::Object(expr_map) => {
                let mut obj = HashMap::new();
                for (key, value) in expr_map {
                    let value_value = self.evaluate_expr(&value, true, scope_indices);
                    obj.insert(key.to_string(), value_value);
                }
                Value::Object(obj)
            }
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}
