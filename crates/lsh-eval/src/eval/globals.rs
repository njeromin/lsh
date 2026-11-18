use std::{collections::HashMap, env, sync::Arc};

use crate::eval::{
    EvalError, Evaluator,
    value::{FunctionValue, Value},
};

impl Evaluator {
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
        let Some(scope) = self.scopes.first_mut() else {
            return self;
        };

        let fmt_cwd_fn = Value::function(FunctionValue::native("fmt_cwd", |eval, args, _| {
            let formatted = if let Some(path_arg) = args.first() {
                let path =
                    lsh_syntax::paths::expand_tilde(std::path::PathBuf::from(path_arg.to_string()));
                if let Some(home_dir) = crate::builtins::fmt_cwd::resolve_home_dir(eval) {
                    crate::builtins::fmt_cwd::format_cwd(path.as_path(), home_dir.as_path())
                } else {
                    path.to_string_lossy().into_owned()
                }
            } else {
                crate::builtins::fmt_cwd::format_cwd_for_eval(eval)
            };
            Value::String(formatted)
        }));
        scope
            .vars
            .insert(String::from("fmt_cwd"), fmt_cwd_fn.clone());

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
        prompt_map.insert("separator".to_string(), Value::String(String::from(" ")));
        prompt_map.insert(
            "segments".to_string(),
            Value::array(vec![
                Value::String(String::from("\n")),
                fmt_cwd_fn,
                Value::String(String::from("\n")),
                Value::String(String::from("\x1b[35m›\x1b[0m ")),
            ]),
        );
        prompt_map.insert("render".to_string(), render_fn);

        scope
            .vars
            .insert(String::from("prompt"), Value::object(prompt_map));

        let import_fn =
            Value::function(FunctionValue::native("import", |eval, args, _receiver| {
                let Some(target) = args.first() else {
                    return Value::Null;
                };
                let target_str = match target {
                    Value::String(s) => s.clone(),
                    _ => target.to_string(),
                };
                match eval.import_module(&target_str) {
                    Ok(val) => val,
                    Err(err) => {
                        eprintln!("import error: {err}");
                        Value::Null
                    }
                }
            }));
        scope.vars.insert(String::from("import"), import_fn);

        let hooks_registry = Arc::clone(&self.hook_registry);

        let add_fn = {
            let reg = Arc::clone(&hooks_registry);
            Value::function(FunctionValue::native("add", move |_eval, args, _| {
                let Some(event_val) = args.first() else {
                    return Value::Bool(false);
                };
                let Some(func_val) = args.get(1) else {
                    return Value::Bool(false);
                };
                let event = match event_val {
                    Value::String(s) => s.as_str(),
                    _ => return Value::Bool(false),
                };
                let Value::Function(f) = func_val else {
                    return Value::Bool(false);
                };
                reg.write()
                    .unwrap_or_else(|p| p.into_inner())
                    .add(event, Arc::clone(f));
                Value::Bool(true)
            }))
        };

        let remove_fn = {
            let reg = Arc::clone(&hooks_registry);
            Value::function(FunctionValue::native("remove", move |_eval, args, _| {
                let Some(event_val) = args.first() else {
                    return Value::Bool(false);
                };
                let Some(func_val) = args.get(1) else {
                    return Value::Bool(false);
                };
                let event = match event_val {
                    Value::String(s) => s.as_str(),
                    _ => return Value::Bool(false),
                };
                let Value::Function(f) = func_val else {
                    return Value::Bool(false);
                };
                let removed = reg
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(event, f);
                Value::Bool(removed)
            }))
        };

        let clear_fn = {
            let reg = Arc::clone(&hooks_registry);
            Value::function(FunctionValue::native("clear", move |_eval, args, _| {
                let event = args.first().and_then(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                });
                reg.write().unwrap_or_else(|p| p.into_inner()).clear(event);
                Value::Null
            }))
        };

        let list_fn = {
            let reg = Arc::clone(&hooks_registry);
            Value::function(FunctionValue::native("list", move |_eval, _args, _| {
                let list = reg.read().unwrap_or_else(|p| p.into_inner()).list();
                let map = list
                    .into_iter()
                    .map(|(k, count)| (k, Value::Number(count as f64)))
                    .collect();
                Value::object(map)
            }))
        };

        let mut hooks_map = HashMap::new();
        hooks_map.insert("add".to_string(), add_fn.clone());
        hooks_map.insert("on".to_string(), add_fn);
        hooks_map.insert("remove".to_string(), remove_fn);
        hooks_map.insert("clear".to_string(), clear_fn);
        hooks_map.insert("list".to_string(), list_fn);

        scope
            .vars
            .insert(String::from("hooks"), Value::object(hooks_map));

        let json_parse_fn =
            Value::function(FunctionValue::try_native("parse", |_eval, args, _| {
                let Some(arg) = args.first() else {
                    return Err(EvalError::InvalidJson(
                        "Expected JSON string or command result argument to json.parse()."
                            .to_string(),
                    ));
                };
                match arg {
                    Value::String(s) => crate::eval::json::parse_json(s)
                        .map_err(|e| EvalError::InvalidJson(format!("Invalid JSON: {e}"))),
                    Value::Object(map) => {
                        let guard = match map.read() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        if let Some(Value::String(out)) = guard.get("out") {
                            crate::eval::json::parse_json(out)
                                .map_err(|e| EvalError::InvalidJson(format!("Invalid JSON: {e}")))
                        } else {
                            Err(EvalError::TypeError(
                                "Cannot parse JSON from object without 'out' string property."
                                    .to_string(),
                            ))
                        }
                    }
                    other => Err(EvalError::TypeError(format!(
                        "Cannot parse JSON from value of type '{}'.",
                        other.get_type()
                    ))),
                }
            }));

        let json_stringify_fn =
            Value::function(FunctionValue::native("stringify", |_eval, args, _| {
                let val = args.first().cloned().unwrap_or(Value::Null);
                let indent = match args.get(1) {
                    Some(Value::Bool(true)) => Some(2),
                    Some(Value::Number(n)) if *n >= 1.0 => Some((*n as usize).min(16)),
                    _ => None,
                };
                Value::String(crate::eval::json::stringify_json(&val, indent))
            }));

        let mut json_map = HashMap::new();
        json_map.insert("parse".to_string(), json_parse_fn);
        json_map.insert("stringify".to_string(), json_stringify_fn);
        scope
            .vars
            .insert(String::from("json"), Value::object(json_map));

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
        if let Ok(path) = env::var("PATH") {
            scope.vars.insert(String::from("PATH"), Value::String(path));
        }

        self
    }
}
