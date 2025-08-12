use std::{
    io::{self, stdout},
    process::{self, Command},
};

use crate::{eval::Evaluator, ui::informer};

// TODO: Stop mixing str and string
pub fn exec_cmd(evaluator: &mut Evaluator, cmd: &str, args: Vec<String>) {
    let resolved_cmd = evaluator.resolve_alias(String::from(cmd));

    match resolved_cmd.as_str() {
        "exit" => {
            // NOTE: This should not be how the shell exits, it should be cleaner.
            process::exit(0);
        }
        "alias" => {
            if let Some(arg1) = args.first() {
                if let Some(arg2) = args.get(1) {
                    evaluator.set_alias(arg1.to_string(), arg2.to_string());
                } else {
                    informer::print_error(&mut stdout(), "Unknown first argument.");
                }
            } else {
                informer::print_error(&mut stdout(), "Unknown first argument.");
            }
        }
        _ => {
            spawn_proc(&resolved_cmd, args);
        }
    }
}

pub fn spawn_proc(cmd: &str, args: Vec<String>) {
    let maybe_cmd = Command::new(cmd).args(args).spawn();

    match maybe_cmd {
        Ok(mut cmd) => match cmd.wait() {
            Ok(_) => {}
            Err(e) => informer::print_error(&mut io::stdout(), e.to_string().as_str()),
        },
        Err(e) => informer::print_error(&mut io::stdout(), e.to_string().as_str()),
    }
}
