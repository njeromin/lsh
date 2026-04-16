pub mod cmds;
pub mod exec;
pub mod value;

use std::{collections::HashMap, env, io};

use crate::{
    eval::value::{FunctionValue, Value},
    expand::{self, ExpandCtx},
    options::Options,
    parser::ast::{BinaryOp, CommandExpr, Expr, Program, Statement},
};

#[derive(Debug)]
pub enum EvalError {
    ScopeNotFound,
}

pub enum EvalFlow {
    Continue,
    Return(Value),
}

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

    pub fn with_globals(mut self) -> Evaluator {
        let scope = self.scopes.first_mut().unwrap(); // TODO: Don't use unwrap.

        scope.vars.insert(
            String::from("prompt"),
            Value::Object(HashMap::from([(
                String::from("text"),
                Value::String(String::from("\x1b[35m$\x1b[0m ")),
            )])),
        );

        scope
            .vars
            .insert(String::from("options"), Options::default().into());

        scope.vars.insert(
            String::from("palette"),
            Value::Object(HashMap::from([
                (
                    String::from("black"),
                    Value::String(String::from("\x1b[30m")),
                ),
                (String::from("red"), Value::String(String::from("\x1b[31m"))),
                (
                    String::from("green"),
                    Value::String(String::from("\x1b[32m")),
                ),
                (
                    String::from("yellow"),
                    Value::String(String::from("\x1b[33m")),
                ),
                (
                    String::from("blue"),
                    Value::String(String::from("\x1b[34m")),
                ),
                (
                    String::from("magenta"),
                    Value::String(String::from("\x1b[35m")),
                ),
                (
                    String::from("cyan"),
                    Value::String(String::from("\x1b[36m")),
                ),
                (
                    String::from("white"),
                    Value::String(String::from("\x1b[37m")),
                ),
                (
                    String::from("default"),
                    Value::String(String::from("\x1b[39m")),
                ),
                (
                    String::from("bright_red"),
                    Value::String(String::from("\x1b[91m")),
                ),
                (
                    String::from("bright_green"),
                    Value::String(String::from("\x1b[92m")),
                ),
                (
                    String::from("bright_yellow"),
                    Value::String(String::from("\x1b[93m")),
                ),
                (
                    String::from("bright_blue"),
                    Value::String(String::from("\x1b[94m")),
                ),
                (
                    String::from("bright_magenta"),
                    Value::String(String::from("\x1b[95m")),
                ),
                (
                    String::from("bright_cyan"),
                    Value::String(String::from("\x1b[96m")),
                ),
                (
                    String::from("bright_white"),
                    Value::String(String::from("\x1b[97m")),
                ),
                (
                    String::from("bg_black"),
                    Value::String(String::from("\x1b[40m")),
                ),
                (
                    String::from("bg_red"),
                    Value::String(String::from("\x1b[41m")),
                ),
                (
                    String::from("bg_green"),
                    Value::String(String::from("\x1b[42m")),
                ),
                (
                    String::from("bg_yellow"),
                    Value::String(String::from("\x1b[43m")),
                ),
                (
                    String::from("bg_blue"),
                    Value::String(String::from("\x1b[44m")),
                ),
                (
                    String::from("bg_magenta"),
                    Value::String(String::from("\x1b[45m")),
                ),
                (
                    String::from("bg_cyan"),
                    Value::String(String::from("\x1b[46m")),
                ),
                (
                    String::from("bg_white"),
                    Value::String(String::from("\x1b[47m")),
                ),
                (
                    String::from("bg_default"),
                    Value::String(String::from("\x1b[49m")),
                ),
                (String::from("bold"), Value::String(String::from("\x1b[1m"))),
                (String::from("dim"), Value::String(String::from("\x1b[2m"))),
                (
                    String::from("italic"),
                    Value::String(String::from("\x1b[3m")),
                ),
                (
                    String::from("underline"),
                    Value::String(String::from("\x1b[4m")),
                ),
                (
                    String::from("blink"),
                    Value::String(String::from("\x1b[5m")),
                ),
                (
                    String::from("inverse"),
                    Value::String(String::from("\x1b[7m")),
                ),
                (
                    String::from("hidden"),
                    Value::String(String::from("\x1b[8m")),
                ),
                (
                    String::from("strikethrough"),
                    Value::String(String::from("\x1b[9m")),
                ),
                (
                    String::from("reset_bold_dim"),
                    Value::String(String::from("\x1b[22m")),
                ),
                (
                    String::from("reset_italic"),
                    Value::String(String::from("\x1b[23m")),
                ),
                (
                    String::from("reset_underline"),
                    Value::String(String::from("\x1b[24m")),
                ),
                (
                    String::from("reset_blink"),
                    Value::String(String::from("\x1b[25m")),
                ),
                (
                    String::from("reset_inverse"),
                    Value::String(String::from("\x1b[27m")),
                ),
                (
                    String::from("reset_hidden"),
                    Value::String(String::from("\x1b[28m")),
                ),
                (
                    String::from("reset_strike"),
                    Value::String(String::from("\x1b[29m")),
                ),
                (
                    String::from("reset"),
                    Value::String(String::from("\x1b[0m")),
                ),
            ])),
        );

        let logical_pwd = env::var("PWD").unwrap_or_else(|_| {
            env::current_dir()
                .ok()
                .and_then(|cwd| cwd.to_str().map(ToOwned::to_owned))
                .unwrap_or_else(|| String::from("."))
        });
        scope
            .vars
            .insert(String::from("PWD"), Value::String(logical_pwd));

        self
    }

    pub fn exec_function(&mut self, func: &FunctionValue, args: Vec<Value>) -> Value {
        let mut scope_indices = vec![0];
        self.call_user_function_with_values(func, args, &mut scope_indices)
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

    pub fn set_var(&mut self, scope: usize, name: &str, value: Value) -> Result<(), EvalError> {
        let scope = self.scopes.get_mut(scope).ok_or(EvalError::ScopeNotFound)?;
        scope.vars.insert(name.to_string(), value);

        Ok(())
    }

    pub fn get_var(&self, scope: usize, name: &str) -> Option<&Value> {
        let scope = self.scopes.get(scope)?;
        scope.vars.get(name)
    }

    pub fn set_alias(&mut self, name: String, value: String) -> Option<String> {
        self.aliases.insert(name, value)
    }

    pub fn get_aliases(&self) -> &HashMap<String, String> {
        &self.aliases
    }

    pub fn evaluate(&mut self, prog: Program) {
        let _ = self.evaluate_statements(&prog.body, &mut vec![0]);
    }

    fn evaluate_in_new_scope(
        &mut self,
        statements: &[Statement],
        scope_indices: &mut Vec<usize>,
    ) -> EvalFlow {
        let scope_index = self.make_scope();
        scope_indices.push(scope_index);

        let flow = self.evaluate_statements(statements, scope_indices);

        let popped_scope_index = scope_indices
            .pop()
            .expect("Failed to pop scope index after scoped evaluation.");
        if popped_scope_index != scope_index {
            panic!(
                "Scope stack mismatch: expected to pop scope {scope_index}, popped {popped_scope_index}."
            );
        }

        self.scopes
            .pop()
            .expect("Failed to pop evaluator scope after scoped evaluation.");

        flow
    }

    fn evaluate_statements(
        &mut self,
        statements: &[Statement],
        scope_indices: &mut Vec<usize>,
    ) -> EvalFlow {
        for statement in statements {
            match statement {
                Statement::Expr(expr) => {
                    self.evaluate_expr(expr, false, scope_indices);
                }
                Statement::Command(cmd_expr) => {
                    let (_code, out, err) = self.evaluate_command_expr_with_input(
                        cmd_expr,
                        vec![],
                        scope_indices,
                        false,
                    );
                    if !out.is_empty() {
                        use std::io::Write as _;
                        let _ = io::stdout().write_all(&out);
                    }
                    if !err.is_empty() {
                        use std::io::Write as _;
                        let _ = io::stderr().write_all(&err);
                    }
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
                Statement::Assign { target, value } => {
                    let assigned_value = self.evaluate_expr(value, true, scope_indices);
                    self.assign_target(target, assigned_value, scope_indices);
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let cond_res = self.evaluate_expr(condition, true, scope_indices);

                    if cond_res.is_truthy() {
                        if let EvalFlow::Return(value) =
                            self.evaluate_in_new_scope(then_branch, scope_indices)
                        {
                            return EvalFlow::Return(value);
                        }
                    } else if let Some(else_branch_statements) = else_branch {
                        if let EvalFlow::Return(value) =
                            self.evaluate_in_new_scope(else_branch_statements, scope_indices)
                        {
                            return EvalFlow::Return(value);
                        }
                    }
                }
                Statement::While { condition, body } => {
                    while self
                        .evaluate_expr(condition, true, scope_indices)
                        .is_truthy()
                    {
                        if let EvalFlow::Return(value) =
                            self.evaluate_in_new_scope(body, scope_indices)
                        {
                            return EvalFlow::Return(value);
                        }
                    }
                }
                Statement::FunctionDeclaration {
                    target,
                    params,
                    body,
                } => {
                    let func_value = Value::Function(FunctionValue {
                        params: params.clone(),
                        body: body.clone(),
                    });
                    self.assign_target(target, func_value, scope_indices);
                }
                Statement::Return(maybe_expr) => {
                    let result = if let Some(expr) = maybe_expr {
                        self.evaluate_expr(expr, true, scope_indices)
                    } else {
                        Value::Null
                    };

                    return EvalFlow::Return(result);
                }
                Statement::Block(block) => {
                    if let EvalFlow::Return(value) =
                        self.evaluate_in_new_scope(block, scope_indices)
                    {
                        return EvalFlow::Return(value);
                    }
                }
            }
        }

        EvalFlow::Continue
    }

    fn make_scope(&mut self) -> usize {
        self.scopes.push(Scope::default());

        self.scopes.len() - 1
    }

    fn object_index_key(value: Value) -> Option<String> {
        match value {
            Value::String(s) => Some(s),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    fn evaluate_command_expr_with_input(
        &mut self,
        command_expr: &CommandExpr,
        input: Vec<u8>,
        scope_indices: &mut Vec<usize>,
        capture_output: bool,
    ) -> (i32, Vec<u8>, Vec<u8>) {
        match command_expr {
            CommandExpr::Call { identifier, args } => {
                let identifier_val = self.evaluate_expr(identifier, true, scope_indices);

                if let Value::Function(func) = &identifier_val {
                    let value = self.call_user_function(func, args, scope_indices);
                    return (0, value.to_string().into_bytes(), vec![]);
                }

                let identifier_str = identifier_val.to_string();

                if let Some(Value::Function(func)) =
                    self.resolve_named_value(&identifier_str, scope_indices)
                {
                    let value = self.call_user_function(&func, args, scope_indices);
                    return (0, value.to_string().into_bytes(), vec![]);
                }

                let (res_cmd, res_args) = self.resolve_command(identifier_str);
                let all_args = self.build_command_args(res_args, args, scope_indices);

                if capture_output {
                    let mut io_ctx = lsh::IoCtx {
                        input: io::Cursor::new(input),
                        out: Vec::<u8>::new(),
                        err: Vec::<u8>::new(),
                        input_mode: lsh::IoStreamMode::Pipe,
                        out_mode: lsh::IoStreamMode::Pipe,
                        err_mode: lsh::IoStreamMode::Pipe,
                    };

                    let code = cmds::exec_cmd(self, &res_cmd, all_args, &mut io_ctx);
                    (code, io_ctx.out, io_ctx.err)
                } else if input.is_empty() {
                    let mut io_ctx = lsh::IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();
                    let code = cmds::exec_cmd(self, &res_cmd, all_args, &mut io_ctx);
                    (code, vec![], vec![])
                } else {
                    let mut io_ctx = lsh::IoCtx {
                        input: io::Cursor::new(input),
                        out: io::stdout(),
                        err: io::stderr(),
                        input_mode: lsh::IoStreamMode::Pipe,
                        out_mode: lsh::IoStreamMode::Inherit,
                        err_mode: lsh::IoStreamMode::Inherit,
                    };

                    let code = cmds::exec_cmd(self, &res_cmd, all_args, &mut io_ctx);
                    (code, vec![], vec![])
                }
            }
            CommandExpr::Pipeline(stages) => {
                let mut piped_input = input;
                let mut final_code = 0;
                let mut combined_err = vec![];

                for (stage_index, stage) in stages.iter().enumerate() {
                    let is_last_stage = stage_index + 1 == stages.len();
                    let stage_capture_output = if is_last_stage { capture_output } else { true };

                    let (code, out, err) = self.evaluate_command_expr_with_input(
                        stage,
                        piped_input,
                        scope_indices,
                        stage_capture_output,
                    );
                    final_code = code;
                    combined_err.extend(err);
                    piped_input = out;
                }

                (final_code, piped_input, combined_err)
            }
            CommandExpr::And { left, right } => {
                let (left_code, mut left_out, mut left_err) = self
                    .evaluate_command_expr_with_input(left, input, scope_indices, capture_output);

                if left_code == 0 {
                    let (right_code, right_out, right_err) = self.evaluate_command_expr_with_input(
                        right,
                        vec![],
                        scope_indices,
                        capture_output,
                    );
                    left_out.extend(right_out);
                    left_err.extend(right_err);
                    (right_code, left_out, left_err)
                } else {
                    (left_code, left_out, left_err)
                }
            }
            CommandExpr::Or { left, right } => {
                let (left_code, mut left_out, mut left_err) = self
                    .evaluate_command_expr_with_input(left, input, scope_indices, capture_output);

                if left_code != 0 {
                    let (right_code, right_out, right_err) = self.evaluate_command_expr_with_input(
                        right,
                        vec![],
                        scope_indices,
                        capture_output,
                    );
                    left_out.extend(right_out);
                    left_err.extend(right_err);
                    (right_code, left_out, left_err)
                } else {
                    (left_code, left_out, left_err)
                }
            }
        }
    }

    fn scoped_var(&self, var_name: &str, scope_indices: &[usize]) -> Option<Value> {
        for scope_index in scope_indices.iter().rev() {
            if let Some(scope) = self.scopes.get(*scope_index) {
                if let Some(value) = scope.vars.get(var_name) {
                    return Some(value.to_owned());
                }
            } else {
                panic!("Failed to retrieve scope {scope_index}.");
            }
        }

        None
    }

    fn assign_var(&mut self, var_name: &str, value: Value, scope_indices: &[usize]) {
        for scope_index in scope_indices.iter().rev() {
            if let Some(scope) = self.scopes.get_mut(*scope_index) {
                if scope.vars.contains_key(var_name) {
                    scope.vars.insert(var_name.to_owned(), value);
                    return;
                }
            } else {
                panic!("Failed to retrieve scope {scope_index}.");
            }
        }

        if let Some(scope_index) = scope_indices.last().copied()
            && let Some(scope) = self.scopes.get_mut(scope_index)
        {
            scope.vars.insert(var_name.to_owned(), value);
        } else {
            panic!("Failed to get current scope for assignment.");
        }
    }

    fn assign_target(&mut self, target: &Expr, value: Value, scope_indices: &mut Vec<usize>) {
        match target {
            Expr::Variable(var_name) => {
                self.assign_var(var_name, value, scope_indices);
            }
            Expr::Index { object, index } => {
                let key_value = self.evaluate_expr(index, true, scope_indices);
                let key = Self::object_index_key(key_value)
                    .unwrap_or_else(|| panic!("Invalid object index for assignment."));

                let mut object_map = match self.evaluate_expr(object, true, scope_indices) {
                    Value::Object(map) => map,
                    Value::Null => HashMap::new(),
                    _ => panic!("Assignment target must be an object member."),
                };

                object_map.insert(key, value);
                self.assign_target(object, Value::Object(object_map), scope_indices);
            }
            _ => panic!("Invalid assignment target."),
        }
    }

    fn resolve_named_value(&self, identifier: &str, scope_indices: &[usize]) -> Option<Value> {
        let mut parts = identifier.split('.');
        let first_part = parts.next()?;
        let mut value = self.scoped_var(first_part, scope_indices)?;

        for part in parts {
            let next_value = match value {
                Value::Object(map) => map.get(part).cloned(),
                _ => None,
            }?;
            value = next_value;
        }

        Some(value)
    }

    fn build_command_args(
        &mut self,
        resolved_args: Vec<String>,
        args: &[Expr],
        scope_indices: &mut Vec<usize>,
    ) -> Vec<String> {
        let mut all_args = expand::expand_args(resolved_args, ExpandCtx::default());

        for arg in args {
            let evaluated = self.evaluate_expr(arg, true, scope_indices).to_string();

            match arg {
                // Match shell behavior more closely: tilde expansion should occur for
                // source literals, not for computed values (variables / command-sub output).
                Expr::String(_) => {
                    all_args.extend(expand::expand_arg(evaluated, ExpandCtx::default()))
                }
                _ => all_args.push(evaluated),
            }
        }

        all_args
    }

    fn call_user_function(
        &mut self,
        func: &FunctionValue,
        args: &[Expr],
        scope_indices: &mut Vec<usize>,
    ) -> Value {
        let evaluated_args: Vec<Value> = args
            .iter()
            .map(|arg| self.evaluate_expr(arg, true, scope_indices))
            .collect();

        self.call_user_function_with_values(func, evaluated_args, scope_indices)
    }

    fn call_user_function_with_values(
        &mut self,
        func: &FunctionValue,
        evaluated_args: Vec<Value>,
        scope_indices: &mut Vec<usize>,
    ) -> Value {
        let scope_index = self.make_scope();
        scope_indices.push(scope_index);

        if let Some(scope) = self.scopes.get_mut(scope_index) {
            for (param_index, param_name) in func.params.iter().enumerate() {
                let value = evaluated_args
                    .get(param_index)
                    .cloned()
                    .unwrap_or(Value::Null);
                scope.vars.insert(param_name.to_owned(), value);
            }
        } else {
            panic!("Failed to create function call scope.");
        }

        let flow = self.evaluate_statements(&func.body, scope_indices);

        let popped_scope_index = scope_indices
            .pop()
            .expect("Failed to pop scope index after function call.");
        if popped_scope_index != scope_index {
            panic!(
                "Scope stack mismatch: expected to pop scope {scope_index}, popped {popped_scope_index}."
            );
        }

        self.scopes
            .pop()
            .expect("Failed to pop evaluator scope after function call.");

        match flow {
            EvalFlow::Continue => Value::Null,
            EvalFlow::Return(value) => value,
        }
    }

    fn evaluate_expr(
        &mut self,
        expr: &Expr,
        is_secondary: bool,
        scope_indices: &mut Vec<usize>,
    ) -> Value {
        match expr {
            Expr::Call { target, args } => {
                let target_val = self.evaluate_expr(target, true, scope_indices);

                if let Value::Function(func) = &target_val {
                    return self.call_user_function(func, args, scope_indices);
                }

                // If target was a Word or Variable that resolved to a function name.
                if let Expr::Word(identifier) | Expr::Variable(identifier) = target.as_ref() {
                    if let Some(Value::Function(func)) =
                        self.resolve_named_value(identifier, scope_indices)
                    {
                        return self.call_user_function(&func, args, scope_indices);
                    }
                }

                // For command execution, we only want to do this if we are not in a secondary expression (like an argument).
                if is_secondary {
                    return target_val;
                }

                let identifier = target_val.to_string();
                let (res_cmd, res_args) = self.resolve_command(identifier);

                // Concatenate arguments from the input cmd with the provided arguments from the evaluator.
                let args = self.build_command_args(res_args, args, scope_indices);

                let mut io = lsh::IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();

                // TODO: Use a shared io::Stdout reference.
                cmds::exec_cmd(self, &res_cmd, args, &mut io);

                Value::Null
            }
            Expr::Word(word) => {
                if let Some(Value::Function(func)) = self.resolve_named_value(word, scope_indices) {
                    return self.call_user_function(&func, &[], scope_indices);
                }

                let (res_cmd, args) = self.resolve_command(word.to_owned());
                let args = expand::expand_args(args, ExpandCtx::default());

                if is_secondary {
                    return Value::String(word.to_owned());
                }

                let mut io = lsh::IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();
                cmds::exec_cmd(self, &res_cmd, args, &mut io);

                Value::Null
            }
            Expr::Variable(var_name) => {
                if let Some(value) = self.scoped_var(var_name, scope_indices) {
                    return value;
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
            Expr::Index { object, index } => {
                let object_value = self.evaluate_expr(object, true, scope_indices);
                let index_value = self.evaluate_expr(index, true, scope_indices);

                match (object_value, Self::object_index_key(index_value)) {
                    (Value::Object(map), Some(key)) => {
                        map.get(&key).cloned().unwrap_or(Value::Null)
                    }
                    _ => Value::Null,
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
                    Expr::Call { target, args } => {
                        let target_val = self.evaluate_expr(target, true, scope_indices);

                        if let Value::Function(func) = &target_val {
                            let value = self.call_user_function(func, args, scope_indices);
                            return Value::String(value.to_string());
                        }

                        let identifier = target_val.to_string();

                        if let Some(Value::Function(func)) =
                            self.resolve_named_value(&identifier, scope_indices)
                        {
                            let value = self.call_user_function(&func, args, scope_indices);
                            return Value::String(value.to_string());
                        }

                        let (res_cmd, res_args) = self.resolve_command(identifier);
                        let all_args = self.build_command_args(res_args, args, scope_indices);

                        let mut io = lsh::IoCtx {
                            input: io::empty(),
                            out: Vec::<u8>::new(),
                            err: Vec::<u8>::new(),
                            input_mode: lsh::IoStreamMode::Pipe,
                            out_mode: lsh::IoStreamMode::Pipe,
                            err_mode: lsh::IoStreamMode::Pipe,
                        };

                        let exit_status = cmds::exec_cmd(self, &res_cmd, all_args, &mut io);

                        let mut res = HashMap::new();

                        res.insert(String::from("code"), Value::Number(exit_status as f64));
                        res.insert(
                            String::from("out"),
                            Value::String(String::from_utf8(io.out).unwrap_or_default()),
                        );
                        res.insert(
                            String::from("err"),
                            Value::String(String::from_utf8(io.err).unwrap_or_default()),
                        );

                        Value::Object(res)
                    }
                    Expr::Word(word) => {
                        if let Some(Value::Function(func)) =
                            self.resolve_named_value(word, scope_indices)
                        {
                            let value = self.call_user_function(&func, &[], scope_indices);
                            return Value::String(value.to_string());
                        }

                        let (res_cmd, all_args) = self.resolve_command(word.to_owned());
                        let all_args = expand::expand_args(all_args, ExpandCtx::default());

                        let mut io = lsh::IoCtx {
                            input: io::empty(),
                            out: Vec::<u8>::new(),
                            err: Vec::<u8>::new(),
                            input_mode: lsh::IoStreamMode::Pipe,
                            out_mode: lsh::IoStreamMode::Pipe,
                            err_mode: lsh::IoStreamMode::Pipe,
                        };

                        let exit_status = cmds::exec_cmd(self, &res_cmd, all_args, &mut io);

                        let mut res = HashMap::new();

                        res.insert(String::from("code"), Value::Number(exit_status as f64));
                        res.insert(
                            String::from("out"),
                            Value::String(String::from_utf8(io.out).unwrap_or_default()),
                        );
                        res.insert(
                            String::from("err"),
                            Value::String(String::from_utf8(io.err).unwrap_or_default()),
                        );

                        Value::Object(res)
                    }
                    other => {
                        // Fallback: allow non-command expressions inside $(...)
                        let value = self.evaluate_expr(other, true, scope_indices);
                        Value::String(value.to_string())
                    }
                }
            }
            Expr::CommandSubCommand(command_expr) => {
                let (code, out, err) = self.evaluate_command_expr_with_input(
                    command_expr,
                    vec![],
                    scope_indices,
                    true,
                );
                let mut res = HashMap::new();
                res.insert(String::from("code"), Value::Number(code as f64));
                res.insert(
                    String::from("out"),
                    Value::String(String::from_utf8(out).unwrap_or_default()),
                );
                res.insert(
                    String::from("err"),
                    Value::String(String::from_utf8(err).unwrap_or_default()),
                );
                Value::Object(res)
            }
            Expr::AnonymousFunction { params, body } => Value::Function(FunctionValue {
                params: params.clone(),
                body: body.clone(),
            }),
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
