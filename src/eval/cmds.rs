use std::{
    io,
    process::{Command, Stdio},
};

use lsh::IoStreamMode;

use crate::{
    builtins::{self},
    eval::Evaluator,
    ui,
};

pub fn exec_cmd<R: io::Read, W: io::Write, WE: io::Write>(
    evaluator: &mut Evaluator,
    cmd: &str,
    args: Vec<String>,
    env_assignments: &[(String, String)],
    ctx: &mut lsh::IoCtx<R, W, WE>,
) -> i32 {
    if let Some(builtin) = builtins::lookup(cmd) {
        builtins::dispatch(builtin.kind, evaluator, ctx, args);
        return 0;
    }

    match spawn_proc(cmd, args, env_assignments, ctx) {
        Ok(status) => status,
        Err(e) => {
            ui::print_error(&mut ctx.err, &e.to_string());
            -1
        }
    }
}

pub fn spawn_proc<R: io::Read, W: io::Write, WE: io::Write>(
    cmd: &str,
    args: Vec<String>,
    env_assignments: &[(String, String)],
    ctx: &mut lsh::IoCtx<R, W, WE>,
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

    match c.spawn() {
        Ok(mut child) => {
            if matches!(ctx.input_mode, IoStreamMode::Pipe)
                && let Some(mut stdin) = child.stdin.take()
            {
                io::copy(&mut ctx.input, &mut stdin)?;
            }

            if matches!(ctx.out_mode, IoStreamMode::Pipe)
                && let Some(mut stdout) = child.stdout.take()
            {
                io::copy(&mut stdout, &mut ctx.out)?;
            }

            if matches!(ctx.err_mode, IoStreamMode::Pipe)
                && let Some(mut stderr) = child.stderr.take()
            {
                io::copy(&mut stderr, &mut ctx.err)?;
            }

            Ok(child.wait()?.code().unwrap_or(0))
        }
        Err(e) => {
            // TODO: Errors could probably be handled more cleanly.

            // If cmd cannot be found, do custom error message, otherwise just print the message.
            if e.kind() == io::ErrorKind::NotFound {
                ui::print_error(&mut ctx.err, format!("Command '{cmd}' not found.").as_str());
            } else {
                ui::print_error(&mut ctx.err, e.to_string().as_str());
            }

            Ok(-1)
        }
    }
}
