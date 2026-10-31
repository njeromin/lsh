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

    match spawn_proc(cmd, args, env_assignments, ctx) {
        Ok(status) => status,
        Err(e) => {
            print_error(&mut ctx.err, &e.to_string());
            -1
        }
    }
}

pub fn spawn_proc<R: io::Read + Send, W: io::Write + Send, WE: io::Write + Send>(
    cmd: &str,
    args: Vec<String>,
    env_assignments: &[(String, String)],
    ctx: &mut IoCtx<R, W, WE>,
) -> io::Result<i32> {
    let mut c = Command::new(cmd);
    c.args(args);
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

            let status = child.wait()?;
            #[cfg(unix)]
            {
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
