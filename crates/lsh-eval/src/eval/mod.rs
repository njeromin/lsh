pub mod cmds;
pub mod exec;
pub mod value;

use std::{
    collections::HashMap,
    env,
    fs::File,
    io,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use lsh_completions::CompletionRegistry;
use lsh_syntax::{
    ast::{BinaryOp, CommandExpr, Expr, Program, Statement},
    print_error,
};

use crate::{
    eval::value::{FunctionValue, Value},
    expand::{self, ExpandCtx},
    io::{IoCtx, IoStreamMode},
    signals::{
        STATUS_SIGINT, TerminalForegroundGuard, get_interrupt_flag, is_interactive_tty,
        signal_exit_code,
    },
};

#[derive(Debug)]
pub enum EvalError {
    ScopeNotFound,
}

pub enum EvalFlow {
    Continue,
    Return(Value),
}

struct EvalOutcome {
    flow: EvalFlow,
    last_status: i32,
}

struct ExternalPipelineStage {
    cmd: String,
    args: Vec<String>,
    env_assignments: Vec<(String, String)>,
    stdout_redirect: Option<String>,
}

#[derive(Default)]
pub struct Scope {
    vars: HashMap<String, Value>,
}

pub struct Evaluator {
    scopes: Vec<Scope>,
    aliases: HashMap<String, String>,
    pub last_status: i32,
    pub interrupted: Arc<AtomicBool>,
    pub completion_registry: Arc<RwLock<CompletionRegistry>>,
}

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator {
            scopes: vec![Scope::default()],
            aliases: HashMap::new(),
            last_status: 0,
            interrupted: get_interrupt_flag(),
            completion_registry: Arc::new(RwLock::new(CompletionRegistry::with_defaults())),
        }
    }

    pub fn with_interrupt_flag(mut self, flag: Arc<AtomicBool>) -> Evaluator {
        self.interrupted = flag;
        self
    }

    pub fn with_completion_registry(
        mut self,
        registry: Arc<RwLock<CompletionRegistry>>,
    ) -> Evaluator {
        self.completion_registry = registry;
        self
    }

    fn default_options() -> Value {
        let nerd_icons = env::var("LSH_NERD_ICONS")
            .or_else(|_| env::var("NERD_ICONS"))
            .map(|v| {
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("yes")
                    || v.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false);

        let mut map = HashMap::new();
        map.insert("tab_stop".to_string(), Value::Number(4.0));
        map.insert(
            "edit_mode".to_string(),
            Value::String("default".to_string()),
        );
        map.insert(
            "completion_type".to_string(),
            Value::String("menu".to_string()),
        );
        map.insert("nerd_icons".to_string(), Value::Bool(nerd_icons));

        Value::Object(map)
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
            .insert(String::from("options"), Self::default_options());

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

    pub fn get_var_names(&self) -> Vec<String> {
        let mut vars = Vec::new();
        for scope in &self.scopes {
            vars.extend(scope.vars.keys().cloned());
        }
        vars
    }

    pub fn evaluate(&mut self, prog: Program) -> i32 {
        self.evaluate_statements(&prog.body, &mut vec![0])
            .last_status
    }

    fn evaluate_in_new_scope(
        &mut self,
        statements: &[Statement],
        scope_indices: &mut Vec<usize>,
    ) -> EvalOutcome {
        let scope_index = self.make_scope();
        scope_indices.push(scope_index);

        let outcome = self.evaluate_statements(statements, scope_indices);

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

        outcome
    }

    fn evaluate_statements(
        &mut self,
        statements: &[Statement],
        scope_indices: &mut Vec<usize>,
    ) -> EvalOutcome {
        let mut last_status = 0;

        for statement in statements {
            if self.interrupted.swap(false, Ordering::Relaxed) {
                self.last_status = STATUS_SIGINT;
                return EvalOutcome {
                    flow: EvalFlow::Return(Value::Null),
                    last_status: STATUS_SIGINT,
                };
            }

            match statement {
                Statement::Expr(expr) => {
                    self.evaluate_expr(expr, false, scope_indices);
                    last_status = self.last_status;
                }
                Statement::Command(cmd_expr) => {
                    let (code, out, err) = self.evaluate_command_expr_with_input(
                        cmd_expr,
                        vec![],
                        scope_indices,
                        false,
                    );
                    last_status = code;
                    self.last_status = code;
                    if !out.is_empty() {
                        use std::io::Write as _;
                        let _ = io::stdout().write_all(&out);
                    }
                    if !err.is_empty() {
                        use std::io::Write as _;
                        let _ = io::stderr().write_all(&err);
                    }
                    if code == STATUS_SIGINT || self.interrupted.swap(false, Ordering::Relaxed) {
                        return EvalOutcome {
                            flow: EvalFlow::Return(Value::Null),
                            last_status: STATUS_SIGINT,
                        };
                    }
                }
                Statement::Let { name, value } => {
                    let expr = self.evaluate_expr(value, true, scope_indices);
                    last_status = 0;

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
                    last_status = 0;
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let cond_res = self.evaluate_expr(condition, true, scope_indices);

                    if cond_res.is_truthy() {
                        let outcome = self.evaluate_in_new_scope(then_branch, scope_indices);
                        last_status = outcome.last_status;
                        if let EvalFlow::Return(value) = outcome.flow {
                            return EvalOutcome {
                                flow: EvalFlow::Return(value),
                                last_status,
                            };
                        }
                    } else if let Some(else_branch_statements) = else_branch {
                        let outcome =
                            self.evaluate_in_new_scope(else_branch_statements, scope_indices);
                        last_status = outcome.last_status;
                        if let EvalFlow::Return(value) = outcome.flow {
                            return EvalOutcome {
                                flow: EvalFlow::Return(value),
                                last_status,
                            };
                        }
                    }
                }
                Statement::While { condition, body } => {
                    while self
                        .evaluate_expr(condition, true, scope_indices)
                        .is_truthy()
                    {
                        if self.interrupted.swap(false, Ordering::Relaxed) {
                            self.last_status = STATUS_SIGINT;
                            last_status = STATUS_SIGINT;
                            break;
                        }

                        let outcome = self.evaluate_in_new_scope(body, scope_indices);
                        last_status = outcome.last_status;
                        if outcome.last_status == STATUS_SIGINT
                            || self.interrupted.swap(false, Ordering::Relaxed)
                        {
                            self.last_status = STATUS_SIGINT;
                            last_status = STATUS_SIGINT;
                            break;
                        }
                        if let EvalFlow::Return(value) = outcome.flow {
                            return EvalOutcome {
                                flow: EvalFlow::Return(value),
                                last_status,
                            };
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
                    last_status = 0;
                }
                Statement::Return(maybe_expr) => {
                    let result = if let Some(expr) = maybe_expr {
                        self.evaluate_expr(expr, true, scope_indices)
                    } else {
                        Value::Null
                    };

                    return EvalOutcome {
                        flow: EvalFlow::Return(result),
                        last_status,
                    };
                }
                Statement::Block(block) => {
                    let outcome = self.evaluate_in_new_scope(block, scope_indices);
                    last_status = outcome.last_status;
                    if let EvalFlow::Return(value) = outcome.flow {
                        return EvalOutcome {
                            flow: EvalFlow::Return(value),
                            last_status,
                        };
                    }
                }
            }
        }

        EvalOutcome {
            flow: EvalFlow::Continue,
            last_status,
        }
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

    fn make_command_result(code: i32, out: String, err: String) -> Value {
        let mut res = HashMap::new();
        res.insert(String::from("code"), Value::Number(code as f64));
        res.insert(String::from("out"), Value::String(out));
        res.insert(String::from("err"), Value::String(err));
        Value::Object(res)
    }

    fn resolve_stdout_redirect_path(
        &mut self,
        stdout_redirect: &Option<Expr>,
        scope_indices: &mut Vec<usize>,
    ) -> Option<String> {
        stdout_redirect.as_ref().map(|target_expr| {
            let raw_target = self
                .evaluate_expr(target_expr, true, scope_indices)
                .to_string();
            let expanded = expand::expand_arg(
                raw_target.clone(),
                ExpandCtx {
                    tilde: true,
                    glob: false,
                },
            );
            expanded.into_iter().next().unwrap_or(raw_target)
        })
    }

    fn maybe_redirect_output(
        output: Vec<u8>,
        stdout_redirect_path: Option<&str>,
    ) -> (i32, Vec<u8>, Vec<u8>) {
        if let Some(path) = stdout_redirect_path {
            match File::create(path) {
                Ok(mut file) => {
                    use std::io::Write as _;
                    if let Err(e) = file.write_all(&output) {
                        (-1, vec![], e.to_string().into_bytes())
                    } else {
                        (0, vec![], vec![])
                    }
                }
                Err(e) => (-1, vec![], e.to_string().into_bytes()),
            }
        } else {
            (0, output, vec![])
        }
    }

    fn try_execute_streaming_pipeline(
        &mut self,
        stages: &[CommandExpr],
        input: Vec<u8>,
        scope_indices: &mut Vec<usize>,
        capture_output: bool,
    ) -> Option<(i32, Vec<u8>, Vec<u8>)> {
        if stages.len() < 2 {
            return None;
        }

        let mut resolved_stages = Vec::with_capacity(stages.len());

        for (stage_idx, stage) in stages.iter().enumerate() {
            let CommandExpr::Call {
                env_assignments,
                identifier,
                args,
                stdout_redirect,
            } = stage
            else {
                return None;
            };

            // Intermediate stages must not have their own file redirection.
            if stage_idx + 1 < stages.len() && stdout_redirect.is_some() {
                return None;
            }

            let temp_scope_index = self.make_scope();
            scope_indices.push(temp_scope_index);

            let mut env_assignment_values = vec![];
            for (name, value_expr) in env_assignments {
                let value = self
                    .evaluate_expr(value_expr, true, scope_indices)
                    .to_string();
                self.push_var(temp_scope_index, name.clone(), Value::String(value.clone()));
                env_assignment_values.push((name.clone(), value));
            }

            let stdout_redirect_path =
                self.resolve_stdout_redirect_path(stdout_redirect, scope_indices);

            let identifier_val = self.evaluate_expr(identifier, true, scope_indices);

            let popped = scope_indices.pop();
            if popped != Some(temp_scope_index) {
                panic!("Scope stack mismatch");
            }
            self.scopes.pop();

            if matches!(identifier_val, Value::Function(_)) {
                return None;
            }

            let identifier_str = identifier_val.to_string();
            if self
                .resolve_named_value(&identifier_str, scope_indices)
                .is_some_and(|v| matches!(v, Value::Function(_)))
            {
                return None;
            }

            if crate::builtins::lookup(&identifier_str).is_some() {
                return None;
            }

            let (res_cmd, res_args) = self.resolve_command(identifier_str);
            let all_args = self.build_command_args(res_args, args, scope_indices);

            resolved_stages.push(ExternalPipelineStage {
                cmd: res_cmd,
                args: all_args,
                env_assignments: env_assignment_values,
                stdout_redirect: stdout_redirect_path,
            });
        }

        Some(self.execute_external_pipeline(resolved_stages, input, capture_output))
    }

    fn execute_external_pipeline(
        &mut self,
        stages: Vec<ExternalPipelineStage>,
        input: Vec<u8>,
        capture_output: bool,
    ) -> (i32, Vec<u8>, Vec<u8>) {
        use std::process::{Command, Stdio};

        let total_stages = stages.len();
        if total_stages == 0 {
            return (0, vec![], vec![]);
        }

        let mut children: Vec<std::process::Child> = Vec::with_capacity(total_stages);
        let mut prev_stdout: Option<std::process::ChildStdout> = None;
        let mut leader_pid: i32 = 0;

        for (i, stage) in stages.into_iter().enumerate() {
            let is_first = i == 0;
            let is_last = i + 1 == total_stages;

            let mut c = Command::new(&stage.cmd);
            c.args(stage.args);
            c.envs(
                stage
                    .env_assignments
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str())),
            );

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                if is_interactive_tty() {
                    if is_first {
                        c.process_group(0);
                    } else {
                        c.process_group(leader_pid);
                    }
                }
            }

            // Stdin
            if is_first {
                if !input.is_empty() {
                    c.stdin(Stdio::piped());
                } else {
                    c.stdin(Stdio::inherit());
                }
            } else {
                let stdin_source = prev_stdout
                    .take()
                    .expect("Previous stage stdout must be available");
                c.stdin(Stdio::from(stdin_source));
            }

            // Stdout
            if !is_last {
                c.stdout(Stdio::piped());
            } else if let Some(redirect_path) = stage.stdout_redirect {
                match File::create(&redirect_path) {
                    Ok(file) => {
                        c.stdout(Stdio::from(file));
                    }
                    Err(e) => {
                        for mut child in children {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                        return (-1, vec![], e.to_string().into_bytes());
                    }
                }
            } else if capture_output {
                c.stdout(Stdio::piped());
            } else {
                c.stdout(Stdio::inherit());
            }

            // Stderr
            if capture_output {
                c.stderr(Stdio::piped());
            } else {
                c.stderr(Stdio::inherit());
            }

            match c.spawn() {
                Ok(mut child) => {
                    if is_first {
                        leader_pid = child.id() as i32;
                    }
                    if !is_last {
                        prev_stdout = child.stdout.take();
                    }
                    children.push(child);
                }
                Err(e) => {
                    for mut child in children {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    let err_msg = if e.kind() == io::ErrorKind::NotFound {
                        format!("Command '{}' not found.", stage.cmd)
                    } else {
                        e.to_string()
                    };
                    if !capture_output {
                        let mut cerr = io::stderr();
                        print_error(&mut cerr, &err_msg);
                        return (-1, vec![], vec![]);
                    } else {
                        return (-1, vec![], err_msg.into_bytes());
                    }
                }
            }
        }

        let _guard = if !capture_output && is_interactive_tty() {
            TerminalForegroundGuard::give_to_pgid(leader_pid)
        } else {
            TerminalForegroundGuard::inactive()
        };

        let mut captured_out = Vec::new();
        let mut captured_err = Vec::new();

        std::thread::scope(|s| {
            if !input.is_empty()
                && !children.is_empty()
                && let Some(mut stdin) = children[0].stdin.take()
            {
                s.spawn(move || {
                    let _ = io::copy(&mut io::Cursor::new(input), &mut stdin);
                });
            }

            let stdout_handle = if capture_output && !children.is_empty() {
                children
                    .last_mut()
                    .unwrap()
                    .stdout
                    .take()
                    .map(|mut stdout| {
                        s.spawn(move || {
                            let mut buf = Vec::new();
                            let _ = io::copy(&mut stdout, &mut buf);
                            buf
                        })
                    })
            } else {
                None
            };

            let mut stderr_handles = Vec::new();
            if capture_output {
                for child in &mut children {
                    if let Some(mut stderr) = child.stderr.take() {
                        stderr_handles.push(s.spawn(move || {
                            let mut buf = Vec::new();
                            let _ = io::copy(&mut stderr, &mut buf);
                            buf
                        }));
                    }
                }
            }

            if let Some(handle) = stdout_handle
                && let Ok(out) = handle.join()
            {
                captured_out = out;
            }

            for handle in stderr_handles {
                if let Ok(err) = handle.join() {
                    captured_err.extend(err);
                }
            }
        });

        let mut last_code = 0;
        for (i, mut child) in children.into_iter().enumerate() {
            let code = match child.wait() {
                Ok(status) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        if let Some(c) = status.code() {
                            c
                        } else if let Some(sig) = status.signal() {
                            if sig == libc::SIGINT && !capture_output {
                                use std::io::Write as _;
                                let _ = writeln!(io::stderr());
                            }
                            signal_exit_code(sig)
                        } else {
                            1
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        status.code().unwrap_or(1)
                    }
                }
                Err(_) => -1,
            };

            if i + 1 == total_stages {
                last_code = code;
            }
        }

        (last_code, captured_out, captured_err)
    }

    fn execute_external_command(
        &mut self,
        cmd: &str,
        args: Vec<String>,
        env_assignment_values: &[(String, String)],
        input: Vec<u8>,
        capture_output: bool,
        stdout_redirect_path: Option<&str>,
    ) -> (i32, Vec<u8>, Vec<u8>) {
        if let Some(path) = stdout_redirect_path {
            let file = match File::create(path) {
                Ok(file) => file,
                Err(e) => return (-1, vec![], e.to_string().into_bytes()),
            };

            if capture_output {
                let mut io_ctx = IoCtx {
                    input: io::Cursor::new(input),
                    out: file,
                    err: Vec::<u8>::new(),
                    input_mode: IoStreamMode::Pipe,
                    out_mode: IoStreamMode::Pipe,
                    err_mode: IoStreamMode::Pipe,
                };

                let code = cmds::exec_cmd(self, cmd, args, env_assignment_values, &mut io_ctx);
                return (code, vec![], io_ctx.err);
            }

            if input.is_empty() {
                let mut io_ctx = IoCtx {
                    input: io::stdin(),
                    out: file,
                    err: io::stderr(),
                    input_mode: IoStreamMode::Inherit,
                    out_mode: IoStreamMode::Pipe,
                    err_mode: IoStreamMode::Inherit,
                };

                let code = cmds::exec_cmd(self, cmd, args, env_assignment_values, &mut io_ctx);
                return (code, vec![], vec![]);
            }

            let mut io_ctx = IoCtx {
                input: io::Cursor::new(input),
                out: file,
                err: io::stderr(),
                input_mode: IoStreamMode::Pipe,
                out_mode: IoStreamMode::Pipe,
                err_mode: IoStreamMode::Inherit,
            };

            let code = cmds::exec_cmd(self, cmd, args, env_assignment_values, &mut io_ctx);
            return (code, vec![], vec![]);
        }

        if capture_output {
            let mut io_ctx = IoCtx {
                input: io::Cursor::new(input),
                out: Vec::<u8>::new(),
                err: Vec::<u8>::new(),
                input_mode: IoStreamMode::Pipe,
                out_mode: IoStreamMode::Pipe,
                err_mode: IoStreamMode::Pipe,
            };

            let code = cmds::exec_cmd(self, cmd, args, env_assignment_values, &mut io_ctx);
            return (code, io_ctx.out, io_ctx.err);
        }

        if input.is_empty() {
            let mut io_ctx = IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();
            let code = cmds::exec_cmd(self, cmd, args, env_assignment_values, &mut io_ctx);
            return (code, vec![], vec![]);
        }

        let mut io_ctx = IoCtx {
            input: io::Cursor::new(input),
            out: io::stdout(),
            err: io::stderr(),
            input_mode: IoStreamMode::Pipe,
            out_mode: IoStreamMode::Inherit,
            err_mode: IoStreamMode::Inherit,
        };

        let code = cmds::exec_cmd(self, cmd, args, env_assignment_values, &mut io_ctx);
        (code, vec![], vec![])
    }

    fn evaluate_command_expr_with_input(
        &mut self,
        command_expr: &CommandExpr,
        input: Vec<u8>,
        scope_indices: &mut Vec<usize>,
        capture_output: bool,
    ) -> (i32, Vec<u8>, Vec<u8>) {
        match command_expr {
            CommandExpr::Call {
                env_assignments,
                identifier,
                args,
                stdout_redirect,
            } => {
                // Evaluate env-prefix assignments in a temporary scope so they are visible
                // during same-command expansion but don't leak into shell state.
                let temp_scope_index = self.make_scope();
                scope_indices.push(temp_scope_index);

                let mut env_assignment_values = vec![];
                for (name, value_expr) in env_assignments {
                    let value = self
                        .evaluate_expr(value_expr, true, scope_indices)
                        .to_string();
                    self.push_var(temp_scope_index, name.clone(), Value::String(value.clone()));
                    env_assignment_values.push((name.clone(), value));
                }

                let stdout_redirect_path =
                    self.resolve_stdout_redirect_path(stdout_redirect, scope_indices);

                let result = {
                    let identifier_val = self.evaluate_expr(identifier, true, scope_indices);

                    if let Value::Function(func) = &identifier_val {
                        let output = self
                            .call_user_function(func, args, scope_indices)
                            .to_string()
                            .into_bytes();
                        Self::maybe_redirect_output(output, stdout_redirect_path.as_deref())
                    } else {
                        let identifier_str = identifier_val.to_string();

                        if let Some(Value::Function(func)) =
                            self.resolve_named_value(&identifier_str, scope_indices)
                        {
                            let output = self
                                .call_user_function(&func, args, scope_indices)
                                .to_string()
                                .into_bytes();
                            Self::maybe_redirect_output(output, stdout_redirect_path.as_deref())
                        } else {
                            let (res_cmd, res_args) = self.resolve_command(identifier_str);
                            let all_args = self.build_command_args(res_args, args, scope_indices);
                            self.execute_external_command(
                                &res_cmd,
                                all_args,
                                env_assignment_values.as_slice(),
                                input,
                                capture_output,
                                stdout_redirect_path.as_deref(),
                            )
                        }
                    }
                };

                let popped_scope_index = scope_indices
                    .pop()
                    .expect("Failed to pop temp env scope index after command call.");
                if popped_scope_index != temp_scope_index {
                    panic!(
                        "Scope stack mismatch: expected to pop scope {temp_scope_index}, popped {popped_scope_index}."
                    );
                }
                self.scopes
                    .pop()
                    .expect("Failed to pop temp env scope after command call.");

                result
            }
            CommandExpr::Pipeline(stages) => {
                if let Some(res) = self.try_execute_streaming_pipeline(
                    stages,
                    input.clone(),
                    scope_indices,
                    capture_output,
                ) {
                    return res;
                }

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

    fn expr_has_unquoted_glob_or_tilde(expr: &Expr) -> (bool, bool) {
        match expr {
            Expr::QuotedString(_) => (false, false),
            Expr::String(s) => {
                let has_tilde = s.starts_with('~');
                let has_glob = expand::has_glob_metachars(s);
                (has_tilde, has_glob)
            }
            Expr::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                let (lt, lg) = Self::expr_has_unquoted_glob_or_tilde(left);
                let (_rt, rg) = Self::expr_has_unquoted_glob_or_tilde(right);
                (lt, lg || rg)
            }
            _ => (false, false),
        }
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
            let (has_tilde, has_glob) = Self::expr_has_unquoted_glob_or_tilde(arg);

            if has_tilde || has_glob {
                all_args.extend(expand::expand_arg(
                    evaluated,
                    ExpandCtx {
                        tilde: has_tilde,
                        glob: has_glob,
                    },
                ));
            } else {
                all_args.push(evaluated);
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

        let outcome = self.evaluate_statements(&func.body, scope_indices);

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

        match outcome.flow {
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
                if let Expr::Word(identifier) | Expr::Variable(identifier) = target.as_ref()
                    && let Some(Value::Function(func)) =
                        self.resolve_named_value(identifier, scope_indices)
                {
                    return self.call_user_function(&func, args, scope_indices);
                }

                // For command execution, we only want to do this if we are not in a secondary expression (like an argument).
                if is_secondary {
                    return target_val;
                }

                let identifier = target_val.to_string();
                let (res_cmd, res_args) = self.resolve_command(identifier);

                // Concatenate arguments from the input cmd with the provided arguments from the evaluator.
                let args = self.build_command_args(res_args, args, scope_indices);

                let mut io = IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();

                let code = cmds::exec_cmd(self, &res_cmd, args, &[], &mut io);
                self.last_status = code;

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

                let mut io = IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();
                let code = cmds::exec_cmd(self, &res_cmd, args, &[], &mut io);
                self.last_status = code;

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
                        (Value::String(s), Value::Number(n))
                        | (Value::Number(n), Value::String(s)) => {
                            if n.is_nan() || n <= 0.0 {
                                Value::String(String::new())
                            } else if n > 1_000_000.0 || n.is_infinite() {
                                Value::Null
                            } else {
                                Value::String(s.repeat(n.round() as usize))
                            }
                        }
                        _ => Value::Null,
                    },

                    BinaryOp::Div => match (left_res, right_res.cast(left_type)) {
                        (Value::Number(l), Value::Number(r)) => {
                            if r == 0.0 || r.is_nan() {
                                Value::Null
                            } else {
                                Value::Number(l / r)
                            }
                        }
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
            Expr::String(string) | Expr::QuotedString(string) => Value::String(string.to_owned()),
            Expr::Number(num) => Value::Number(*num),
            Expr::Bool(bool) => Value::Bool(*bool),
            Expr::CommandSub(expr) => {
                match expr.as_ref() {
                    Expr::Call { target, args } => {
                        let target_val = self.evaluate_expr(target, true, scope_indices);

                        if let Value::Function(func) = &target_val {
                            let value = self.call_user_function(func, args, scope_indices);
                            return Self::make_command_result(0, value.to_string(), String::new());
                        }

                        let identifier = target_val.to_string();

                        if let Some(Value::Function(func)) =
                            self.resolve_named_value(&identifier, scope_indices)
                        {
                            let value = self.call_user_function(&func, args, scope_indices);
                            return Self::make_command_result(0, value.to_string(), String::new());
                        }

                        let (res_cmd, res_args) = self.resolve_command(identifier);
                        let all_args = self.build_command_args(res_args, args, scope_indices);

                        let mut io = IoCtx {
                            input: io::empty(),
                            out: Vec::<u8>::new(),
                            err: Vec::<u8>::new(),
                            input_mode: IoStreamMode::Pipe,
                            out_mode: IoStreamMode::Pipe,
                            err_mode: IoStreamMode::Pipe,
                        };

                        let exit_status = cmds::exec_cmd(self, &res_cmd, all_args, &[], &mut io);

                        Self::make_command_result(
                            exit_status,
                            String::from_utf8(io.out).unwrap_or_default(),
                            String::from_utf8(io.err).unwrap_or_default(),
                        )
                    }
                    Expr::Word(word) => {
                        if let Some(Value::Function(func)) =
                            self.resolve_named_value(word, scope_indices)
                        {
                            let value = self.call_user_function(&func, &[], scope_indices);
                            return Self::make_command_result(0, value.to_string(), String::new());
                        }

                        let (res_cmd, all_args) = self.resolve_command(word.to_owned());
                        let all_args = expand::expand_args(all_args, ExpandCtx::default());

                        let mut io = IoCtx {
                            input: io::empty(),
                            out: Vec::<u8>::new(),
                            err: Vec::<u8>::new(),
                            input_mode: IoStreamMode::Pipe,
                            out_mode: IoStreamMode::Pipe,
                            err_mode: IoStreamMode::Pipe,
                        };

                        let exit_status = cmds::exec_cmd(self, &res_cmd, all_args, &[], &mut io);

                        Self::make_command_result(
                            exit_status,
                            String::from_utf8(io.out).unwrap_or_default(),
                            String::from_utf8(io.err).unwrap_or_default(),
                        )
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
                Self::make_command_result(
                    code,
                    String::from_utf8(out).unwrap_or_default(),
                    String::from_utf8(err).unwrap_or_default(),
                )
            }
            Expr::AnonymousFunction { params, body } => Value::Function(FunctionValue {
                params: params.clone(),
                body: body.clone(),
            }),
            Expr::Object(expr_map) => {
                let mut obj = HashMap::new();
                for (key, value) in expr_map {
                    let value_value = self.evaluate_expr(value, true, scope_indices);
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

#[cfg(test)]
mod tests {
    use super::*;
    use lsh_syntax::{lexer, parser::Parser};

    fn eval_str(code: &str) -> i32 {
        let tokens = lexer::lex_input(0, code).expect("lex error");
        let mut parser = Parser::new(0, tokens);
        let prog = parser.parse_program().expect("parse error");
        let mut eval = Evaluator::new()
            .with_globals()
            .with_interrupt_flag(Arc::new(AtomicBool::new(false)));
        eval.evaluate(prog)
    }

    #[test]
    fn cd_failure_returns_non_zero_and_stops_and_chain() {
        let status = eval_str("cd /nonexistent_directory_12345 && cd /");
        assert_eq!(status, 1);
    }

    #[test]
    fn pipeline_streaming_returns_last_stage_exit_code() {
        let status = eval_str("echo 'test stream' | cat");
        assert_eq!(status, 0);
    }

    fn eval_var(code: &str, var_name: &str) -> Option<Value> {
        let (eval, _) = eval_with_evaluator(code);
        eval.scopes
            .first()
            .and_then(|s| s.vars.get(var_name).cloned())
    }

    fn eval_with_evaluator(code: &str) -> (Evaluator, i32) {
        let tokens = lexer::lex_input(0, code).expect("lex error");
        let mut parser = Parser::new(0, tokens);
        let prog = parser.parse_program().expect("parse error");
        let mut eval = Evaluator::new()
            .with_globals()
            .with_interrupt_flag(Arc::new(AtomicBool::new(false)));
        let status = eval.evaluate(prog);
        (eval, status)
    }

    #[test]
    fn signal_interrupt_aborts_evaluation_with_status_130() {
        let code = r#"
            let reached_first = 0
            let reached_second = 0
            $reached_first = 1
            $reached_second = 2
        "#;

        let tokens = lexer::lex_input(0, code).expect("lex error");
        let mut parser = Parser::new(0, tokens);
        let prog = parser.parse_program().expect("parse error");
        let flag = Arc::new(AtomicBool::new(true));
        let mut eval = Evaluator::new().with_globals().with_interrupt_flag(flag);
        let status = eval.evaluate(prog);

        assert_eq!(status, STATUS_SIGINT);
        assert_eq!(
            eval.scopes
                .first()
                .and_then(|s| s.vars.get("reached_second")),
            None
        );
    }

    #[test]
    fn signal_interrupt_aborts_while_loop_with_status_130() {
        let code = r#"
            let count = 0
            while true {
                $count = 1
            }
        "#;

        let tokens = lexer::lex_input(0, code).expect("lex error");
        let mut parser = Parser::new(0, tokens);
        let prog = parser.parse_program().expect("parse error");
        let flag = Arc::new(AtomicBool::new(true));
        let mut eval = Evaluator::new().with_globals().with_interrupt_flag(flag);
        let status = eval.evaluate(prog);

        assert_eq!(status, STATUS_SIGINT);
    }

    #[test]
    fn command_sub_on_user_function_returns_object_with_out_and_code() {
        let code = r#"
            fn add(a, b) {
                return $a + $b
            }
            let res = $(add 2 3)
            let out_val = $(add 2 3).out
            let code_val = $(add 2 3).code
        "#;
        assert_eq!(
            eval_var(code, "out_val"),
            Some(Value::String("5".to_string()))
        );
        assert_eq!(eval_var(code, "code_val"), Some(Value::Number(0.0)));
    }

    #[test]
    fn string_multiplication_is_commutative_and_safe() {
        let code = r#"
            let a = "ab" * 3
            let b = 3 * "ab"
            let zero = "ab" * 0
            let neg = "ab" * -2
            let huge = "ab" * 10000000
        "#;
        assert_eq!(
            eval_var(code, "a"),
            Some(Value::String("ababab".to_string()))
        );
        assert_eq!(
            eval_var(code, "b"),
            Some(Value::String("ababab".to_string()))
        );
        assert_eq!(eval_var(code, "zero"), Some(Value::String(String::new())));
        assert_eq!(eval_var(code, "neg"), Some(Value::String(String::new())));
        assert_eq!(eval_var(code, "huge"), Some(Value::Null));
    }

    #[test]
    fn division_by_zero_returns_null() {
        let code = r#"
            let ok = 10 / 2
            let zero = 10 / 0
        "#;
        assert_eq!(eval_var(code, "ok"), Some(Value::Number(5.0)));
        assert_eq!(eval_var(code, "zero"), Some(Value::Null));
    }

    #[test]
    fn null_equality_in_evaluator_conditions() {
        let code = r#"
            let matched_null_first = 0
            let matched_empty_first = 0
            if $missing == "" {
                $matched_null_first = 1
            }
            if "" == $missing {
                $matched_empty_first = 1
            }
        "#;
        assert_eq!(
            eval_var(code, "matched_null_first"),
            Some(Value::Number(1.0))
        );
        assert_eq!(
            eval_var(code, "matched_empty_first"),
            Some(Value::Number(1.0))
        );
    }

    #[test]
    fn build_command_args_expands_unquoted_wildcards_and_preserves_quotes() {
        let mut eval = Evaluator::new().with_interrupt_flag(Arc::new(AtomicBool::new(false)));
        let mut scopes = vec![0];

        // Unquoted wildcard: echo src/*.rs
        let unquoted_code = "echo src/*.rs";
        let tokens = lexer::lex_input(0, unquoted_code).unwrap();
        let mut parser = Parser::new(0, tokens);
        let prog = parser.parse_program().unwrap();
        let call_args = match &prog.body[0] {
            Statement::Command(CommandExpr::Call { args, .. }) => args,
            Statement::Expr(Expr::Call { args, .. }) => args,
            _ => panic!("expected call-like statement"),
        };
        let args = eval.build_command_args(vec![], call_args, &mut scopes);
        assert!(args.contains(&"src/lib.rs".to_string()));
        assert!(args.iter().any(|a| a.ends_with(".rs")));

        // Quoted wildcard: echo "src/*.rs"
        let quoted_code = "echo \"src/*.rs\"";
        let tokens = lexer::lex_input(0, quoted_code).unwrap();
        let mut parser = Parser::new(0, tokens);
        let prog = parser.parse_program().unwrap();
        let call_args = match &prog.body[0] {
            Statement::Command(CommandExpr::Call { args, .. }) => args,
            Statement::Expr(Expr::Call { args, .. }) => args,
            _ => panic!("expected call-like statement"),
        };
        let args = eval.build_command_args(vec![], call_args, &mut scopes);
        assert_eq!(args, vec!["src/*.rs".to_string()]);

        // Nonexistent wildcard fallback
        let fallback_code = "echo nonexistent_*.xyz";
        let tokens = lexer::lex_input(0, fallback_code).unwrap();
        let mut parser = Parser::new(0, tokens);
        let prog = parser.parse_program().unwrap();
        let call_args = match &prog.body[0] {
            Statement::Command(CommandExpr::Call { args, .. }) => args,
            Statement::Expr(Expr::Call { args, .. }) => args,
            _ => panic!("expected call-like statement"),
        };
        let args = eval.build_command_args(vec![], call_args, &mut scopes);
        assert_eq!(args, vec!["nonexistent_*.xyz".to_string()]);
    }
}
