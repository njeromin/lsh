use std::{
    io,
    process::{Command, Stdio},
};

use lsh::IoStreamMode;
use which::which;

use crate::{
    builtins::{self},
    eval::Evaluator,
    ui::informer,
};

pub fn is_valid_cmd(cmd: &str) -> bool {
    builtins::is_valid_builtin(cmd) || which(cmd).is_ok()
}

// TODO: This could probably really just go in evaluator, stop mixing str and string, stop repeating stdout calls
pub fn exec_cmd<R: io::Read, W: io::Write, WE: io::Write>(
    evaluator: &mut Evaluator,
    cmd: &str,
    args: Vec<String>,
    ctx: &mut lsh::IoCtx<R, W, WE>,
) {
    match cmd {
        "exit" => builtins::exit::exit(evaluator, ctx, args),
        "source" => builtins::source::source(evaluator, ctx, args),
        "exec" => builtins::exec::exec(evaluator, ctx, args),
        "alias" => builtins::alias::alias(evaluator, ctx, args),
        "cd" => builtins::cd::cd(evaluator, ctx, args),
        _ => {
            // TODO: Handle error.
            if let Err(e) = spawn_proc(cmd, args, ctx) {
                informer::print_error(&mut ctx.err, &e.to_string());
            }
        }
    }
}

pub fn spawn_proc<R: io::Read, W: io::Write, WE: io::Write>(
    cmd: &str,
    args: Vec<String>,
    ctx: &mut lsh::IoCtx<R, W, WE>,
) -> io::Result<i32> {
    let mut c = Command::new(cmd);
    c.args(args);

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
            if matches!(ctx.input_mode, IoStreamMode::Pipe) {
                if let Some(mut stdin) = child.stdin.take() {
                    io::copy(&mut ctx.input, &mut stdin)?;
                }
            }

            if matches!(ctx.out_mode, IoStreamMode::Pipe) {
                if let Some(mut stdout) = child.stdout.take() {
                    io::copy(&mut stdout, &mut ctx.out)?;
                }
            }

            if matches!(ctx.err_mode, IoStreamMode::Pipe) {
                if let Some(mut stderr) = child.stderr.take() {
                    io::copy(&mut stderr, &mut ctx.err)?;
                }
            }

            Ok(child.wait()?.code().unwrap_or(1))
        }
        Err(e) => {
            // TODO: Errors could probably be handled more cleanly.

            // If cmd cannot be found, do custom error message, otherwise just print the message.
            if e.kind() == io::ErrorKind::NotFound {
                informer::print_error(&mut ctx.err, format!("Command '{cmd}' not found.").as_str());
            } else {
                informer::print_error(&mut ctx.err, e.to_string().as_str());
            }

            Ok(-1)
        }
    }
}
