use lsh_syntax::ast::Expr;

use crate::eval::{Evaluator, value::Value};

impl Evaluator {
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

        let (segments_val, order_val, separator_str) =
            if let Value::Object(ref prompt_map) = prompt_val {
                let guard = match prompt_map.read() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                let segs = guard.get("segments").cloned();
                let order = guard.get("order").cloned();
                let sep = guard
                    .get("separator")
                    .and_then(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| " ".to_string());
                (segs, order, sep)
            } else {
                (None, None, " ".to_string())
            };

        let separator = separator_arg.unwrap_or(&separator_str);

        let items: Vec<Value> = if let Some(order_override) = order_arg {
            order_override
                .split_whitespace()
                .map(|s| Value::String(s.to_string()))
                .collect()
        } else if let Some(Value::Array(arr)) = segments_val {
            let guard = match arr.read() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            guard.clone()
        } else if let Some(ref order) = order_val {
            match order {
                Value::Array(arr) => {
                    let guard = match arr.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    guard.clone()
                }
                Value::String(s) => s
                    .split_whitespace()
                    .map(|w| Value::String(w.to_string()))
                    .collect(),
                _ => Vec::new(),
            }
        } else if let Some(Value::Object(seg_map)) = segments_val {
            let guard = match seg_map.read() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            guard.values().cloned().collect()
        } else {
            Vec::new()
        };

        let mut rendered: Vec<String> = Vec::new();
        for item in items {
            match item {
                Value::Function(func) => {
                    let rec = if func.bound_this.is_some() {
                        None
                    } else {
                        Some((Expr::Variable("this".to_string()), prompt_val.clone()))
                    };
                    let is_newline = func.name() == Some("newline");
                    let val = self.exec_function_with_receiver(&func, vec![], rec);
                    let text = match val {
                        Value::String(s) => s,
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => String::new(),
                    };
                    let cleaned = if text == "\n" || is_newline {
                        text
                    } else {
                        text.trim_end_matches('\n').to_string()
                    };
                    if !cleaned.is_empty() {
                        rendered.push(cleaned);
                    }
                }
                Value::String(s) => {
                    if s == "newline" || s == "\n" {
                        rendered.push("\n".to_string());
                    } else if s == "cwd" {
                        let cwd = crate::builtins::fmt_cwd::format_cwd_for_eval(self);
                        rendered.push(format!("\x1b[1;34m{cwd}\x1b[0m"));
                    } else if s == "char" || s == "symbol" {
                        let text = if let Value::Object(ref prompt_map) = prompt_val
                            && let Ok(guard) = prompt_map.read()
                            && let Some(Value::String(t)) = guard.get("text")
                        {
                            t.clone()
                        } else {
                            "\x1b[35m›\x1b[0m ".to_string()
                        };
                        let cleaned = text.trim_end_matches('\n').to_string();
                        if !cleaned.is_empty() {
                            rendered.push(cleaned);
                        }
                    } else if let Some((plugin_func, manifest)) =
                        self.plugin_manager.get_prompt_segment(&s)
                    {
                        let rec = Some((Expr::Variable("this".to_string()), manifest));
                        let val = self.exec_function_with_receiver(&plugin_func, vec![], rec);
                        let text = match val {
                            Value::String(t) => t,
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            _ => String::new(),
                        };
                        let cleaned = text.trim_end_matches('\n').to_string();
                        if !cleaned.is_empty() {
                            rendered.push(cleaned);
                        }
                    } else if let Value::Object(ref prompt_map) = prompt_val
                        && let Ok(guard) = prompt_map.read()
                        && let Some(Value::Object(seg_map)) = guard.get("segments")
                        && let Ok(seg_guard) = seg_map.read()
                        && let Some(seg_val) = seg_guard.get(&s)
                    {
                        match seg_val {
                            Value::Function(func) => {
                                let rec =
                                    Some((Expr::Variable("this".to_string()), prompt_val.clone()));
                                let val = self.exec_function_with_receiver(func, vec![], rec);
                                let text = match val {
                                    Value::String(t) => t,
                                    Value::Number(n) => n.to_string(),
                                    Value::Bool(b) => b.to_string(),
                                    _ => String::new(),
                                };
                                let cleaned = text.trim_end_matches('\n').to_string();
                                if !cleaned.is_empty() {
                                    rendered.push(cleaned);
                                }
                            }
                            Value::String(t) => {
                                let cleaned = t.trim_end_matches('\n').to_string();
                                if !cleaned.is_empty() {
                                    rendered.push(cleaned);
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Direct string!
                        let cleaned = s.trim_end_matches('\n').to_string();
                        if !cleaned.is_empty() {
                            rendered.push(cleaned);
                        }
                    }
                }
                Value::Number(n) => {
                    rendered.push(n.to_string());
                }
                Value::Bool(b) => {
                    rendered.push(b.to_string());
                }
                _ => {}
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
}
