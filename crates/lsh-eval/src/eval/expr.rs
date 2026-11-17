use std::{collections::HashMap, env, io, sync::Arc};

use lsh_syntax::ast::{BinaryOp, Expr, UnaryOp};

use crate::{
    eval::{
        EvalError, EvalFlow, Evaluator, cmds,
        value::{FunctionValue, Value},
    },
    expand::{self, ExpandCtx},
    io::{IoCtx, IoStreamMode},
};

pub(crate) type NamedReceiver = (String, Value);
pub(crate) type NamedFunctionWithReceiver = (Arc<FunctionValue>, Option<NamedReceiver>);

impl Evaluator {
    pub fn exec_function(&mut self, func: &FunctionValue, args: Vec<Value>) -> Value {
        self.exec_function_with_receiver(func, args, None)
    }

    pub fn exec_function_with_receiver(
        &mut self,
        func: &FunctionValue,
        args: Vec<Value>,
        receiver: Option<(Expr, Value)>,
    ) -> Value {
        self.last_error_span = None;
        let mut scope_indices = vec![0];
        match self.try_call_user_function_with_values_and_receiver(
            func,
            args,
            receiver,
            &mut scope_indices,
        ) {
            Ok(val) => val,
            Err(err) => {
                self.report_runtime_error(&err);
                Value::Null
            }
        }
    }

    pub(crate) fn try_call_user_function_with_receiver(
        &mut self,
        func: &FunctionValue,
        args: &[Expr],
        receiver: Option<(Expr, Value)>,
        scope_indices: &mut Vec<usize>,
    ) -> Result<Value, EvalError> {
        let mut evaluated_args = Vec::with_capacity(args.len());
        for arg in args {
            evaluated_args.push(self.try_evaluate_expr(arg, true, scope_indices)?);
        }

        self.try_call_user_function_with_values_and_receiver(
            func,
            evaluated_args,
            receiver,
            scope_indices,
        )
    }

    pub(crate) fn try_call_user_function_with_values_and_receiver(
        &mut self,
        func: &FunctionValue,
        evaluated_args: Vec<Value>,
        receiver: Option<(Expr, Value)>,
        scope_indices: &mut Vec<usize>,
    ) -> Result<Value, EvalError> {
        let effective_receiver = receiver.clone().or_else(|| {
            func.bound_this
                .as_ref()
                .map(|this_val| (Expr::Variable("this".to_string()), this_val.clone()))
        });

        if let crate::eval::value::FunctionKind::Native { ref func, .. } = func.kind {
            let this_val = effective_receiver.map(|(_, ref v)| v.clone());
            return Ok((func.clone())(self, evaluated_args, this_val));
        }

        let (outcome_res, final_this) = self.with_scope(
            scope_indices,
            |scope| {
                if let Some((_, ref this_val)) = effective_receiver {
                    scope.vars.insert("this".to_string(), this_val.clone());
                }

                for (param_index, param_name) in func.params.iter().enumerate() {
                    let value = evaluated_args
                        .get(param_index)
                        .cloned()
                        .unwrap_or(Value::Null);
                    scope.vars.insert(param_name.to_owned(), value);
                }
            },
            |eval, scope_indices, scope_index| {
                let outcome = eval.try_evaluate_statements(&func.body, scope_indices);
                let final_this = if receiver.is_some() {
                    eval.scopes
                        .get(scope_index)
                        .and_then(|s| s.vars.get("this").cloned())
                } else {
                    None
                };
                (outcome, final_this)
            },
        );

        let outcome = outcome_res?;

        if let (Some((receiver_expr, initial_this)), Some(updated_this)) = (receiver, final_this)
            && updated_this != initial_this
            && Self::is_assignable(&receiver_expr)
        {
            self.assign_target(&receiver_expr, updated_this, scope_indices)?;
        }

        Ok(match outcome.flow {
            EvalFlow::Continue | EvalFlow::LoopBreak | EvalFlow::LoopContinue => Value::Null,
            EvalFlow::Return(value) => value,
        })
    }

    pub(crate) fn try_evaluate_expr(
        &mut self,
        expr: &Expr,
        is_secondary: bool,
        scope_indices: &mut Vec<usize>,
    ) -> Result<Value, EvalError> {
        match expr {
            Expr::Call { target, args } => {
                let (target_val, receiver) = self.resolve_callable_target(target, scope_indices)?;

                if let Value::Function(func) = &target_val {
                    return self.try_call_user_function_with_receiver(
                        func,
                        args,
                        receiver,
                        scope_indices,
                    );
                }

                // For command execution, we only want to do this if we are not in a secondary expression (like an argument).
                if is_secondary {
                    return Ok(target_val);
                }

                let identifier = target_val.to_string();
                if let Some((func, plugin_manifest)) = self.plugin_manager.get_command(&identifier)
                {
                    let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                    return self.try_call_user_function_with_receiver(
                        &func,
                        args,
                        rec,
                        scope_indices,
                    );
                }

                let (res_cmd, res_args) = self.resolve_command(identifier);

                // Concatenate arguments from the input cmd with the provided arguments from the evaluator.
                let args = self.build_command_args(res_args, args, scope_indices)?;

                let mut io = IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();

                let code = cmds::exec_cmd(self, &res_cmd, args, &[], &mut io);
                self.last_status = code;

                Ok(Value::Null)
            }
            Expr::Word(word) => {
                if let Some((func, rec_info)) =
                    self.resolve_named_function_with_receiver(word, scope_indices)
                {
                    let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                    return self.try_call_user_function_with_receiver(
                        &func,
                        &[],
                        rec,
                        scope_indices,
                    );
                }

                if let Some((func, plugin_manifest)) = self.plugin_manager.get_command(word) {
                    let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                    return self.try_call_user_function_with_receiver(
                        &func,
                        &[],
                        rec,
                        scope_indices,
                    );
                }

                let (res_cmd, args) = self.resolve_command(word.to_owned());
                let args = expand::expand_args(args, ExpandCtx::default());

                if is_secondary {
                    if !matches!(word.as_str(), "?" | "$" | "!" | "#" | "*" | "@")
                        && let Some(val) = self.scoped_var(word, scope_indices)
                    {
                        return Ok(val);
                    }
                    if word.contains('.') {
                        let parts: Vec<&str> = word.split('.').collect();
                        if let Some(base_val) = self.scoped_var(parts[0], scope_indices) {
                            let mut cur = base_val;
                            let mut parent = cur.clone();
                            for part in &parts[1..] {
                                parent = cur.clone();
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
                                if let Value::Function(f) = cur {
                                    if f.bound_this.is_none() {
                                        return Ok(Value::function(f.with_bound_this(parent)));
                                    } else {
                                        return Ok(Value::Function(f));
                                    }
                                }
                                return Ok(cur);
                            }
                        }
                    }
                    if let Ok(env_var) = env::var(word) {
                        return Ok(Value::String(env_var));
                    }
                    return Ok(Value::String(word.to_owned()));
                }

                let mut io = IoCtx::<io::Stdin, io::Stdout, io::Stderr>::default();
                let code = cmds::exec_cmd(self, &res_cmd, args, &[], &mut io);
                self.last_status = code;

                Ok(Value::Null)
            }
            Expr::Variable(var_name) => {
                if var_name.contains('.') {
                    let expanded_target = Self::path_to_expr(var_name);
                    return self.try_evaluate_expr(&expanded_target, is_secondary, scope_indices);
                }

                if let Some(value) = self.scoped_var(var_name, scope_indices) {
                    return Ok(value);
                }

                // Try to find variable in environment.
                if let Ok(env_var) = env::var(var_name) {
                    // Environment variables are resolved into strings.
                    Ok(Value::String(env_var))
                } else {
                    Ok(Value::Null)
                }
            }
            Expr::Unary { op, expr } => {
                let val = self.try_evaluate_expr(expr, true, scope_indices)?;
                Ok(match op {
                    UnaryOp::Not => Value::Bool(!val.is_truthy()),
                    UnaryOp::Neg => match val {
                        Value::Number(n) => Value::Number(-n),
                        _ => Value::Null,
                    },
                })
            }
            Expr::Binary {
                op,
                left: left_box,
                right: right_box,
            } => {
                let left_res = self.try_evaluate_expr(left_box, true, scope_indices)?;
                let right_res = self.try_evaluate_expr(right_box, true, scope_indices)?;
                Ok(Self::eval_binary_op(op, left_res, right_res))
            }
            Expr::Index { object, index } => {
                let object_value = self.try_evaluate_expr(object, true, scope_indices)?;
                let index_value = self.try_evaluate_expr(index, true, scope_indices)?;

                if let Some(key) = Self::object_index_key(index_value) {
                    let (prop_val, parent_obj) = if key.contains('.') {
                        let parts: Vec<&str> = key.split('.').collect();
                        let mut cur_obj = object_value;
                        for part in &parts[..parts.len().saturating_sub(1)] {
                            cur_obj = Self::get_child_member(&cur_obj, part);
                            if matches!(cur_obj, Value::Null) {
                                break;
                            }
                        }
                        let last = parts.last().copied().unwrap_or_default();
                        let prop = Self::get_child_member(&cur_obj, last);
                        (prop, cur_obj)
                    } else {
                        let prop = Self::get_child_member(&object_value, &key);
                        (prop, object_value)
                    };

                    if let Value::Function(f) = prop_val {
                        if f.bound_this.is_none() {
                            Ok(Value::function(f.with_bound_this(parent_obj)))
                        } else {
                            Ok(Value::Function(f))
                        }
                    } else {
                        Ok(prop_val)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expr::TypeOf(expr) => {
                // Evaluate the expression and return its type as a string.
                let evaluated_value = self.try_evaluate_expr(expr, true, scope_indices)?;
                Ok(Value::String(evaluated_value.get_type().to_string()))
            }
            Expr::String(string) | Expr::QuotedString(string) => {
                Ok(Value::String(string.to_owned()))
            }
            Expr::InterpolatedString(parts) => {
                let mut buf = String::new();
                for part in parts {
                    let val = self.try_evaluate_expr(part, true, scope_indices)?;
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
                Ok(Value::String(buf))
            }
            Expr::Number(num) => Ok(Value::Number(*num)),
            Expr::Bool(bool) => Ok(Value::Bool(*bool)),
            Expr::CommandSub(expr) => {
                match expr.as_ref() {
                    Expr::Call { target, args } => {
                        let (target_val, receiver) =
                            self.resolve_callable_target(target, scope_indices)?;

                        if let Value::Function(func) = &target_val {
                            let value = self.try_call_user_function_with_receiver(
                                func,
                                args,
                                receiver,
                                scope_indices,
                            )?;
                            return Ok(Self::make_command_result(
                                0,
                                value.to_string(),
                                String::new(),
                            ));
                        }

                        let identifier = target_val.to_string();

                        if let Some((func, rec_info)) =
                            self.resolve_named_function_with_receiver(&identifier, scope_indices)
                        {
                            let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                            let value = self.try_call_user_function_with_receiver(
                                &func,
                                args,
                                rec,
                                scope_indices,
                            )?;
                            return Ok(Self::make_command_result(
                                0,
                                value.to_string(),
                                String::new(),
                            ));
                        }

                        if let Some((func, plugin_manifest)) =
                            self.plugin_manager.get_command(&identifier)
                        {
                            let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                            let value = self.try_call_user_function_with_receiver(
                                &func,
                                args,
                                rec,
                                scope_indices,
                            )?;
                            return Ok(Self::make_command_result(
                                0,
                                value.to_string(),
                                String::new(),
                            ));
                        }

                        let (res_cmd, res_args) = self.resolve_command(identifier);
                        let all_args = self.build_command_args(res_args, args, scope_indices)?;

                        let mut io = IoCtx {
                            input: io::empty(),
                            out: Vec::<u8>::new(),
                            err: Vec::<u8>::new(),
                            input_mode: IoStreamMode::Pipe,
                            out_mode: IoStreamMode::Pipe,
                            err_mode: IoStreamMode::Pipe,
                        };

                        let exit_status = cmds::exec_cmd(self, &res_cmd, all_args, &[], &mut io);

                        Ok(Self::make_command_result(
                            exit_status,
                            String::from_utf8(io.out).unwrap_or_default(),
                            String::from_utf8(io.err).unwrap_or_default(),
                        ))
                    }
                    Expr::Word(word) => {
                        if let Some((func, rec_info)) =
                            self.resolve_named_function_with_receiver(word, scope_indices)
                        {
                            let rec = rec_info.map(|(path, val)| (Self::path_to_expr(&path), val));
                            let value = self.try_call_user_function_with_receiver(
                                &func,
                                &[],
                                rec,
                                scope_indices,
                            )?;
                            return Ok(Self::make_command_result(
                                0,
                                value.to_string(),
                                String::new(),
                            ));
                        }

                        if let Some((func, plugin_manifest)) = self.plugin_manager.get_command(word)
                        {
                            let rec = Some((Expr::Variable("this".to_string()), plugin_manifest));
                            let value = self.try_call_user_function_with_receiver(
                                &func,
                                &[],
                                rec,
                                scope_indices,
                            )?;
                            return Ok(Self::make_command_result(
                                0,
                                value.to_string(),
                                String::new(),
                            ));
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

                        Ok(Self::make_command_result(
                            exit_status,
                            String::from_utf8(io.out).unwrap_or_default(),
                            String::from_utf8(io.err).unwrap_or_default(),
                        ))
                    }
                    Expr::Bool(b) => {
                        let word = if *b { "true" } else { "false" };
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

                        Ok(Self::make_command_result(
                            exit_status,
                            String::from_utf8(io.out).unwrap_or_default(),
                            String::from_utf8(io.err).unwrap_or_default(),
                        ))
                    }
                    other => {
                        // Fallback: allow non-command expressions inside $(...)
                        let value = self.try_evaluate_expr(other, true, scope_indices)?;
                        Ok(Self::make_command_result(
                            0,
                            value.to_string(),
                            String::new(),
                        ))
                    }
                }
            }
            Expr::CommandSubCommand(command_expr) => {
                let (code, out, err) = self.evaluate_command_expr_with_input(
                    command_expr,
                    vec![],
                    scope_indices,
                    true,
                )?;
                Ok(Self::make_command_result(
                    code,
                    String::from_utf8(out).unwrap_or_default(),
                    String::from_utf8(err).unwrap_or_default(),
                ))
            }
            Expr::AnonymousFunction { params, body } => Ok(Value::function(FunctionValue::ast(
                params.clone(),
                body.clone(),
            ))),
            Expr::Object(expr_map) => {
                let mut obj = HashMap::new();
                for (key, value) in expr_map {
                    let value_value = self.try_evaluate_expr(value, true, scope_indices)?;
                    obj.insert(key.to_string(), value_value);
                }
                Ok(Value::object(obj))
            }
            Expr::Array(elements) => {
                let mut evaluated = Vec::with_capacity(elements.len());
                for elem in elements {
                    evaluated.push(self.try_evaluate_expr(elem, true, scope_indices)?);
                }
                Ok(Value::array(evaluated))
            }
        }
    }

    pub(crate) fn eval_binary_op(op: &BinaryOp, left_res: Value, right_res: Value) -> Value {
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
                (Value::String(s), Value::Number(n)) | (Value::Number(n), Value::String(s)) => {
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
}
