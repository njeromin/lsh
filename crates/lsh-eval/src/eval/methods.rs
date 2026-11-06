use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use lsh_syntax::ast::Expr;

use crate::eval::value::{FunctionValue, Value};

use super::{Evaluator, NamedFunctionWithReceiver};

impl Evaluator {
    pub(crate) fn resolve_named_function_with_receiver(
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
            let next_val = Self::get_child_member(&current_val, part);
            if matches!(next_val, Value::Null) {
                return None;
            }
            current_val = next_val;
        }

        let last_part = parts.last()?;
        let func_val = Self::resolve_member_or_method(&current_val, last_part);

        if let Value::Function(func) = func_val {
            Some((func, Some((receiver_name, current_val))))
        } else {
            None
        }
    }

    pub(crate) fn resolve_array_method(arr: &Arc<RwLock<Vec<Value>>>, method: &str) -> Value {
        match method {
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
        }
    }

    pub(crate) fn resolve_string_method(s: &str, method: &str) -> Value {
        let s_string = s.to_string();
        match method {
            "trim" => Value::function(FunctionValue::native("trim", move |_, _, _| {
                Value::String(s_string.trim().to_string())
            })),
            "trim_start" | "trim_left" => {
                Value::function(FunctionValue::native("trim_start", move |_, _, _| {
                    Value::String(s_string.trim_start().to_string())
                }))
            }
            "trim_end" | "trim_right" => {
                Value::function(FunctionValue::native("trim_end", move |_, _, _| {
                    Value::String(s_string.trim_end().to_string())
                }))
            }
            "len" | "length" => Value::function(FunctionValue::native("len", move |_, _, _| {
                Value::Number(s_string.chars().count() as f64)
            })),
            "to_lowercase" | "to_lower" => {
                Value::function(FunctionValue::native("to_lowercase", move |_, _, _| {
                    Value::String(s_string.to_lowercase())
                }))
            }
            "to_uppercase" | "to_upper" => {
                Value::function(FunctionValue::native("to_uppercase", move |_, _, _| {
                    Value::String(s_string.to_uppercase())
                }))
            }
            "split" => Value::function(FunctionValue::native("split", move |_, args, _| {
                let delimiter = args
                    .first()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| " ".to_string());
                let parts: Vec<Value> = s_string
                    .split(&delimiter)
                    .map(|p| Value::String(p.to_string()))
                    .collect();
                Value::array(parts)
            })),
            "replace" => Value::function(FunctionValue::native("replace", move |_, args, _| {
                let from = args.first().map(|v| v.to_string()).unwrap_or_default();
                let to = args.get(1).map(|v| v.to_string()).unwrap_or_default();
                Value::String(s_string.replace(&from, &to))
            })),
            "starts_with" => {
                Value::function(FunctionValue::native("starts_with", move |_, args, _| {
                    let pat = args.first().map(|v| v.to_string()).unwrap_or_default();
                    Value::Bool(s_string.starts_with(&pat))
                }))
            }
            "ends_with" => {
                Value::function(FunctionValue::native("ends_with", move |_, args, _| {
                    let pat = args.first().map(|v| v.to_string()).unwrap_or_default();
                    Value::Bool(s_string.ends_with(&pat))
                }))
            }
            "contains" => Value::function(FunctionValue::native("contains", move |_, args, _| {
                let pat = args.first().map(|v| v.to_string()).unwrap_or_default();
                Value::Bool(s_string.contains(&pat))
            })),
            _ => Value::Null,
        }
    }

    pub(crate) fn resolve_command_result_method(
        guard: &HashMap<String, Value>,
        method: &str,
    ) -> Option<Value> {
        if !guard.contains_key("code") || !guard.contains_key("out") || !guard.contains_key("err") {
            return None;
        }
        match method {
            "is_ok" | "ok" => {
                let code = match guard.get("code") {
                    Some(Value::Number(n)) => *n as i32,
                    _ => -1,
                };
                Some(Value::function(FunctionValue::native(
                    "is_ok",
                    move |_, _, _| Value::Bool(code == 0),
                )))
            }
            "is_err" | "err" => {
                let code = match guard.get("code") {
                    Some(Value::Number(n)) => *n as i32,
                    _ => -1,
                };
                Some(Value::function(FunctionValue::native(
                    "is_err",
                    move |_, _, _| Value::Bool(code != 0),
                )))
            }
            "lines" => {
                let out = match guard.get("out") {
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };
                Some(Value::function(FunctionValue::native(
                    "lines",
                    move |_, _, _| {
                        let trimmed = out.trim_end_matches(['\r', '\n']);
                        let lines: Vec<Value> = if trimmed.is_empty() {
                            Vec::new()
                        } else {
                            trimmed
                                .split('\n')
                                .map(|line| {
                                    Value::String(
                                        line.strip_suffix('\r').unwrap_or(line).to_string(),
                                    )
                                })
                                .collect()
                        };
                        Value::array(lines)
                    },
                )))
            }
            _ => None,
        }
    }

    pub(crate) fn resolve_member_or_method(receiver: &Value, key: &str) -> Value {
        match receiver {
            Value::Object(map) => {
                let guard = match map.read() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                if let Some(val) = guard.get(key).cloned() {
                    val
                } else if let Some(method_val) = Self::resolve_command_result_method(&guard, key) {
                    method_val
                } else {
                    Value::Null
                }
            }
            Value::Array(arr) => Self::resolve_array_method(arr, key),
            Value::String(s) => Self::resolve_string_method(s, key),
            _ => Value::Null,
        }
    }

    pub(crate) fn get_child_member(parent: &Value, key: &str) -> Value {
        match parent {
            Value::Object(map) => {
                let guard = match map.read() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                if let Some(val) = guard.get(key).cloned() {
                    val
                } else if let Some(method_val) = Self::resolve_command_result_method(&guard, key) {
                    method_val
                } else {
                    Value::Null
                }
            }
            Value::Array(arr) => {
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
                    Self::resolve_array_method(arr, key)
                }
            }
            Value::String(s) => {
                if let Ok(idx) = key.parse::<isize>() {
                    let chars: Vec<char> = s.chars().collect();
                    let actual_idx = if idx < 0 {
                        chars.len() as isize + idx
                    } else {
                        idx
                    };
                    if actual_idx >= 0 && (actual_idx as usize) < chars.len() {
                        Value::String(chars[actual_idx as usize].to_string())
                    } else {
                        Value::Null
                    }
                } else if key == "len" || key == "length" {
                    Value::Number(s.chars().count() as f64)
                } else {
                    Self::resolve_string_method(s, key)
                }
            }
            _ => Value::Null,
        }
    }

    pub(crate) fn resolve_callable_target(
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

                let func_val = match key {
                    Some(k) if k.contains('.') => {
                        let parts: Vec<&str> = k.split('.').collect();
                        let mut current_val = receiver_val;
                        for part in &parts[..parts.len() - 1] {
                            receiver_expr = Expr::Index {
                                object: Box::new(receiver_expr),
                                index: Box::new(Expr::String(part.to_string())),
                            };
                            current_val = Self::get_child_member(&current_val, part);
                            if matches!(current_val, Value::Null) {
                                break;
                            }
                        }
                        receiver_val = current_val;
                        let last_key = parts.last().unwrap();
                        Self::resolve_member_or_method(&receiver_val, last_key)
                    }
                    Some(k) => Self::resolve_member_or_method(&receiver_val, &k),
                    None => Value::Null,
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
}
