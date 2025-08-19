use std::{io, process::Command};

use which::which;

use crate::{builtins, eval::Evaluator, ui::informer};

pub fn is_valid_cmd(cmd: &str) -> bool {
    builtins::is_valid_builtin(cmd) || which(cmd).is_ok()
}

// TODO: This could probably really just go in evaluator, stop mixing str and string, stop repeating stdout calls
pub fn exec_cmd(evaluator: &mut Evaluator, cmd: &str, args: Vec<String>) {
    let mut stdout = io::stdout();

    match cmd {
        "exit" => builtins::exit::exit(evaluator, &mut stdout, args),
        "source" => builtins::source::source(evaluator, &mut stdout, args),
        "exec" => builtins::exec::exec(evaluator, &mut stdout, args),
        "alias" => builtins::alias::alias(evaluator, &mut stdout, args),
        "cd" => builtins::cd::cd(evaluator, &mut stdout, args),
        _ => spawn_proc(cmd, args),
    }
}

pub fn spawn_proc(cmd: &str, args: Vec<String>) {
    let mut stdout = io::stdout();

    let maybe_cmd = Command::new(cmd).args(args).spawn();

    match maybe_cmd {
        Ok(mut cmd) => match cmd.wait() {
            Ok(_) => {}
            Err(e) => informer::print_error(&mut io::stdout(), e.to_string().as_str()),
        },
        Err(e) => {
            // If cmd cannot be found, do custom error message, otherwise just print the message.
            if e.kind() == io::ErrorKind::NotFound {
                informer::print_error(&mut stdout, format!("Command '{cmd}' not found.").as_str());
            } else {
                informer::print_error(&mut stdout, e.to_string().as_str())
            }
        }
    }
}
