use std::{
    io,
    process::{Command, Stdio},
};

use lsh_syntax::print_error;
use lsh_sys::signals;

use crate::{
    builtins,
    eval::Evaluator,
    io::{IoCtx, IoStreamMode},
};

pub fn exec_cmd<R: io::Read + Send, W: io::Write + Send, WE: io::Write + Send>(
    evaluator: &mut Evaluator,
    cmd: &str,
    args: Vec<String>,
    env_assignments: &[(String, String)],
    ctx: &mut IoCtx<R, W, WE>,
) -> i32 {
    if let Some(builtin) = builtins::lookup(cmd) {
        return builtins::dispatch(builtin.kind, evaluator, ctx, args);
    }

    match spawn_proc(Some(evaluator), cmd, args, env_assignments, ctx) {
        Ok(status) => status,
        Err(e) => {
            print_error(&mut ctx.err, &e.to_string());
            -1
        }
    }
}

pub fn spawn_proc<R: io::Read + Send, W: io::Write + Send, WE: io::Write + Send>(
    evaluator: Option<&mut Evaluator>,
    cmd: &str,
    args: Vec<String>,
    env_assignments: &[(String, String)],
    ctx: &mut IoCtx<R, W, WE>,
) -> io::Result<i32> {
    let mut c = Command::new(cmd);
    c.args(args.clone());
    c.envs(
        env_assignments
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str())),
    );

    c.stdin(match ctx.input_mode {
        IoStreamMode::Inherit => Stdio::inherit(),
        IoStreamMode::Pipe => Stdio::piped(),
        IoStreamMode::Null => Stdio::null(),
    });
    c.stdout(match ctx.out_mode {
        IoStreamMode::Inherit => Stdio::inherit(),
        IoStreamMode::Pipe => Stdio::piped(),
        IoStreamMode::Null => Stdio::null(),
    });
    c.stderr(match ctx.err_mode {
        IoStreamMode::Inherit => Stdio::inherit(),
        IoStreamMode::Pipe => Stdio::piped(),
        IoStreamMode::Null => Stdio::null(),
    });

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        if signals::is_interactive_tty() {
            c.process_group(0);
        }
        unsafe {
            c.pre_exec(|| signals::reset_child_signals_pre_exec());
        }
    }

    match c.spawn() {
        Ok(mut child) => {
            let child_pid = child.id() as i32;
            let is_foreground = matches!(ctx.input_mode, IoStreamMode::Inherit)
                || matches!(ctx.out_mode, IoStreamMode::Inherit);

            let _guard = if is_foreground && signals::is_interactive_tty() {
                signals::TerminalForegroundGuard::give_to_pgid(child_pid)
            } else {
                signals::TerminalForegroundGuard::inactive()
            };

            std::thread::scope(|s| {
                if matches!(ctx.input_mode, IoStreamMode::Pipe)
                    && let Some(mut stdin) = child.stdin.take()
                {
                    let input = &mut ctx.input;
                    s.spawn(move || {
                        let _ = io::copy(input, &mut stdin);
                    });
                }

                if matches!(ctx.out_mode, IoStreamMode::Pipe)
                    && let Some(mut stdout) = child.stdout.take()
                {
                    let out = &mut ctx.out;
                    s.spawn(move || {
                        let _ = io::copy(&mut stdout, out);
                    });
                }

                if matches!(ctx.err_mode, IoStreamMode::Pipe)
                    && let Some(mut stderr) = child.stderr.take()
                {
                    let err = &mut ctx.err;
                    s.spawn(move || {
                        let _ = io::copy(&mut stderr, err);
                    });
                }
            });

            #[cfg(unix)]
            if is_foreground && signals::is_interactive_tty() {
                let exit_code;
                loop {
                    match signals::wait_pid_status(child_pid, libc::WUNTRACED) {
                        Ok((pid, signals::ProcessStatus::Stopped(sig))) if pid == child_pid => {
                            let saved_termios = signals::get_terminal_attributes();
                            signals::restore_shell_termios();
                            drop(_guard);

                            let full_cmd = if args.is_empty() {
                                cmd.to_string()
                            } else {
                                format!("{cmd} {}", args.join(" "))
                            };

                            if let Some(eval) = evaluator {
                                let job_id = eval.job_manager.add_job(
                                    child_pid,
                                    vec![child_pid],
                                    full_cmd.clone(),
                                    crate::jobs::JobState::Stopped(sig),
                                    saved_termios,
                                );
                                use std::io::Write as _;
                                let _ = writeln!(
                                    io::stderr(),
                                    "\n[{job_id}]+  Stopped                 {full_cmd}"
                                );
                            }
                            return Ok(signals::signal_exit_code(sig));
                        }
                        Ok((pid, signals::ProcessStatus::Exited(code))) if pid == child_pid => {
                            exit_code = code;
                            break;
                        }
                        Ok((pid, signals::ProcessStatus::Signaled(sig))) if pid == child_pid => {
                            if sig == signals::SIGINT {
                                use std::io::Write as _;
                                let _ = writeln!(io::stderr());
                            }
                            exit_code = signals::signal_exit_code(sig);
                            break;
                        }
                        Ok((_, signals::ProcessStatus::StillRunning))
                        | Ok((_, signals::ProcessStatus::Continued)) => {
                            continue;
                        }
                        Err(e) if e.raw_os_error() == Some(libc::EINTR) => {
                            continue;
                        }
                        Err(e) => {
                            return Err(e);
                        }
                        _ => {}
                    }
                }
                signals::restore_shell_termios();
                Ok(exit_code)
            } else {
                let status = child.wait()?;
                use std::os::unix::process::ExitStatusExt;
                if let Some(code) = status.code() {
                    Ok(code)
                } else if let Some(sig) = status.signal() {
                    if sig == signals::SIGINT {
                        use std::io::Write as _;
                        let _ = writeln!(io::stderr());
                    }
                    Ok(signals::signal_exit_code(sig))
                } else {
                    Ok(1)
                }
            }
            #[cfg(not(unix))]
            {
                Ok(status.code().unwrap_or(1))
            }
        }
        Err(e) => {
            // TODO: Errors could probably be handled more cleanly.

            // If cmd cannot be found, do custom error message, otherwise just print the message.
            if e.kind() == io::ErrorKind::NotFound {
                print_error(&mut ctx.err, format!("Command '{cmd}' not found.").as_str());
            } else {
                print_error(&mut ctx.err, e.to_string().as_str());
            }

            Ok(-1)
        }
    }
}
