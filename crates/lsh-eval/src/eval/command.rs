use std::{collections::HashMap, fs::File, io};

use lsh_syntax::{
    ast::{BinaryOp, CommandExpr, Expr},
    print_error,
};

#[cfg(unix)]
use lsh_sys::signals::{
    ProcessStatus, SIGINT, TerminalForegroundGuard, get_terminal_attributes, is_interactive_tty,
    reset_child_signals_pre_exec, restore_shell_termios, signal_exit_code, wait_pid_status,
};

use crate::{
    eval::{Evaluator, cmds, value::Value},
    expand::{self, ExpandCtx},
    io::{IoCtx, IoStreamMode},
    jobs::JobState,
};

struct ExternalPipelineStage {
    cmd: String,
    args: Vec<String>,
    env_assignments: Vec<(String, String)>,
    stdout_redirect: Option<String>,
}

impl Evaluator {
    pub(crate) fn make_command_result(code: i32, out: String, err: String) -> Value {
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

        let pipeline_cmd_summary = stages
            .iter()
            .map(|s| {
                if s.args.is_empty() {
                    s.cmd.clone()
                } else {
                    format!("{} {}", s.cmd, s.args.join(" "))
                }
            })
            .collect::<Vec<_>>()
            .join(" | ");

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
                unsafe {
                    c.pre_exec(reset_child_signals_pre_exec);
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

        #[cfg(unix)]
        if !capture_output && is_interactive_tty() {
            let pids: Vec<i32> = children.iter().map(|c| c.id() as i32).collect();
            let mut remaining_pids = pids.clone();
            let mut last_code = 0;

            loop {
                if remaining_pids.is_empty() {
                    break;
                }

                match wait_pid_status(-leader_pid, libc::WUNTRACED) {
                    Ok((_pid, ProcessStatus::Stopped(sig))) => {
                        let saved_termios = get_terminal_attributes();
                        restore_shell_termios();
                        drop(_guard);

                        let job_id = self.job_manager.add_job(
                            leader_pid,
                            pids,
                            pipeline_cmd_summary.clone(),
                            JobState::Stopped(sig),
                            saved_termios,
                        );
                        use std::io::Write as _;
                        let _ = writeln!(
                            io::stderr(),
                            "\n[{job_id}]+  Stopped                 {pipeline_cmd_summary}"
                        );
                        return (signal_exit_code(sig), vec![], vec![]);
                    }
                    Ok((pid, ProcessStatus::Exited(code))) => {
                        if Some(pid) == pids.last().copied() {
                            last_code = code;
                        }
                        remaining_pids.retain(|&p| p != pid);
                    }
                    Ok((pid, ProcessStatus::Signaled(sig))) => {
                        if sig == SIGINT {
                            use std::io::Write as _;
                            let _ = writeln!(io::stderr());
                        }
                        if Some(pid) == pids.last().copied() {
                            last_code = signal_exit_code(sig);
                        }
                        remaining_pids.retain(|&p| p != pid);
                    }
                    Ok((_, ProcessStatus::StillRunning)) | Ok((_, ProcessStatus::Continued)) => {
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }

            restore_shell_termios();
            return (last_code, captured_out, captured_err);
        }

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

    pub(crate) fn evaluate_command_expr_with_input(
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
            CommandExpr::Background(inner) => {
                self.evaluate_background_command_expr(inner, scope_indices)
            }
        }
    }

    pub(crate) fn evaluate_background_command_expr(
        &mut self,
        inner: &CommandExpr,
        scope_indices: &mut Vec<usize>,
    ) -> (i32, Vec<u8>, Vec<u8>) {
        use std::process::{Command, Stdio};

        match inner {
            CommandExpr::Call {
                env_assignments,
                identifier,
                args,
                stdout_redirect,
            } => {
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

                let (identifier_val, _receiver) =
                    self.resolve_callable_target(identifier, scope_indices);

                let identifier_str = identifier_val.to_string();
                let (res_cmd, res_args) = self.resolve_command(identifier_str);
                let all_args = self.build_command_args(res_args, args, scope_indices);

                let popped_scope_index = scope_indices
                    .pop()
                    .expect("Failed to pop temp env scope index");
                if popped_scope_index != temp_scope_index {
                    panic!("Scope stack mismatch");
                }
                self.scopes.pop();

                let mut c = Command::new(&res_cmd);
                c.args(&all_args);
                c.envs(
                    env_assignment_values
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str())),
                );

                // Background stdin is null to prevent SIGTTIN
                c.stdin(Stdio::null());

                if let Some(path) = &stdout_redirect_path {
                    match File::create(path) {
                        Ok(file) => {
                            c.stdout(Stdio::from(file));
                        }
                        Err(e) => {
                            use std::io::Write as _;
                            let _ = writeln!(io::stderr(), "{e}");
                            return (-1, vec![], vec![]);
                        }
                    }
                } else {
                    c.stdout(Stdio::inherit());
                }
                c.stderr(Stdio::inherit());

                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    c.process_group(0);
                    unsafe {
                        c.pre_exec(reset_child_signals_pre_exec);
                    }
                }

                match c.spawn() {
                    Ok(child) => {
                        let child_pid = child.id() as i32;
                        let full_cmd = if all_args.is_empty() {
                            format!("{res_cmd} &")
                        } else {
                            format!("{res_cmd} {} &", all_args.join(" "))
                        };

                        let job_id = self.job_manager.add_job(
                            child_pid,
                            vec![child_pid],
                            full_cmd,
                            JobState::Running,
                            None,
                        );

                        use std::io::Write as _;
                        let _ = writeln!(io::stderr(), "[{job_id}] {child_pid}");
                        (0, vec![], vec![])
                    }
                    Err(e) => {
                        use std::io::Write as _;
                        let _ = writeln!(io::stderr(), "Failed to spawn {res_cmd}: {e}");
                        (-1, vec![], vec![])
                    }
                }
            }
            CommandExpr::Pipeline(stages) => {
                let mut resolved_stages = Vec::new();
                for stage in stages {
                    let CommandExpr::Call {
                        env_assignments,
                        identifier,
                        args,
                        stdout_redirect,
                    } = stage
                    else {
                        use std::io::Write as _;
                        let _ = writeln!(
                            io::stderr(),
                            "Background execution only supports pipelines of external commands."
                        );
                        return (-1, vec![], vec![]);
                    };

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

                    let (identifier_val, _) =
                        self.resolve_callable_target(identifier, scope_indices);
                    let popped = scope_indices.pop();
                    if popped != Some(temp_scope_index) {
                        panic!("Scope stack mismatch");
                    }
                    self.scopes.pop();

                    let identifier_str = identifier_val.to_string();
                    let (res_cmd, res_args) = self.resolve_command(identifier_str);
                    let all_args = self.build_command_args(res_args, args, scope_indices);

                    resolved_stages.push(ExternalPipelineStage {
                        cmd: res_cmd,
                        args: all_args,
                        env_assignments: env_assignment_values,
                        stdout_redirect: stdout_redirect_path,
                    });
                }

                let total_stages = resolved_stages.len();
                if total_stages == 0 {
                    return (0, vec![], vec![]);
                }

                let pipeline_cmd_summary = resolved_stages
                    .iter()
                    .map(|s| {
                        if s.args.is_empty() {
                            s.cmd.clone()
                        } else {
                            format!("{} {}", s.cmd, s.args.join(" "))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
                    + " &";

                let mut children: Vec<std::process::Child> = Vec::with_capacity(total_stages);
                let mut prev_stdout: Option<std::process::ChildStdout> = None;
                let mut leader_pid: i32 = 0;

                for (i, stage) in resolved_stages.into_iter().enumerate() {
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
                        if is_first {
                            c.process_group(0);
                        } else {
                            c.process_group(leader_pid);
                        }
                        unsafe {
                            c.pre_exec(reset_child_signals_pre_exec);
                        }
                    }

                    if is_first {
                        c.stdin(Stdio::null());
                    } else {
                        let stdin_source = prev_stdout
                            .take()
                            .expect("Previous stage stdout must be available");
                        c.stdin(Stdio::from(stdin_source));
                    }

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
                                use std::io::Write as _;
                                let _ = writeln!(io::stderr(), "{e}");
                                return (-1, vec![], vec![]);
                            }
                        }
                    } else {
                        c.stdout(Stdio::inherit());
                    }

                    c.stderr(Stdio::inherit());

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
                            use std::io::Write as _;
                            let _ = writeln!(io::stderr(), "{e}");
                            return (-1, vec![], vec![]);
                        }
                    }
                }

                let pids: Vec<i32> = children.iter().map(|c| c.id() as i32).collect();
                let job_id = self.job_manager.add_job(
                    leader_pid,
                    pids,
                    pipeline_cmd_summary,
                    JobState::Running,
                    None,
                );

                use std::io::Write as _;
                let _ = writeln!(io::stderr(), "[{job_id}] {leader_pid}");

                (0, vec![], vec![])
            }
            _ => {
                use std::io::Write as _;
                let _ = writeln!(
                    io::stderr(),
                    "Background execution only supports commands and pipelines."
                );
                (-1, vec![], vec![])
            }
        }
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

    pub(crate) fn build_command_args(
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
}
