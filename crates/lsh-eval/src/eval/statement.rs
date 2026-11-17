use std::{
    io::{self, Write},
    sync::atomic::Ordering,
};

use lsh_syntax::ast::{Program, Statement};
use lsh_sys::signals::STATUS_SIGINT;

use crate::eval::{
    EvalError, Evaluator,
    value::{FunctionValue, Value},
};

#[derive(Debug, PartialEq)]
pub enum EvalFlow {
    Continue,
    LoopBreak,
    LoopContinue,
    Return(Value),
}

#[derive(Debug, PartialEq)]
pub struct EvalOutcome {
    pub flow: EvalFlow,
    pub last_status: i32,
}

impl Evaluator {
    pub fn try_evaluate_program(&mut self, prog: Program) -> Result<EvalOutcome, EvalError> {
        self.try_evaluate_statements(&prog.body, &mut vec![0])
    }

    pub fn evaluate_program(&mut self, prog: Program) -> EvalOutcome {
        match self.try_evaluate_program(prog) {
            Ok(outcome) => outcome,
            Err(err) => {
                self.report_runtime_error(&err);
                EvalOutcome {
                    flow: EvalFlow::Continue,
                    last_status: 1,
                }
            }
        }
    }

    pub fn evaluate(&mut self, prog: Program) -> i32 {
        self.evaluate_program(prog).last_status
    }

    pub fn evaluate_statements(
        &mut self,
        statements: &[Statement],
        scope_indices: &mut Vec<usize>,
    ) -> EvalOutcome {
        match self.try_evaluate_statements(statements, scope_indices) {
            Ok(outcome) => outcome,
            Err(err) => {
                self.report_runtime_error(&err);
                EvalOutcome {
                    flow: EvalFlow::Continue,
                    last_status: 1,
                }
            }
        }
    }

    fn evaluate_in_new_scope(
        &mut self,
        statements: &[Statement],
        scope_indices: &mut Vec<usize>,
    ) -> Result<EvalOutcome, EvalError> {
        self.with_scope(
            scope_indices,
            |_| {},
            |eval, scope_indices, _| eval.try_evaluate_statements(statements, scope_indices),
        )
    }

    fn evaluate_in_new_scope_with_var(
        &mut self,
        statements: &[Statement],
        var_name: &str,
        var_val: Value,
        scope_indices: &mut Vec<usize>,
    ) -> Result<EvalOutcome, EvalError> {
        self.with_scope(
            scope_indices,
            |scope| {
                scope.vars.insert(var_name.to_string(), var_val);
            },
            |eval, scope_indices, _| eval.try_evaluate_statements(statements, scope_indices),
        )
    }

    pub fn try_evaluate_statements(
        &mut self,
        statements: &[Statement],
        scope_indices: &mut Vec<usize>,
    ) -> Result<EvalOutcome, EvalError> {
        let mut last_status = 0;

        for statement in statements {
            if self.interrupted.swap(false, Ordering::Relaxed) {
                self.last_status = STATUS_SIGINT;
                return Ok(EvalOutcome {
                    flow: EvalFlow::Return(Value::Null),
                    last_status: STATUS_SIGINT,
                });
            }

            match statement {
                Statement::Expr(expr) => {
                    self.try_evaluate_expr(expr, false, scope_indices)?;
                    last_status = self.last_status;
                }
                Statement::Command(cmd_expr) => {
                    let (code, out, err) = self.evaluate_command_expr_with_input(
                        cmd_expr,
                        vec![],
                        scope_indices,
                        false,
                    )?;
                    last_status = code;
                    self.last_status = code;
                    if !out.is_empty() {
                        let _ = io::stdout().write_all(&out);
                    }
                    if !err.is_empty() {
                        let _ = io::stderr().write_all(&err);
                    }
                    if code == STATUS_SIGINT || self.interrupted.swap(false, Ordering::Relaxed) {
                        return Ok(EvalOutcome {
                            flow: EvalFlow::Return(Value::Null),
                            last_status: STATUS_SIGINT,
                        });
                    }
                }
                Statement::Let { name, value } => {
                    let expr = self.try_evaluate_expr(value, true, scope_indices)?;
                    last_status = 0;

                    if let Some(last_scope_index) = scope_indices.last()
                        && let Some(scope) = self.scopes.get_mut(*last_scope_index)
                    {
                        if scope.vars.insert(name.to_owned(), expr.clone()).is_none() {
                            self.completions_dirty = true;
                        }
                        self.sync_env_var_if_exported(name, &expr);
                    } else {
                        return Err(EvalError::ScopeNotFound);
                    }
                }
                Statement::Assign { target, value } => {
                    let assigned_value = self.try_evaluate_expr(value, true, scope_indices)?;
                    self.assign_target(target, assigned_value, scope_indices)?;
                    last_status = 0;
                }
                Statement::CompoundAssign { target, op, value } => {
                    self.compound_assign_target(target, op, value, scope_indices)?;
                    last_status = 0;
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let cond_res = self.try_evaluate_expr(condition, true, scope_indices)?;

                    if cond_res.is_truthy() {
                        let outcome = self.evaluate_in_new_scope(then_branch, scope_indices)?;
                        last_status = outcome.last_status;
                        if outcome.flow != EvalFlow::Continue {
                            return Ok(outcome);
                        }
                    } else if let Some(else_branch_statements) = else_branch {
                        let outcome =
                            self.evaluate_in_new_scope(else_branch_statements, scope_indices)?;
                        last_status = outcome.last_status;
                        if outcome.flow != EvalFlow::Continue {
                            return Ok(outcome);
                        }
                    }
                }
                Statement::While { condition, body } => {
                    while self
                        .try_evaluate_expr(condition, true, scope_indices)?
                        .is_truthy()
                    {
                        if self.interrupted.swap(false, Ordering::Relaxed) {
                            self.last_status = STATUS_SIGINT;
                            last_status = STATUS_SIGINT;
                            break;
                        }

                        let outcome = self.evaluate_in_new_scope(body, scope_indices)?;
                        last_status = outcome.last_status;
                        if outcome.last_status == STATUS_SIGINT
                            || self.interrupted.swap(false, Ordering::Relaxed)
                        {
                            self.last_status = STATUS_SIGINT;
                            last_status = STATUS_SIGINT;
                            break;
                        }
                        if outcome.flow == EvalFlow::LoopBreak {
                            break;
                        }
                        if outcome.flow == EvalFlow::LoopContinue {
                            continue;
                        }
                        if let EvalFlow::Return(value) = outcome.flow {
                            return Ok(EvalOutcome {
                                flow: EvalFlow::Return(value),
                                last_status,
                            });
                        }
                    }
                }
                Statement::For {
                    variable,
                    iterable,
                    body,
                } => {
                    let iter_val = self.try_evaluate_expr(iterable, true, scope_indices)?;
                    let items: Vec<Value> = match iter_val {
                        Value::Array(arr) => {
                            let guard = match arr.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            guard.clone()
                        }
                        Value::Object(ref map) => {
                            let guard = match map.read() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            if guard.contains_key("code")
                                && guard.contains_key("out")
                                && guard.contains_key("err")
                            {
                                let out = match guard.get("out") {
                                    Some(Value::String(s)) => s.clone(),
                                    _ => String::new(),
                                };
                                let trimmed = out.trim_end_matches(['\r', '\n']);
                                if trimmed.is_empty() {
                                    Vec::new()
                                } else {
                                    trimmed
                                        .split('\n')
                                        .map(|l| {
                                            Value::String(
                                                l.strip_suffix('\r').unwrap_or(l).to_string(),
                                            )
                                        })
                                        .collect()
                                }
                            } else {
                                let mut keys: Vec<String> = guard.keys().cloned().collect();
                                keys.sort();
                                keys.into_iter().map(Value::String).collect()
                            }
                        }
                        Value::String(s) => {
                            s.chars().map(|c| Value::String(c.to_string())).collect()
                        }
                        Value::Null => Vec::new(),
                        other => vec![other],
                    };

                    for item in items {
                        if self.interrupted.swap(false, Ordering::Relaxed) {
                            self.last_status = STATUS_SIGINT;
                            last_status = STATUS_SIGINT;
                            break;
                        }

                        let outcome = self.evaluate_in_new_scope_with_var(
                            body,
                            variable,
                            item,
                            scope_indices,
                        )?;
                        last_status = outcome.last_status;
                        if outcome.last_status == STATUS_SIGINT
                            || self.interrupted.swap(false, Ordering::Relaxed)
                        {
                            self.last_status = STATUS_SIGINT;
                            last_status = STATUS_SIGINT;
                            break;
                        }
                        if outcome.flow == EvalFlow::LoopBreak {
                            break;
                        }
                        if outcome.flow == EvalFlow::LoopContinue {
                            continue;
                        }
                        if let EvalFlow::Return(value) = outcome.flow {
                            return Ok(EvalOutcome {
                                flow: EvalFlow::Return(value),
                                last_status,
                            });
                        }
                    }
                }
                Statement::Break => {
                    return Ok(EvalOutcome {
                        flow: EvalFlow::LoopBreak,
                        last_status,
                    });
                }
                Statement::Continue => {
                    return Ok(EvalOutcome {
                        flow: EvalFlow::LoopContinue,
                        last_status,
                    });
                }
                Statement::FunctionDeclaration {
                    target,
                    params,
                    body,
                } => {
                    let func_value =
                        Value::function(FunctionValue::ast(params.clone(), body.clone()));
                    self.assign_target(target, func_value, scope_indices)?;
                    last_status = 0;
                }
                Statement::Return(maybe_expr) => {
                    let result = if let Some(expr) = maybe_expr {
                        self.try_evaluate_expr(expr, true, scope_indices)?
                    } else {
                        Value::Null
                    };

                    return Ok(EvalOutcome {
                        flow: EvalFlow::Return(result),
                        last_status,
                    });
                }
                Statement::Block(block) => {
                    let outcome = self.evaluate_in_new_scope(block, scope_indices)?;
                    last_status = outcome.last_status;
                    if outcome.flow != EvalFlow::Continue {
                        return Ok(outcome);
                    }
                }
            }
        }

        Ok(EvalOutcome {
            flow: EvalFlow::Continue,
            last_status,
        })
    }
}
