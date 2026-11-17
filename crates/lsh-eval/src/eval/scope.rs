use std::{
    collections::{HashMap, HashSet},
    env,
};

use lsh_syntax::ast::{BinaryOp, Expr};

use crate::eval::{EvalError, Evaluator, value::Value};

#[derive(Default)]
pub struct Scope {
    pub(crate) vars: HashMap<String, Value>,
}

impl Evaluator {
    pub fn is_exported(&self, name: &str) -> bool {
        self.exported_vars.contains(name)
    }

    pub fn export_var(&mut self, name: &str, value: &str) {
        self.exported_vars.insert(name.to_string());
        self.completions_dirty = true;
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
            unsafe {
                env::set_var(name, value.to_string());
            }
        }
    }

    pub fn push_var(&mut self, scope_index: usize, name: String, value: Value) {
        if let Some(scope) = self.scopes.get_mut(scope_index) {
            scope.vars.insert(name, value);
            if scope_index == 0 {
                self.completions_dirty = true;
            }
        }
    }

    pub fn resolve_command(&self, cmd: String) -> (String, Vec<String>) {
        if let Some(resolved_alias) = self.aliases.get(cmd.as_str()) {
            // Split the unsplit command by spaces, take out the command itself, and return the rest of the
            // parts as arguments.
            let mut split_cmd = resolved_alias.split(' ');
            let resolved_cmd = split_cmd.next().unwrap_or(cmd.as_str()).to_string();
            let args: Vec<String> = split_cmd.map(|arg| arg.to_string()).collect();

            (resolved_cmd, args)
        } else {
            (cmd, vec![])
        }
    }

    pub fn set_var(&mut self, scope: usize, name: &str, value: Value) -> Result<(), EvalError> {
        let s = self.scopes.get_mut(scope).ok_or(EvalError::ScopeNotFound)?;
        s.vars.insert(name.to_string(), value.clone());
        if scope == 0 {
            self.completions_dirty = true;
        }
        self.sync_env_var_if_exported(name, &value);

        Ok(())
    }

    pub fn get_var(&self, scope: usize, name: &str) -> Option<&Value> {
        let scope = self.scopes.get(scope)?;
        scope.vars.get(name)
    }

    pub fn set_alias(&mut self, name: String, value: String) -> Option<String> {
        self.completions_dirty = true;
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
        vars.insert("?".to_string());
        vars.insert("$".to_string());
        vars.insert("!".to_string());
        vars.insert("#".to_string());
        vars.into_iter().collect()
    }

    pub(crate) fn make_scope(&mut self) -> usize {
        self.scopes.push(Scope::default());

        self.scopes.len() - 1
    }

    pub(crate) fn with_scope<R>(
        &mut self,
        scope_indices: &mut Vec<usize>,
        init: impl FnOnce(&mut Scope),
        body: impl FnOnce(&mut Self, &mut Vec<usize>, usize) -> R,
    ) -> R {
        let scope_index = self.make_scope();
        if let Some(scope) = self.scopes.get_mut(scope_index) {
            init(scope);
        }
        scope_indices.push(scope_index);

        let result = body(self, scope_indices, scope_index);

        if scope_indices.last().copied() == Some(scope_index) {
            scope_indices.pop();
        }
        if self.scopes.len() == scope_index + 1 {
            self.scopes.pop();
        }

        result
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
            if let Some(scope) = self.scopes.get(*scope_index)
                && let Some(value) = scope.vars.get(var_name)
            {
                return Some(value.to_owned());
            }
        }

        match var_name {
            "?" => Some(Value::Number(self.last_status as f64)),
            "$" => Some(Value::Number(std::process::id() as f64)),
            "!" => {
                let last_pid = self
                    .job_manager
                    .jobs
                    .values()
                    .last()
                    .and_then(|j| j.pids.last().copied());
                Some(Value::Number(last_pid.unwrap_or(0) as f64))
            }
            "#" => Some(Value::Number(0.0)),
            "*" | "@" => Some(Value::String(String::new())),
            _ => None,
        }
    }

    pub(crate) fn assign_var(
        &mut self,
        var_name: &str,
        value: Value,
        scope_indices: &[usize],
    ) -> Result<(), EvalError> {
        for scope_index in scope_indices.iter().rev() {
            if let Some(scope) = self.scopes.get_mut(*scope_index)
                && let Some(existing) = scope.vars.get_mut(var_name)
            {
                *existing = value.clone();
                self.sync_env_var_if_exported(var_name, &value);
                return Ok(());
            }
        }

        if let Some(scope_index) = scope_indices.last().copied()
            && let Some(scope) = self.scopes.get_mut(scope_index)
        {
            if scope
                .vars
                .insert(var_name.to_owned(), value.clone())
                .is_none()
            {
                self.completions_dirty = true;
            }
            self.sync_env_var_if_exported(var_name, &value);
            Ok(())
        } else {
            Err(EvalError::ScopeNotFound)
        }
    }

    pub(crate) fn is_assignable(expr: &Expr) -> bool {
        match expr {
            Expr::Variable(_) => true,
            Expr::Index { object, .. } => Self::is_assignable(object),
            _ => false,
        }
    }

    fn expand_dotted_index_target(object: &Expr, key: &str) -> Expr {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current_obj = object.clone();
        for part in &parts[..parts.len().saturating_sub(1)] {
            current_obj = Expr::Index {
                object: Box::new(current_obj),
                index: Box::new(Expr::String(part.to_string())),
            };
        }
        let last_key = parts.last().copied().unwrap_or_default().to_string();
        Expr::Index {
            object: Box::new(current_obj),
            index: Box::new(Expr::String(last_key)),
        }
    }

    fn resolve_array_assign_index(len: usize, key: &str) -> Result<usize, EvalError> {
        let idx = key
            .parse::<isize>()
            .map_err(|_| EvalError::InvalidIndex(format!("Invalid array index '{key}'.")))?;
        let actual_idx = if idx < 0 { len as isize + idx } else { idx };
        if actual_idx < 0 || (actual_idx as usize) > len {
            return Err(EvalError::InvalidIndex(format!(
                "Array index {idx} out of bounds for length {len}."
            )));
        }
        Ok(actual_idx as usize)
    }

    pub(crate) fn assign_target(
        &mut self,
        target: &Expr,
        value: Value,
        scope_indices: &mut Vec<usize>,
    ) -> Result<(), EvalError> {
        match target {
            Expr::Variable(var_name) => {
                if var_name.contains('.') {
                    let expanded_target = Self::path_to_expr(var_name);
                    return self.assign_target(&expanded_target, value, scope_indices);
                }
                self.assign_var(var_name, value, scope_indices)
            }
            Expr::Index { object, index } => {
                let key_value = self.try_evaluate_expr(index, true, scope_indices)?;
                let key = Self::object_index_key(key_value).ok_or_else(|| {
                    EvalError::InvalidIndex("Invalid object index for assignment.".to_string())
                })?;

                if key.contains('.') {
                    let expanded_target = Self::expand_dotted_index_target(object, &key);
                    return self.assign_target(&expanded_target, value, scope_indices);
                }

                let target_obj = self.try_evaluate_expr(object, true, scope_indices)?;
                match target_obj {
                    Value::Object(map) => {
                        let mut m = match map.write() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        m.insert(key, value);
                        Ok(())
                    }
                    Value::Array(arr) => {
                        let mut a = match arr.write() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        let u_idx = Self::resolve_array_assign_index(a.len(), &key)?;
                        if u_idx < a.len() {
                            a[u_idx] = value;
                        } else if u_idx == a.len() {
                            a.push(value);
                        }
                        Ok(())
                    }
                    Value::Null => {
                        let mut new_map = HashMap::new();
                        new_map.insert(key, value);
                        self.assign_target(object, Value::object(new_map), scope_indices)
                    }
                    other => Err(EvalError::TypeError(format!(
                        "Cannot assign member '{key}' on value of type '{}'.",
                        other.get_type()
                    ))),
                }
            }
            _ => Err(EvalError::InvalidAssignmentTarget(
                "Invalid assignment target.".to_string(),
            )),
        }
    }

    pub(crate) fn compound_assign_target(
        &mut self,
        target: &Expr,
        op: &BinaryOp,
        rhs_expr: &Expr,
        scope_indices: &mut Vec<usize>,
    ) -> Result<(), EvalError> {
        match target {
            Expr::Variable(var_name) => {
                if var_name.contains('.') {
                    let expanded_target = Self::path_to_expr(var_name);
                    return self.compound_assign_target(
                        &expanded_target,
                        op,
                        rhs_expr,
                        scope_indices,
                    );
                }
                let current = self
                    .scoped_var(var_name, scope_indices)
                    .or_else(|| env::var(var_name).ok().map(Value::String))
                    .unwrap_or(Value::Null);
                let rhs = self.try_evaluate_expr(rhs_expr, true, scope_indices)?;
                let updated = Self::eval_binary_op(op, current, rhs);
                self.assign_var(var_name, updated, scope_indices)
            }
            Expr::Index { object, index } => {
                let key_value = self.try_evaluate_expr(index, true, scope_indices)?;
                let key = Self::object_index_key(key_value).ok_or_else(|| {
                    EvalError::InvalidIndex("Invalid object index for assignment.".to_string())
                })?;

                if key.contains('.') {
                    let expanded_target = Self::expand_dotted_index_target(object, &key);
                    return self.compound_assign_target(
                        &expanded_target,
                        op,
                        rhs_expr,
                        scope_indices,
                    );
                }

                let target_obj = self.try_evaluate_expr(object, true, scope_indices)?;
                match target_obj {
                    Value::Object(map) => {
                        let current = {
                            let guard = match map.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            guard.get(&key).cloned().unwrap_or(Value::Null)
                        };
                        let rhs = self.try_evaluate_expr(rhs_expr, true, scope_indices)?;
                        let updated = Self::eval_binary_op(op, current, rhs);
                        let mut m = match map.write() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        m.insert(key, updated);
                        Ok(())
                    }
                    Value::Array(arr) => {
                        let (u_idx, current) = {
                            let guard = match arr.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            let u_idx = Self::resolve_array_assign_index(guard.len(), &key)?;
                            let current = guard.get(u_idx).cloned().unwrap_or(Value::Null);
                            (u_idx, current)
                        };
                        let rhs = self.try_evaluate_expr(rhs_expr, true, scope_indices)?;
                        let updated = Self::eval_binary_op(op, current, rhs);
                        let mut a = match arr.write() {
                            Ok(g) => g,
                            Err(p) => p.into_inner(),
                        };
                        if u_idx < a.len() {
                            a[u_idx] = updated;
                        } else if u_idx == a.len() {
                            a.push(updated);
                        }
                        Ok(())
                    }
                    Value::Null => {
                        let rhs = self.try_evaluate_expr(rhs_expr, true, scope_indices)?;
                        let updated = Self::eval_binary_op(op, Value::Null, rhs);
                        let mut new_map = HashMap::new();
                        new_map.insert(key, updated);
                        self.assign_target(object, Value::object(new_map), scope_indices)
                    }
                    other => Err(EvalError::TypeError(format!(
                        "Cannot assign member '{key}' on value of type '{}'.",
                        other.get_type()
                    ))),
                }
            }
            _ => Err(EvalError::InvalidAssignmentTarget(
                "Invalid assignment target.".to_string(),
            )),
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
