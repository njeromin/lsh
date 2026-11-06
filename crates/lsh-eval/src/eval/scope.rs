use std::{
    collections::{HashMap, HashSet},
    env,
};

use lsh_syntax::ast::Expr;

use crate::eval::{EvalError, Evaluator, value::Value};

#[derive(Default)]
pub struct Scope {
    pub(crate) vars: HashMap<String, Value>,
}

impl Evaluator {
    pub fn is_exported(&self, name: &str) -> bool {
        name == "PATH" || self.exported_vars.contains(name) || env::var(name).is_ok()
    }

    pub fn export_var(&mut self, name: &str, value: &str) {
        self.exported_vars.insert(name.to_string());
        unsafe {
            env::set_var(name, value);
        }
        if let Some(scope) = self.scopes.first_mut() {
            scope
                .vars
                .insert(name.to_string(), Value::String(value.to_string()));
        }
    }

    pub fn sync_env_var_if_exported(&mut self, name: &str, value: &Value) {
        if self.is_exported(name) {
            self.exported_vars.insert(name.to_string());
            unsafe {
                env::set_var(name, value.to_string());
            }
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
            let mut split_cmd = resolved_alias.split(' ');
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
        let s = self.scopes.get_mut(scope).ok_or(EvalError::ScopeNotFound)?;
        s.vars.insert(name.to_string(), value.clone());
        self.sync_env_var_if_exported(name, &value);

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
        let mut vars = HashSet::new();
        for scope in &self.scopes {
            vars.extend(scope.vars.keys().cloned());
        }
        vars.extend(self.exported_vars.iter().cloned());
        vars.into_iter().collect()
    }

    pub(crate) fn make_scope(&mut self) -> usize {
        self.scopes.push(Scope::default());

        self.scopes.len() - 1
    }

    pub(crate) fn object_index_key(value: Value) -> Option<String> {
        match value {
            Value::String(s) => Some(s),
            Value::Number(n) => {
                if n.fract() == 0.0 {
                    Some((n as i64).to_string())
                } else {
                    Some(n.to_string())
                }
            }
            _ => None,
        }
    }

    pub(crate) fn scoped_var(&self, var_name: &str, scope_indices: &[usize]) -> Option<Value> {
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

    pub(crate) fn assign_var(&mut self, var_name: &str, value: Value, scope_indices: &[usize]) {
        for scope_index in scope_indices.iter().rev() {
            if let Some(scope) = self.scopes.get_mut(*scope_index) {
                if scope.vars.contains_key(var_name) {
                    scope.vars.insert(var_name.to_owned(), value.clone());
                    self.sync_env_var_if_exported(var_name, &value);
                    return;
                }
            } else {
                panic!("Failed to retrieve scope {scope_index}.");
            }
        }

        if let Some(scope_index) = scope_indices.last().copied()
            && let Some(scope) = self.scopes.get_mut(scope_index)
        {
            scope.vars.insert(var_name.to_owned(), value.clone());
            self.sync_env_var_if_exported(var_name, &value);
        } else {
            panic!("Failed to get current scope for assignment.");
        }
    }

    pub(crate) fn is_assignable(expr: &Expr) -> bool {
        matches!(expr, Expr::Variable(_) | Expr::Word(_) | Expr::Index { .. })
    }

    pub(crate) fn assign_target(
        &mut self,
        target: &Expr,
        value: Value,
        scope_indices: &mut Vec<usize>,
    ) {
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

    pub(crate) fn path_to_expr(path: &str) -> Expr {
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

    pub(crate) fn resolve_named_value(
        &self,
        identifier: &str,
        scope_indices: &[usize],
    ) -> Option<Value> {
        let mut parts = identifier.split('.');
        let first_part = parts.next()?;
        let mut value = self.scoped_var(first_part, scope_indices)?.clone();

        for part in parts {
            let next_value = Self::get_child_member(&value, part);
            if matches!(next_value, Value::Null) {
                return None;
            }
            value = next_value;
        }

        Some(value)
    }
}
