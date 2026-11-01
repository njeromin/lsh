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

use lsh_sys::signals::{
    SIGINT, STATUS_SIGINT, TerminalForegroundGuard, get_interrupt_flag, is_interactive_tty,
    signal_exit_code,
};

use crate::{
    eval::value::{FunctionValue, Value},
    expand::{self, ExpandCtx},
    io::{IoCtx, IoStreamMode},
    plugins::PluginManager,
};

#[derive(Debug)]
pub enum EvalError {
    ScopeNotFound,
}

#[derive(Debug, PartialEq)]
pub enum EvalFlow {
    Continue,
    Return(Value),
}

#[derive(Debug, PartialEq)]
pub struct EvalOutcome {
    pub flow: EvalFlow,
    pub last_status: i32,
}

struct ExternalPipelineStage {
    cmd: String,
    args: Vec<String>,
    env_assignments: Vec<(String, String)>,
    stdout_redirect: Option<String>,
}

struct PromptSegmentInvocation {
    _name: String,
    func: Arc<FunctionValue>,
    receiver: Option<(Expr, Value)>,
}

type NamedReceiver = (String, Value);
type NamedFunctionWithReceiver = (Arc<FunctionValue>, Option<NamedReceiver>);

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
    pub plugin_manager: PluginManager,
}

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator {
            scopes: vec![Scope::default()],
            aliases: HashMap::new(),
            last_status: 0,
            interrupted: get_interrupt_flag(),
            completion_registry: Arc::new(RwLock::new(CompletionRegistry::with_defaults())),
            plugin_manager: PluginManager::new(),
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

        Value::object(map)
    }

    pub fn with_globals(mut self) -> Evaluator {
        let scope = self.scopes.first_mut().unwrap(); // TODO: Don't use unwrap.

        let mut segments_map = HashMap::new();
        let newline_fn = Value::function(FunctionValue::native("newline", |_eval, _, _| {
            Value::String(String::from("\n"))
        }));
        segments_map.insert(String::from("newline"), newline_fn.clone());
        segments_map.insert(String::from("\n"), newline_fn);
        segments_map.insert(
            String::from("cwd"),
            Value::function(FunctionValue::native("cwd", |eval, _, _| {
                let cwd = crate::builtins::fmt_cwd::format_cwd_for_eval(eval);
                Value::String(format!("\x1b[1;34m{cwd}\x1b[0m"))
            })),
        );

        let render_fn = Value::function(FunctionValue::native("render", |eval, args, receiver| {
            let order_arg = args.first().and_then(|v| match v {
                Value::String(s) if !s.trim().is_empty() => Some(s.as_str()),
                _ => None,
            });
            let separator_arg = args.get(1).and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });
            let rendered = eval.render_prompt_segments(order_arg, separator_arg, receiver.as_ref());
            Value::String(rendered)
        }));

        let mut prompt_map = HashMap::new();
        prompt_map.insert(
            "text".to_string(),
            Value::String(String::from("\x1b[35m$\x1b[0m ")),
        );
        prompt_map.insert(
            "order".to_string(),
            Value::array(vec![
                Value::String(String::from("newline")),
                Value::String(String::from("cwd")),
            ]),
        );
        prompt_map.insert("separator".to_string(), Value::String(String::from(" ")));
        prompt_map.insert("segments".to_string(), Value::object(segments_map));
        prompt_map.insert("render".to_string(), render_fn);

        scope
            .vars
            .insert(String::from("prompt"), Value::object(prompt_map));

        scope
            .vars
            .insert(String::from("options"), Self::default_options());

        scope.vars.insert(
            String::from("palette"),
            Value::object(HashMap::from([
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
        self.exec_function_with_receiver(func, args, None)
    }

    pub fn exec_function_with_receiver(
        &mut self,
        func: &FunctionValue,
        args: Vec<Value>,
        receiver: Option<(Expr, Value)>,
    ) -> Value {
        let mut scope_indices = vec![0];
        self.call_user_function_with_values_and_receiver(func, args, receiver, &mut scope_indices)
    }

    pub fn trigger_pre_prompt(&mut self) {
        let hooks = self.plugin_manager.get_hooks_for("on_pre_prompt");
        for (func, receiver) in hooks {
            let mut scope_indices = vec![0];
            let rec = Some((Expr::Variable("this".to_string()), receiver));
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![],
                rec,
                &mut scope_indices,
            );
        }
    }

    pub fn trigger_pre_exec(&mut self, cmd_line: &str) {
        let hooks = self.plugin_manager.get_hooks_for("on_pre_exec");
        for (func, receiver) in hooks {
            let mut scope_indices = vec![0];
            let rec = Some((Expr::Variable("this".to_string()), receiver));
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![Value::String(cmd_line.to_string())],
                rec,
                &mut scope_indices,
            );
        }
    }

    pub fn trigger_post_exec(&mut self, cmd_line: &str, exit_code: i32) {
        let hooks = self.plugin_manager.get_hooks_for("on_post_exec");
        for (func, receiver) in hooks {
            let mut scope_indices = vec![0];
            let rec = Some((Expr::Variable("this".to_string()), receiver));
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![
                    Value::String(cmd_line.to_string()),
                    Value::Number(exit_code as f64),
                ],
                rec,
                &mut scope_indices,
            );
        }
    }

    pub fn trigger_chpwd(&mut self, old_pwd: &str, new_pwd: &str) {
        let hooks = self.plugin_manager.get_hooks_for("on_chpwd");
        for (func, receiver) in hooks {
            let mut scope_indices = vec![0];
            let rec = Some((Expr::Variable("this".to_string()), receiver));
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![
                    Value::String(old_pwd.to_string()),
                    Value::String(new_pwd.to_string()),
                ],
                rec,
                &mut scope_indices,
            );
        }
    }

    pub fn load_plugin(&mut self, name_or_path: &str) -> Result<String, String> {
        let path = self
            .plugin_manager
            .find_plugin_file(name_or_path)
            .ok_or_else(|| format!("Plugin '{name_or_path}' not found in search paths."))?;

        let outcome = crate::eval::exec::exec_file_outcome(self, path)?;

        if let EvalFlow::Return(manifest_val @ Value::Object(_)) = outcome.flow {
            let name = self.plugin_manager.register(manifest_val)?;
            self.on_plugin_registered(&name);
            return Ok(name);
        }

        Err(format!(
            "Plugin '{name_or_path}' must return an object manifest (e.g. 'return {{ name = \"...\", ... }}')."
        ))
    }

    pub fn unload_plugin(&mut self, name: &str) -> bool {
        self.on_plugin_unregistered(name);
        self.plugin_manager.unregister(name)
    }

    pub fn on_plugin_registered(&mut self, plugin_name: &str) {
        let segment_names = self.plugin_manager.list_prompt_segments_for(plugin_name);
        if segment_names.is_empty() {
            return;
        }

        let Some(prompt_val) = self.get_var(0, "prompt").cloned() else {
            return;
        };

        let Value::Object(ref prompt_map) = prompt_val else {
            return;
        };

        let mut prompt_guard = prompt_map.write().unwrap_or_else(|p| p.into_inner());

        let segments_obj = prompt_guard
            .entry("segments".to_string())
            .or_insert_with(|| Value::object(HashMap::new()))
            .clone();

        if let Value::Object(ref seg_map) = segments_obj {
            let mut seg_guard = seg_map.write().unwrap_or_else(|p| p.into_inner());
            for seg_name in &segment_names {
                if let Some((func, _)) = self.plugin_manager.get_prompt_segment(seg_name) {
                    seg_guard.insert(seg_name.clone(), Value::Function(func));
                }
            }
        }

        match prompt_guard.get_mut("order") {
            Some(Value::Array(arr)) => {
                if let Ok(mut a) = arr.write() {
                    for seg_name in segment_names {
                        if !a.iter().any(|v| match v {
                            Value::String(s) => s == &seg_name,
                            _ => false,
                        }) {
                            a.push(Value::String(seg_name));
                        }
                    }
                }
            }
            Some(Value::String(order_str)) => {
                let mut order_parts: Vec<String> = order_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                for seg_name in segment_names {
                    if !order_parts.iter().any(|part| part == &seg_name) {
                        order_parts.push(seg_name);
                    }
                }

                *order_str = order_parts.join(" ");
            }
            _ => {
                let items: Vec<Value> = segment_names.into_iter().map(Value::String).collect();
                prompt_guard.insert("order".to_string(), Value::array(items));
            }
        }
    }

    pub fn on_plugin_unregistered(&mut self, plugin_name: &str) {
        let segment_names = self.plugin_manager.list_prompt_segments_for(plugin_name);
        if segment_names.is_empty() {
            return;
        }

        let Some(prompt_val) = self.get_var(0, "prompt").cloned() else {
            return;
        };

        let Value::Object(ref prompt_map) = prompt_val else {
            return;
        };

        let mut prompt_guard = prompt_map.write().unwrap_or_else(|p| p.into_inner());

        if let Some(Value::Object(seg_map)) = prompt_guard.get("segments") {
            let mut seg_guard = seg_map.write().unwrap_or_else(|p| p.into_inner());
            for seg_name in &segment_names {
                seg_guard.remove(seg_name);
            }
        }

        match prompt_guard.get_mut("order") {
            Some(Value::Array(arr)) => {
                if let Ok(mut a) = arr.write() {
                    a.retain(|v| match v {
                        Value::String(s) => !segment_names.contains(s),
                        _ => true,
                    });
                }
            }
            Some(Value::String(order_str)) => {
                let order_parts: Vec<String> = order_str
                    .split_whitespace()
                    .filter(|part| !segment_names.iter().any(|seg| seg == part))
                    .map(|s| s.to_string())
                    .collect();

                *order_str = order_parts.join(" ");
            }
            _ => {}
        }
    }

    pub fn render_prompt_segments(
        &mut self,
        order_arg: Option<&str>,
        separator_arg: Option<&str>,
        receiver: Option<&Value>,
    ) -> String {
        let prompt_val = if let Some(r @ Value::Object(_)) = receiver {
            r.clone()
        } else {
            match self.get_var(0, "prompt").cloned() {
                Some(v @ Value::Object(_)) => v,
                _ => return String::new(),
            }
        };

        let (order_val, separator_str) = if let Value::Object(ref prompt_map) = prompt_val {
            let guard = match prompt_map.read() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let order = guard.get("order").cloned();
            let sep = guard
                .get("separator")
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| " ".to_string());
            (order, sep)
        } else {
            (None, " ".to_string())
        };

        let separator = separator_arg.unwrap_or(&separator_str);

        let segment_names: Vec<String> = if let Some(order_override) = order_arg {
            order_override
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        } else if let Some(ref order) = order_val {
            match order {
                Value::Array(arr) => {
                    let guard = match arr.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    guard.iter().map(|v| v.to_string()).collect()
                }
                Value::String(s) => s.split_whitespace().map(|s| s.to_string()).collect(),
                _ => Vec::new(),
            }
        } else {
            let mut names = Vec::new();
            if let Value::Object(ref prompt_map) = prompt_val {
                let guard = match prompt_map.read() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                if let Some(Value::Object(seg_map)) = guard.get("segments") {
                    let seg_guard = match seg_map.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    names.extend(seg_guard.keys().cloned());
                }
            }
            names
        };

        let mut to_run: Vec<PromptSegmentInvocation> = Vec::new();
        for name in segment_names {
            if let Some((func, manifest)) = self.plugin_manager.get_prompt_segment(&name) {
                let rec = Some((Expr::Variable("this".to_string()), manifest));
                to_run.push(PromptSegmentInvocation {
                    _name: name,
                    func,
                    receiver: rec,
                });
            } else if let Value::Object(ref prompt_map) = prompt_val {
                let guard = match prompt_map.read() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                if let Some(Value::Object(seg_map)) = guard.get("segments") {
                    let seg_guard = match seg_map.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    match seg_guard.get(&name) {
                        Some(Value::Function(func)) => {
                            let rec =
                                Some((Expr::Variable("this".to_string()), prompt_val.clone()));
                            to_run.push(PromptSegmentInvocation {
                                _name: name,
                                func: Arc::clone(func),
                                receiver: rec,
                            });
                        }
                        Some(Value::String(s)) => {
                            let s_clone = s.clone();
                            to_run.push(PromptSegmentInvocation {
                                _name: name,
                                func: Arc::new(FunctionValue::native(
                                    "static_text",
                                    move |_, _, _| Value::String(s_clone.clone()),
                                )),
                                receiver: None,
                            });
                        }
                        _ => {
                            if name == "newline" || name == "\n" {
                                to_run.push(PromptSegmentInvocation {
                                    _name: name,
                                    func: Arc::new(FunctionValue::native(
                                        "newline",
                                        |_eval, _, _| Value::String(String::from("\n")),
                                    )),
                                    receiver: None,
                                });
                            }
                        }
                    }
                } else if name == "newline" || name == "\n" {
                    to_run.push(PromptSegmentInvocation {
                        _name: name,
                        func: Arc::new(FunctionValue::native("newline", |_eval, _, _| {
                            Value::String(String::from("\n"))
                        })),
                        receiver: None,
                    });
                }
            } else if name == "newline" || name == "\n" {
                to_run.push(PromptSegmentInvocation {
                    _name: name,
                    func: Arc::new(FunctionValue::native("newline", |_eval, _, _| {
                        Value::String(String::from("\n"))
                    })),
                    receiver: None,
                });
            }
        }

        let mut rendered = Vec::new();
        for invocation in to_run {
            let is_newline_segment = invocation._name == "newline" || invocation._name == "\n";
            let val =
                self.exec_function_with_receiver(&invocation.func, vec![], invocation.receiver);
            let text = match val {
                Value::String(s) => s,
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            };

            let cleaned = if text == "\n" || is_newline_segment {
                text
            } else {
                text.trim_end_matches('\n').to_string()
            };

            if !cleaned.is_empty() {
                rendered.push(cleaned);
            }
        }

        let mut final_rendered = String::new();
        for part in rendered {
            if part.is_empty() {
                continue;
            }
            if part == "\n" {
                final_rendered.push('\n');
            } else {
                if !final_rendered.is_empty()
                    && !final_rendered.ends_with('\n')
                    && !part.starts_with('\n')
                {
                    final_rendered.push_str(separator);
                }
                final_rendered.push_str(&part);
            }
        }

        final_rendered
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

    pub fn evaluate_program(&mut self, prog: Program) -> EvalOutcome {
        self.evaluate_statements(&prog.body, &mut vec![0])
    }

    pub fn evaluate(&mut self, prog: Program) -> i32 {
        self.evaluate_program(prog).last_status
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
                    let func_value =
                        Value::function(FunctionValue::ast(params.clone(), body.clone()));
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
        Value::object(res)
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
                            if sig == SIGINT && !capture_output {
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
                    let (identifier_val, receiver) =
                        self.resolve_callable_target(identifier, scope_indices);

                    if let Value::Function(func) = &identifier_val {
                        let output = self
                            .call_user_function_with_receiver(func, args, receiver, scope_indices)
                            .to_string()
                            .into_bytes();
                        Self::maybe_redirect_output(output, stdout_redirect_path.as_deref())
                    } else {
                        let identifier_str = identifier_val.to_string();

                        if let Some((func, rec_info)) = self
                            .resolve_named_function_with_receiver(&identifier_str, scope_indices)
                        {
                            let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                            let output = self
                                .call_user_function_with_receiver(&func, args, rec, scope_indices)
                                .to_string()
                                .into_bytes();
                            Self::maybe_redirect_output(output, stdout_redirect_path.as_deref())
                        } else if let Some((func, plugin_manifest)) =
                            self.plugin_manager.get_command(&identifier_str)
                        {
                            let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                            let output = self
                                .call_user_function_with_receiver(&func, args, rec, scope_indices)
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

    fn is_assignable(expr: &Expr) -> bool {
        matches!(expr, Expr::Variable(_) | Expr::Word(_) | Expr::Index { .. })
    }

    fn assign_target(&mut self, target: &Expr, value: Value, scope_indices: &mut Vec<usize>) {
        match target {
            Expr::Variable(var_name) | Expr::Word(var_name) => {
                if var_name.contains('.') {
                    let expanded_target = Self::path_to_expr(var_name);
                    self.assign_target(&expanded_target, value, scope_indices);
                    return;
                }
                self.assign_var(var_name, value, scope_indices);
            }
            Expr::Index { object, index } => {
                let key_value = self.evaluate_expr(index, true, scope_indices);
                let key = Self::object_index_key(key_value)
                    .unwrap_or_else(|| panic!("Invalid object index for assignment."));

                if key.contains('.') {
                    let parts: Vec<&str> = key.split('.').collect();
                    let mut current_obj = *object.clone();
                    for part in &parts[..parts.len() - 1] {
                        current_obj = Expr::Index {
                            object: Box::new(current_obj),
                            index: Box::new(Expr::String(part.to_string())),
                        };
                    }
                    let last_key = parts.last().unwrap().to_string();
                    let expanded_target = Expr::Index {
                        object: Box::new(current_obj),
                        index: Box::new(Expr::String(last_key)),
                    };
                    self.assign_target(&expanded_target, value, scope_indices);
                    return;
                }

                let target_obj = self.evaluate_expr(object, true, scope_indices);
                match target_obj {
                    Value::Object(map) => {
                        let mut m = match map.write() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        m.insert(key, value);
                    }
                    Value::Array(arr) => {
                        let mut a = match arr.write() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        if let Ok(idx) = key.parse::<isize>() {
                            let actual_idx = if idx < 0 { a.len() as isize + idx } else { idx };
                            if actual_idx >= 0 {
                                let u_idx = actual_idx as usize;
                                if u_idx < a.len() {
                                    a[u_idx] = value;
                                } else if u_idx == a.len() {
                                    a.push(value);
                                }
                            }
                        }
                    }
                    Value::Null => {
                        let mut new_map = HashMap::new();
                        new_map.insert(key, value);
                        self.assign_target(object, Value::object(new_map), scope_indices);
                    }
                    _ => panic!("Assignment target must be an object or array member."),
                }
            }
            _ => panic!("Invalid assignment target."),
        }
    }

    fn path_to_expr(path: &str) -> Expr {
        let parts: Vec<&str> = path.split('.').collect();
        let mut expr = Expr::Variable(parts[0].to_string());
        for part in &parts[1..] {
            expr = Expr::Index {
                object: Box::new(expr),
                index: Box::new(Expr::String(part.to_string())),
            };
        }
        expr
    }

    fn resolve_named_function_with_receiver(
        &self,
        identifier: &str,
        scope_indices: &[usize],
    ) -> Option<NamedFunctionWithReceiver> {
        let parts: Vec<&str> = identifier.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        let first = parts[0].strip_prefix('$').unwrap_or(parts[0]);

        if parts.len() == 1 {
            let val = self.scoped_var(first, scope_indices)?;
            if let Value::Function(f) = val {
                return Some((f, None));
            }
            return None;
        }

        let mut current_val = self.scoped_var(first, scope_indices)?.clone();
        let mut receiver_name = first.to_string();

        for part in &parts[1..parts.len() - 1] {
            receiver_name.push('.');
            receiver_name.push_str(part);
            let next_val = match &current_val {
                Value::Object(map) => {
                    let m = match map.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    m.get(*part).cloned()?
                }
                _ => return None,
            };
            current_val = next_val;
        }

        let last_part = parts.last()?;
        let func_val = match &current_val {
            Value::Object(map) => {
                let m = match map.read() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                m.get(*last_part).cloned()?
            }
            _ => return None,
        };

        if let Value::Function(func) = func_val {
            Some((func, Some((receiver_name, current_val))))
        } else {
            None
        }
    }

    fn resolve_callable_target(
        &mut self,
        target: &Expr,
        scope_indices: &mut Vec<usize>,
    ) -> (Value, Option<(Expr, Value)>) {
        match target {
            Expr::Index { object, index } => {
                let mut receiver_expr = *object.clone();
                let mut receiver_val = self.evaluate_expr(object, true, scope_indices);
                let index_val = self.evaluate_expr(index, true, scope_indices);
                let key = Self::object_index_key(index_val);

                let func_val = match (&receiver_val, key) {
                    (Value::Object(map), Some(k)) if k.contains('.') => {
                        let parts: Vec<&str> = k.split('.').collect();
                        let mut current_val = Value::Object(Arc::clone(map));
                        for part in &parts[..parts.len() - 1] {
                            receiver_expr = Expr::Index {
                                object: Box::new(receiver_expr),
                                index: Box::new(Expr::String(part.to_string())),
                            };
                            current_val = match &current_val {
                                Value::Object(m) => {
                                    let guard = match m.read() {
                                        Ok(g) => g,
                                        Err(p) => p.into_inner(),
                                    };
                                    guard.get(*part).cloned().unwrap_or(Value::Null)
                                }
                                _ => Value::Null,
                            };
                        }
                        receiver_val = current_val;
                        let last_key = parts.last().unwrap();
                        match &receiver_val {
                            Value::Object(m) => {
                                let guard = match m.read() {
                                    Ok(g) => g,
                                    Err(p) => p.into_inner(),
                                };
                                guard.get(*last_key).cloned().unwrap_or(Value::Null)
                            }
                            _ => Value::Null,
                        }
                    }
                    (Value::Object(map), Some(k)) => {
                        let guard = match map.read() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        guard.get(&k).cloned().unwrap_or(Value::Null)
                    }
                    (Value::Array(arr), Some(k)) => match k.as_str() {
                        "len" | "length" => {
                            let arr_clone = Arc::clone(arr);
                            Value::function(FunctionValue::native("len", move |_, _, _| {
                                let guard = match arr_clone.read() {
                                    Ok(g) => g,
                                    Err(p) => p.into_inner(),
                                };
                                Value::Number(guard.len() as f64)
                            }))
                        }
                        "push" => {
                            let arr_clone = Arc::clone(arr);
                            Value::function(FunctionValue::native("push", move |_, args, _| {
                                let mut guard = match arr_clone.write() {
                                    Ok(g) => g,
                                    Err(p) => p.into_inner(),
                                };
                                for arg in args {
                                    guard.push(arg);
                                }
                                Value::Number(guard.len() as f64)
                            }))
                        }
                        "pop" => {
                            let arr_clone = Arc::clone(arr);
                            Value::function(FunctionValue::native("pop", move |_, _, _| {
                                let mut guard = match arr_clone.write() {
                                    Ok(g) => g,
                                    Err(p) => p.into_inner(),
                                };
                                guard.pop().unwrap_or(Value::Null)
                            }))
                        }
                        "join" => {
                            let arr_clone = Arc::clone(arr);
                            Value::function(FunctionValue::native("join", move |_, args, _| {
                                let sep = args
                                    .first()
                                    .and_then(|v| match v {
                                        Value::String(s) => Some(s.as_str()),
                                        _ => None,
                                    })
                                    .unwrap_or("");
                                let guard = match arr_clone.read() {
                                    Ok(g) => g,
                                    Err(p) => p.into_inner(),
                                };
                                let s = guard
                                    .iter()
                                    .map(|item| item.to_string())
                                    .collect::<Vec<_>>()
                                    .join(sep);
                                Value::String(s)
                            }))
                        }
                        "contains" => {
                            let arr_clone = Arc::clone(arr);
                            Value::function(FunctionValue::native("contains", move |_, args, _| {
                                let target = args.first().cloned().unwrap_or(Value::Null);
                                let guard = match arr_clone.read() {
                                    Ok(g) => g,
                                    Err(p) => p.into_inner(),
                                };
                                Value::Bool(guard.iter().any(|item| item.check_equality(&target)))
                            }))
                        }
                        _ => Value::Null,
                    },
                    _ => Value::Null,
                };
                (func_val, Some((receiver_expr, receiver_val)))
            }
            Expr::Word(identifier) | Expr::Variable(identifier) => {
                if let Some((func, rec_info)) =
                    self.resolve_named_function_with_receiver(identifier, scope_indices)
                {
                    let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                    (Value::Function(func), rec)
                } else {
                    (self.evaluate_expr(target, true, scope_indices), None)
                }
            }
            _ => (self.evaluate_expr(target, true, scope_indices), None),
        }
    }

    fn resolve_named_value(&self, identifier: &str, scope_indices: &[usize]) -> Option<Value> {
        let mut parts = identifier.split('.');
        let first_part = parts.next()?;
        let mut value = self.scoped_var(first_part, scope_indices)?.clone();

        for part in parts {
            let next_value = match &value {
                Value::Object(map) => {
                    let guard = match map.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    guard.get(part).cloned()
                }
                _ => None,
            }?;
            value = next_value;
        }

        Some(value)
    }

    fn expr_has_unquoted_glob_or_tilde(expr: &Expr) -> (bool, bool) {
        match expr {
            Expr::QuotedString(_) | Expr::InterpolatedString(_) => (false, false),
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
            let val = self.evaluate_expr(arg, true, scope_indices);
            match val {
                Value::Array(arr) => {
                    let guard = match arr.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    for item in guard.iter() {
                        let evaluated = item.to_string();
                        let (has_tilde, has_glob) = (
                            evaluated.starts_with('~'),
                            expand::has_glob_metachars(&evaluated),
                        );
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
                }
                other => {
                    let evaluated = other.to_string();
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
            }
        }

        all_args
    }

    fn call_user_function_with_receiver(
        &mut self,
        func: &FunctionValue,
        args: &[Expr],
        receiver: Option<(Expr, Value)>,
        scope_indices: &mut Vec<usize>,
    ) -> Value {
        let evaluated_args: Vec<Value> = args
            .iter()
            .map(|arg| self.evaluate_expr(arg, true, scope_indices))
            .collect();

        self.call_user_function_with_values_and_receiver(
            func,
            evaluated_args,
            receiver,
            scope_indices,
        )
    }

    fn call_user_function_with_values_and_receiver(
        &mut self,
        func: &FunctionValue,
        evaluated_args: Vec<Value>,
        receiver: Option<(Expr, Value)>,
        scope_indices: &mut Vec<usize>,
    ) -> Value {
        if let crate::eval::value::FunctionKind::Native { ref func, .. } = func.kind {
            let this_val = receiver.map(|(_, ref v)| v.clone());
            return (func.clone())(self, evaluated_args, this_val);
        }

        let scope_index = self.make_scope();
        scope_indices.push(scope_index);

        if let Some(scope) = self.scopes.get_mut(scope_index) {
            if let Some((_, ref this_val)) = receiver {
                scope.vars.insert("this".to_string(), this_val.clone());
            }

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

        let final_this = if receiver.is_some() {
            self.scopes
                .get(scope_index)
                .and_then(|s| s.vars.get("this").cloned())
        } else {
            None
        };

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

        if let (Some((receiver_expr, initial_this)), Some(updated_this)) = (receiver, final_this)
            && updated_this != initial_this
            && Self::is_assignable(&receiver_expr)
        {
            self.assign_target(&receiver_expr, updated_this, scope_indices);
        }

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
                let (target_val, receiver) = self.resolve_callable_target(target, scope_indices);

                if let Value::Function(func) = &target_val {
                    return self.call_user_function_with_receiver(
                        func,
                        args,
                        receiver,
                        scope_indices,
                    );
                }

                // For command execution, we only want to do this if we are not in a secondary expression (like an argument).
                if is_secondary {
                    return target_val;
                }

                let identifier = target_val.to_string();
                if let Some((func, plugin_manifest)) = self.plugin_manager.get_command(&identifier)
                {
                    let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                    return self.call_user_function_with_receiver(&func, args, rec, scope_indices);
                }

                let (res_cmd, res_args) = self.resolve_command(identifier);

                // Concatenate arguments from the input cmd with the provided arguments from the evaluator.
                let args = self.build_command_args(res_args, args, scope_indices);

                let mut io = IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();

                let code = cmds::exec_cmd(self, &res_cmd, args, &[], &mut io);
                self.last_status = code;

                Value::Null
            }
            Expr::Word(word) => {
                if let Some((func, rec_info)) =
                    self.resolve_named_function_with_receiver(word, scope_indices)
                {
                    let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                    return self.call_user_function_with_receiver(&func, &[], rec, scope_indices);
                }

                if let Some((func, plugin_manifest)) = self.plugin_manager.get_command(word) {
                    let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                    return self.call_user_function_with_receiver(&func, &[], rec, scope_indices);
                }

                let (res_cmd, args) = self.resolve_command(word.to_owned());
                let args = expand::expand_args(args, ExpandCtx::default());

                if is_secondary {
                    if let Some(val) = self.scoped_var(word, scope_indices) {
                        return val;
                    }
                    if word.contains('.') {
                        let parts: Vec<&str> = word.split('.').collect();
                        if let Some(base_val) = self.scoped_var(parts[0], scope_indices) {
                            let mut cur = base_val;
                            for part in &parts[1..] {
                                cur = match cur {
                                    Value::Object(map) => {
                                        let guard = match map.read() {
                                            Ok(g) => g,
                                            Err(p) => p.into_inner(),
                                        };
                                        guard.get(*part).cloned().unwrap_or(Value::Null)
                                    }
                                    _ => Value::Null,
                                };
                            }
                            if !matches!(cur, Value::Null) {
                                return cur;
                            }
                        }
                    }
                    if let Ok(env_var) = env::var(word) {
                        return Value::String(env_var);
                    }
                    return Value::String(word.to_owned());
                }

                let mut io = IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();
                let code = cmds::exec_cmd(self, &res_cmd, args, &[], &mut io);
                self.last_status = code;

                Value::Null
            }
            Expr::Variable(var_name) => {
                if var_name.contains('.') {
                    let expanded_target = Self::path_to_expr(var_name);
                    return self.evaluate_expr(&expanded_target, is_secondary, scope_indices);
                }

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
                    BinaryOp::Add => match (left_res, right_res) {
                        (Value::Number(l), Value::Number(r)) => Value::Number(l + r),
                        (Value::String(l), Value::String(r)) => Value::String(l + &r),
                        (Value::String(l), r) => Value::String(l + &r.to_string()),
                        (Value::Array(a1), Value::Array(a2)) => {
                            let mut combined = Vec::new();
                            if let Ok(g1) = a1.read() {
                                combined.extend(g1.clone());
                            }
                            if let Ok(g2) = a2.read() {
                                combined.extend(g2.clone());
                            }
                            Value::array(combined)
                        }
                        (Value::Array(a), other) => {
                            let mut combined = Vec::new();
                            if let Ok(g) = a.read() {
                                combined.extend(g.clone());
                            }
                            combined.push(other);
                            Value::array(combined)
                        }
                        _ => Value::Null,
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

                match (&object_value, Self::object_index_key(index_value)) {
                    (Value::Object(map), Some(key)) => {
                        if key.contains('.') {
                            let mut val = Value::Object(Arc::clone(map));
                            for part in key.split('.') {
                                val = match &val {
                                    Value::Object(m) => {
                                        let guard = match m.read() {
                                            Ok(g) => g,
                                            Err(p) => p.into_inner(),
                                        };
                                        guard.get(part).cloned().unwrap_or(Value::Null)
                                    }
                                    _ => Value::Null,
                                };
                                if matches!(val, Value::Null) {
                                    break;
                                }
                            }
                            val
                        } else {
                            let guard = match map.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            guard.get(&key).cloned().unwrap_or(Value::Null)
                        }
                    }
                    (Value::Array(arr), Some(key)) => {
                        if let Ok(idx) = key.parse::<isize>() {
                            let guard = match arr.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            let actual_idx = if idx < 0 {
                                guard.len() as isize + idx
                            } else {
                                idx
                            };
                            if actual_idx >= 0 && (actual_idx as usize) < guard.len() {
                                guard[actual_idx as usize].clone()
                            } else {
                                Value::Null
                            }
                        } else if key == "len" || key == "length" {
                            let guard = match arr.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            Value::Number(guard.len() as f64)
                        } else {
                            Value::Null
                        }
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
            Expr::InterpolatedString(parts) => {
                let mut buf = String::new();
                for part in parts {
                    let val = self.evaluate_expr(part, true, scope_indices);
                    match val {
                        Value::Object(map) => {
                            let guard = match map.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            if let (Some(_code), Some(out)) = (guard.get("code"), guard.get("out"))
                            {
                                let out_str = out.to_string();
                                buf.push_str(out_str.trim_end_matches(['\r', '\n']));
                            } else {
                                drop(guard);
                                buf.push_str(&Value::Object(map).to_string());
                            }
                        }
                        other => {
                            buf.push_str(&other.to_string());
                        }
                    }
                }
                Value::String(buf)
            }
            Expr::Number(num) => Value::Number(*num),
            Expr::Bool(bool) => Value::Bool(*bool),
            Expr::CommandSub(expr) => {
                match expr.as_ref() {
                    Expr::Call { target, args } => {
                        let (target_val, receiver) =
                            self.resolve_callable_target(target, scope_indices);

                        if let Value::Function(func) = &target_val {
                            let value = self.call_user_function_with_receiver(
                                func,
                                args,
                                receiver,
                                scope_indices,
                            );
                            return Self::make_command_result(0, value.to_string(), String::new());
                        }

                        let identifier = target_val.to_string();

                        if let Some((func, rec_info)) =
                            self.resolve_named_function_with_receiver(&identifier, scope_indices)
                        {
                            let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                            let value = self.call_user_function_with_receiver(
                                &func,
                                args,
                                rec,
                                scope_indices,
                            );
                            return Self::make_command_result(0, value.to_string(), String::new());
                        }

                        if let Some((func, plugin_manifest)) =
                            self.plugin_manager.get_command(&identifier)
                        {
                            let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                            let value = self.call_user_function_with_receiver(
                                &func,
                                args,
                                rec,
                                scope_indices,
                            );
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
                        if let Some((func, rec_info)) =
                            self.resolve_named_function_with_receiver(word, scope_indices)
                        {
                            let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                            let value = self.call_user_function_with_receiver(
                                &func,
                                &[],
                                rec,
                                scope_indices,
                            );
                            return Self::make_command_result(0, value.to_string(), String::new());
                        }

                        if let Some((func, plugin_manifest)) = self.plugin_manager.get_command(word)
                        {
                            let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                            let value = self.call_user_function_with_receiver(
                                &func,
                                &[],
                                rec,
                                scope_indices,
                            );
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
            Expr::AnonymousFunction { params, body } => {
                Value::function(FunctionValue::ast(params.clone(), body.clone()))
            }
            Expr::Object(expr_map) => {
                let mut obj = HashMap::new();
                for (key, value) in expr_map {
                    let value_value = self.evaluate_expr(value, true, scope_indices);
                    obj.insert(key.to_string(), value_value);
                }
                Value::object(obj)
            }
            Expr::Array(elements) => {
                let mut evaluated = Vec::with_capacity(elements.len());
                for elem in elements {
                    evaluated.push(self.evaluate_expr(elem, true, scope_indices));
                }
                Value::array(evaluated)
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

    #[test]
    fn method_call_binds_this_and_reads_properties() {
        let res = eval_var(
            r#"
            let person = {
                name = "Alice",
                greet = () => {
                    return "Hello, " + $this.name
                }
            }
            let msg = $person.greet()
            "#,
            "msg",
        );
        assert_eq!(res, Some(Value::String("Hello, Alice".to_string())));
    }

    #[test]
    fn method_call_mutates_this_and_persists_to_receiver() {
        let res = eval_var(
            r#"
            let counter = {
                count = 0,
                inc = () => {
                    $this.count = $this.count + 1
                    return $this.count
                }
            }
            let first = $counter.inc()
            let second = $counter.inc()
            let final_count = $counter.count
            "#,
            "final_count",
        );
        assert_eq!(res, Some(Value::Number(2.0)));
    }

    #[test]
    fn command_style_method_call_binds_this_and_args() {
        let res = eval_var(
            r#"
            let greeter = {
                prefix = "Hi",
                greet = (target) => {
                    return $this.prefix + " " + $target
                }
            }
            let msg = $(greeter.greet "Bob").out
            "#,
            "msg",
        );
        assert_eq!(res, Some(Value::String("Hi Bob".to_string())));
    }

    #[test]
    fn nested_method_calls_on_this() {
        let res = eval_var(
            r#"
            let counter = {
                count = 0,
                inc = () => {
                    $this.count = $this.count + 1
                    return $this.count
                },
                add_two = () => {
                    $this.inc()
                    $this.inc()
                    return $this.count
                }
            }
            $counter.add_two()
            let final_count = $counter.count
            "#,
            "final_count",
        );
        assert_eq!(res, Some(Value::Number(2.0)));
    }

    #[test]
    fn nested_object_method_call_updates_nested_property() {
        let res = eval_var(
            r#"
            let user = {
                stats = {
                    score = 10,
                    add = (pts) => {
                        $this.score = $this.score + $pts
                    }
                }
            }
            $user.stats.add 15
            let final_score = $user.stats.score
            "#,
            "final_score",
        );
        assert_eq!(res, Some(Value::Number(25.0)));
    }

    #[test]
    fn fn_declaration_on_object_binds_this() {
        let res = eval_var(
            r#"
            let obj = { x = 42 }
            fn obj.get_x() {
                return $this.x
            }
            let val = $obj.get_x()
            "#,
            "val",
        );
        assert_eq!(res, Some(Value::Number(42.0)));
    }

    #[test]
    fn object_variables_share_reference_mutations() {
        let res = eval_var(
            r#"
            let a = { count = 1 }
            let b = $a
            $b.count = 42
            let res = $a.count
            "#,
            "res",
        );
        assert_eq!(res, Some(Value::Number(42.0)));
    }

    #[test]
    fn function_mutates_passed_object_reference() {
        let res = eval_var(
            r#"
            fn mutate_obj(target) {
                $target.modified = true
            }
            let my_obj = { modified = false }
            mutate_obj $my_obj
            let res = $my_obj.modified
            "#,
            "res",
        );
        assert_eq!(res, Some(Value::Bool(true)));
    }

    #[test]
    fn object_reference_equality_in_evaluator() {
        let res = eval_var(
            r#"
            let a = { x = 1 }
            let b = $a
            let c = { x = 1 }
            let same_ref = $a == $b
            "#,
            "same_ref",
        );
        assert_eq!(res, Some(Value::Bool(true)));

        let res_deep = eval_var(
            r#"
            let a = { x = 1 }
            let c = { x = 1 }
            let deep_eq = $a == $c
            "#,
            "deep_eq",
        );
        assert_eq!(res_deep, Some(Value::Bool(true)));
    }

    fn write_test_plugin(
        plugin_name: &str,
        content: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!(
            "lsh_test_{}_{}_{}",
            plugin_name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let plugin_dir = temp_dir.join(plugin_name);
        let _ = std::fs::create_dir_all(&plugin_dir);
        let plugin_file = plugin_dir.join("plugin.lsh");
        std::fs::write(&plugin_file, content).unwrap();
        (temp_dir, plugin_file)
    }

    #[test]
    fn plugin_return_and_command_execution_with_this() {
        let (temp_dir, _) = write_test_plugin(
            "test-plugin",
            r#"
            return {
                name = "test-plugin",
                version = "1.0.0",
                description = "Test plugin",
                prefix = ">>",
                commands = {
                    echo_prefixed = (msg) => {
                        return $this.prefix + " " + $msg
                    }
                }
            }
            "#,
        );

        let (mut eval, _) = eval_with_evaluator("");
        eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
        let res = eval.load_plugin("test-plugin");
        assert_eq!(res, Ok("test-plugin".to_string()));

        let _ = eval.evaluate_statements(
            &lsh_syntax::parser::Parser::new(
                0,
                lsh_syntax::lexer::lex_input(0, "let out = $(echo_prefixed \"hello\").out")
                    .unwrap(),
            )
            .parse_program()
            .unwrap()
            .body,
            &mut vec![0],
        );

        let out = eval.scopes.first().and_then(|s| s.vars.get("out").cloned());
        assert_eq!(out, Some(Value::String(">> hello".to_string())));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn plugin_lifecycle_hooks_mutate_plugin_state() {
        let (temp_dir, _) = write_test_plugin(
            "git-plugin",
            r#"
            return {
                name = "git-plugin",
                hooks = {
                    on_chpwd = (old_p, new_p) => {
                        $this.current_dir = $new_p
                    },
                    on_pre_prompt = () => {
                        $this.prompt_runs = 1
                    }
                },
                current_dir = "",
                prompt_runs = 0
            }
            "#,
        );

        let (mut eval, _) = eval_with_evaluator("");
        eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
        assert_eq!(eval.load_plugin("git-plugin"), Ok("git-plugin".to_string()));

        eval.trigger_chpwd("/home/user", "/home/user/projects");
        eval.trigger_pre_prompt();

        let plugin = eval
            .plugin_manager
            .get("git-plugin")
            .expect("plugin not found");
        let current_dir = plugin.manifest.get_property("current_dir");
        let prompt_runs = plugin.manifest.get_property("prompt_runs");

        assert_eq!(
            current_dir,
            Some(Value::String("/home/user/projects".to_string()))
        );
        assert_eq!(prompt_runs, Some(Value::Number(1.0)));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn plugin_unload_removes_commands_and_hooks() {
        let (temp_dir, _) = write_test_plugin(
            "temp-plugin",
            r#"
            return {
                name = "temp-plugin",
                commands = {
                    temp_cmd = () => "temp"
                }
            }
            "#,
        );

        let (mut eval, _) = eval_with_evaluator("");
        eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
        assert_eq!(
            eval.load_plugin("temp-plugin"),
            Ok("temp-plugin".to_string())
        );

        assert!(eval.plugin_manager.get("temp-plugin").is_some());
        assert!(eval.plugin_manager.get_command("temp_cmd").is_some());

        assert!(eval.unload_plugin("temp-plugin"));
        assert!(eval.plugin_manager.get("temp-plugin").is_none());
        assert!(eval.plugin_manager.get_command("temp_cmd").is_none());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn plugin_load_from_file_registers_manifest() {
        let (temp_dir, plugin_file) = write_test_plugin(
            "file-plugin",
            r#"
            return {
                name = "file-plugin",
                version = "2.0.0",
                commands = {
                    file_cmd = () => "from-file"
                }
            }
            "#,
        );

        let (mut eval, _) = eval_with_evaluator("");
        let load_res = eval.load_plugin(plugin_file.to_str().unwrap());
        assert_eq!(load_res, Ok("file-plugin".to_string()));

        let plugin = eval.plugin_manager.get("file-plugin");
        assert!(plugin.is_some());
        assert_eq!(plugin.unwrap().version.as_deref(), Some("2.0.0"));
        assert!(eval.plugin_manager.get_command("file_cmd").is_some());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn plugin_fails_to_load_if_not_returning_object() {
        let (temp_dir, plugin_file) = write_test_plugin(
            "invalid-plugin",
            r#"
            let plugin = {
                name = "invalid-plugin"
            }
            "#,
        );

        let (mut eval, _) = eval_with_evaluator("");
        let load_res = eval.load_plugin(plugin_file.to_str().unwrap());
        assert!(load_res.is_err());
        assert!(
            load_res
                .unwrap_err()
                .contains("must return an object manifest")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn loads_git_prompt_plugin_from_config_directory() {
        let (mut eval, _) = eval_with_evaluator("");

        let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("config")
            .join("plugins");

        eval.plugin_manager
            .search_paths
            .insert(0, manifest_path.clone());

        let load_res = eval.load_plugin("git-prompt");
        assert_eq!(load_res, Ok("git-prompt".to_string()));

        let prompt_obj = eval.get_var(0, "prompt").expect("prompt should exist");
        let order = prompt_obj.get_property("order");
        assert_eq!(
            order,
            Some(Value::array(vec![
                Value::String("newline".to_string()),
                Value::String("cwd".to_string()),
                Value::String("git".to_string()),
            ]))
        );

        let segments = prompt_obj
            .get_property("segments")
            .expect("segments exists");
        assert!(segments.get_property("cwd").is_some());
        assert!(segments.get_property("git").is_some());

        let rendered = eval.render_prompt_segments(None, None, None);
        assert!(!rendered.is_empty());

        // Test custom order and separator
        let custom_rendered = eval.render_prompt_segments(Some("git"), None, None);
        assert!(!custom_rendered.is_empty());

        let (mut script_eval, _) = eval_with_evaluator("");
        script_eval
            .plugin_manager
            .search_paths
            .insert(0, manifest_path);
        let _ = script_eval.load_plugin("git-prompt");
        let render_call_res = script_eval.evaluate_statements(
            &lsh_syntax::parser::Parser::new(
                0,
                lsh_syntax::lexer::lex_input(0, "let out = $prompt.render()").unwrap(),
            )
            .parse_program()
            .unwrap()
            .body,
            &mut vec![0],
        );
        assert_eq!(render_call_res.flow, crate::eval::EvalFlow::Continue);
        assert!(script_eval.get_var(0, "out").is_some());
    }

    #[test]
    fn prompt_segments_custom_registration_and_unload() {
        let (temp_dir, _) = write_test_plugin(
            "test-plugin",
            r#"
            return {
                name = "test-plugin",
                prompt_segments = {
                    alpha = () => {
                        return "ALPHA"
                    },
                    empty_seg = () => {
                        return ""
                    }
                }
            }
            "#,
        );

        let (mut eval, _) = eval_with_evaluator("");
        eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
        assert_eq!(
            eval.load_plugin("test-plugin"),
            Ok("test-plugin".to_string())
        );

        let prompt_obj = eval.get_var(0, "prompt").expect("prompt exists");
        let order = prompt_obj.get_property("order").unwrap().to_string();
        assert!(order.contains("alpha"));
        assert!(order.contains("empty_seg"));

        // empty_seg returns "" so it must be omitted
        let rendered = eval.render_prompt_segments(Some("alpha empty_seg"), None, None);
        assert_eq!(rendered, "ALPHA");

        // Custom separator
        let rendered_sep = eval.render_prompt_segments(Some("cwd alpha"), Some(" -> "), None);
        assert!(rendered_sep.contains(" -> ALPHA"));

        // Unload removes segments
        assert!(eval.unload_plugin("test-plugin"));
        let prompt_after_unload = eval.get_var(0, "prompt").expect("prompt exists");
        let order_after = prompt_after_unload
            .get_property("order")
            .unwrap()
            .to_string();
        assert!(!order_after.contains("alpha"));
        assert!(!order_after.contains("empty_seg"));
        assert_eq!(order_after, "[\"newline\", \"cwd\"]");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn prompt_newline_segment_behavior() {
        let (mut eval, _) = eval_with_evaluator("");

        // 1. Leading newline is emitted cleanly without separator space after it
        let rendered = eval.render_prompt_segments(Some("newline cwd"), None, None);
        assert!(
            rendered.starts_with('\n'),
            "Leading newline should be emitted"
        );
        assert!(
            !rendered.starts_with("\n "),
            "Newline should not have separator space after it"
        );
        assert!(rendered.contains("\x1b[1;34m"));

        // 2. Omission of newline gives a compact prompt without newline
        let rendered_compact = eval.render_prompt_segments(Some("cwd"), None, None);
        assert!(!rendered_compact.starts_with('\n'));

        // 3. Middle newline is emitted cleanly for multi-line prompts
        let rendered_multiline = eval.render_prompt_segments(Some("cwd newline cwd"), None, None);
        assert!(rendered_multiline.contains('\n'));
        assert!(!rendered_multiline.contains(" \n"));
        assert!(!rendered_multiline.contains("\n "));
    }

    #[test]
    fn native_array_literals_indexing_methods_and_operations() {
        let (mut eval, _) = eval_with_evaluator("");

        let script = r#"
        let arr = ["first", 20, "third"];
        let first = $arr[0];
        let last = $arr[-1];
        let len_prop = $arr.len;
        let len_call = $arr.len();
        
        $arr[0] = "updated_first";
        $arr.push("fourth");
        let popped = $arr.pop();
        let joined = $arr.join("-");
        let has_twenty = $arr.contains(20);
        let has_missing = $arr.contains("missing");

        let combined = [1, 2] + [3, 4];
        let appended = [1, 2] + 3;
        "#;

        let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
            .parse_program()
            .unwrap();
        eval.evaluate_statements(&statements.body, &mut vec![0]);

        assert_eq!(
            eval.get_var(0, "first"),
            Some(&Value::String("first".to_string()))
        );
        assert_eq!(
            eval.get_var(0, "last"),
            Some(&Value::String("third".to_string()))
        );
        assert_eq!(eval.get_var(0, "len_prop"), Some(&Value::Number(3.0)));
        assert_eq!(eval.get_var(0, "len_call"), Some(&Value::Number(3.0)));

        let arr_val = eval.get_var(0, "arr").expect("arr exists");
        if let Value::Array(a) = arr_val {
            let guard = a.read().unwrap();
            assert_eq!(guard[0], Value::String("updated_first".to_string()));
            assert_eq!(guard.len(), 3);
        } else {
            panic!("Expected Value::Array");
        }

        assert_eq!(
            eval.get_var(0, "popped"),
            Some(&Value::String("fourth".to_string()))
        );
        assert_eq!(
            eval.get_var(0, "joined"),
            Some(&Value::String("updated_first-20-third".to_string()))
        );
        assert_eq!(eval.get_var(0, "has_twenty"), Some(&Value::Bool(true)));
        assert_eq!(eval.get_var(0, "has_missing"), Some(&Value::Bool(false)));

        let combined_val = eval.get_var(0, "combined").expect("combined exists");
        assert_eq!(
            combined_val,
            &Value::array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Number(4.0),
            ])
        );

        let appended_val = eval.get_var(0, "appended").expect("appended exists");
        assert_eq!(
            appended_val,
            &Value::array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
            ])
        );
    }

    #[test]
    fn prompt_order_with_native_array_literal() {
        let (mut eval, _) = eval_with_evaluator("");

        let script = r#"
        $prompt.order = ["cwd"];
        let rendered = $prompt.render();
        "#;

        let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
            .parse_program()
            .unwrap();
        eval.evaluate_statements(&statements.body, &mut vec![0]);

        let rendered = eval.get_var(0, "rendered").expect("rendered exists");
        assert!(!rendered.to_string().is_empty());
    }

    #[test]
    fn command_with_array_expansion_and_indexing() {
        let (mut eval, _) = eval_with_evaluator("");

        let script = r#"
        let words = ["one", "two", "three"];
        let out_full = $(echo $words).out;
        let out_idx = $(echo $words[0] $words[-1]).out;
        "#;

        let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
            .parse_program()
            .unwrap();
        eval.evaluate_statements(&statements.body, &mut vec![0]);

        let out_full = eval.get_var(0, "out_full").unwrap().to_string();
        let out_idx = eval.get_var(0, "out_idx").unwrap().to_string();
        assert_eq!(out_full.trim(), "one two three");
        assert_eq!(out_idx.trim(), "one three");

        let script_brackets = r#"
        let out_brackets = $(echo [0-9]).out;
        let out_compound = $(echo --flag=[val]).out;
        "#;
        let stmts_brackets = Parser::new(0, lexer::lex_input(0, script_brackets).unwrap())
            .parse_program()
            .unwrap();
        eval.evaluate_statements(&stmts_brackets.body, &mut vec![0]);

        let out_brackets = eval.get_var(0, "out_brackets").unwrap().to_string();
        let out_compound = eval.get_var(0, "out_compound").unwrap().to_string();
        assert_eq!(out_brackets.trim(), "[0-9]");
        assert_eq!(out_compound.trim(), "--flag=[val]");
    }

    #[test]
    fn command_with_escapes_and_line_continuations() {
        let (mut eval, _) = eval_with_evaluator("");

        let script = "let out_space = $(echo My\\ Documents).out;\n\
                      let out_escapes = $(echo {} \\; \\$NOT_EXPANDED).out;\n\
                      let out_continuation = $(echo line1 \\\n line2).out;";

        let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
            .parse_program()
            .unwrap();
        eval.evaluate_statements(&statements.body, &mut vec![0]);

        let out_space = eval.get_var(0, "out_space").unwrap().to_string();
        let out_escapes = eval.get_var(0, "out_escapes").unwrap().to_string();
        let out_continuation = eval.get_var(0, "out_continuation").unwrap().to_string();

        assert_eq!(out_space.trim(), "My Documents");
        assert_eq!(out_escapes.trim(), "{} ; $NOT_EXPANDED");
        assert_eq!(out_continuation.trim(), "line1 line2");
    }

    #[test]
    fn evaluates_interpolated_string_with_variables_and_expressions() {
        let code = r#"
            let name = "world"
            let msg = "Hello $name!"
            let a = 10
            let b = 20
            let calc = "Sum: ${a + b}"
            let single = 'Raw $name unexpanded'
            let escaped = "Cost: \$100"
        "#;
        assert_eq!(
            eval_var(code, "msg"),
            Some(Value::String("Hello world!".to_string()))
        );
        assert_eq!(
            eval_var(code, "calc"),
            Some(Value::String("Sum: 30".to_string()))
        );
        assert_eq!(
            eval_var(code, "single"),
            Some(Value::String("Raw $name unexpanded".to_string()))
        );
        assert_eq!(
            eval_var(code, "escaped"),
            Some(Value::String("Cost: $100".to_string()))
        );
    }

    #[test]
    fn evaluates_interpolated_string_with_dot_path_and_indexing() {
        let code = r#"
            let user = { name = "Alice", role = "admin" }
            let greeting = "Welcome ${user.name} (${$user.role})"
            let items = ["first", "second"]
            let item_msg = "Item 0: ${items[0]}"
        "#;
        assert_eq!(
            eval_var(code, "greeting"),
            Some(Value::String("Welcome Alice (admin)".to_string()))
        );
        assert_eq!(
            eval_var(code, "item_msg"),
            Some(Value::String("Item 0: first".to_string()))
        );
    }

    #[test]
    fn evaluates_interpolated_string_with_command_sub() {
        let code = r#"
            let msg = "Output: $(echo hi)"
        "#;
        assert_eq!(
            eval_var(code, "msg"),
            Some(Value::String("Output: hi".to_string()))
        );
    }

    #[test]
    fn evaluates_interpolated_string_in_command_arguments() {
        let (mut eval, _) = eval_with_evaluator("");
        let script = r#"
            let who = "LSH"
            let out = $(echo "Hello $who!").out
        "#;
        let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
            .parse_program()
            .unwrap();
        eval.evaluate_statements(&statements.body, &mut vec![0]);
        let out = eval.get_var(0, "out").unwrap().to_string();
        assert_eq!(out.trim(), "Hello LSH!");
    }
}
