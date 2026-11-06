use std::{
    io::{self, Write},
    sync::atomic::Ordering,
};

use lsh_syntax::ast::{Program, Statement};
use lsh_sys::signals::STATUS_SIGINT;

use crate::eval::{
    Evaluator,
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

    fn evaluate_in_new_scope_with_var(
        &mut self,
        statements: &[Statement],
        var_name: &str,
        var_val: Value,
        scope_indices: &mut Vec<usize>,
    ) -> EvalOutcome {
        let scope_index = self.make_scope();
        if let Some(scope) = self.scopes.get_mut(scope_index) {
            scope.vars.insert(var_name.to_string(), var_val);
        }
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

    pub fn evaluate_statements(
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
                        let _ = io::stdout().write_all(&out);
                    }
                    if !err.is_empty() {
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
                        scope.vars.insert(name.to_owned(), expr.clone());
                        self.sync_env_var_if_exported(name, &expr);
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
                        if outcome.flow != EvalFlow::Continue {
                            return outcome;
                        }
                    } else if let Some(else_branch_statements) = else_branch {
                        let outcome =
                            self.evaluate_in_new_scope(else_branch_statements, scope_indices);
                        last_status = outcome.last_status;
                        if outcome.flow != EvalFlow::Continue {
                            return outcome;
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
                        if outcome.flow == EvalFlow::LoopBreak {
                            break;
                        }
                        if outcome.flow == EvalFlow::LoopContinue {
                            continue;
                        }
                        if let EvalFlow::Return(value) = outcome.flow {
                            return EvalOutcome {
                                flow: EvalFlow::Return(value),
                                last_status,
                            };
                        }
                    }
                }
                Statement::For {
                    variable,
                    iterable,
                    body,
                } => {
                    let iter_val = self.evaluate_expr(iterable, true, scope_indices);
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
                        );
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
                            return EvalOutcome {
                                flow: EvalFlow::Return(value),
                                last_status,
                            };
                        }
                    }
                }
                Statement::Break => {
                    return EvalOutcome {
                        flow: EvalFlow::LoopBreak,
                        last_status,
                    };
                }
                Statement::Continue => {
                    return EvalOutcome {
                        flow: EvalFlow::LoopContinue,
                        last_status,
                    };
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
                    if outcome.flow != EvalFlow::Continue {
                        return outcome;
                    }
                }
            }
        }

        EvalOutcome {
            flow: EvalFlow::Continue,
            last_status,
        }
    }
}
