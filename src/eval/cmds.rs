use std::{
    io::{self, stdout},
    process::{self, Command},
};

use crate::{eval::Evaluator, ui::informer};

// TODO: Stop mixing str and string
pub fn exec_cmd(evaluator: &mut Evaluator, input_cmd: &str, input_args: Vec<String>) {
    let unaliased_cmd = evaluator.resolve_alias(String::from(input_cmd));
    let (resolved_cmd, resolved_args) = resolve_cmd(&unaliased_cmd);

    // Concatenate arguments from the input cmd with the provided arguments from the evaluator.
    let args = [resolved_args, input_args].concat();

    match resolved_cmd {
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
            spawn_proc(resolved_cmd, args);
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

fn resolve_cmd(unsplit_cmd: &str) -> (&str, Vec<String>) {
    // Split the unsplit command by spaces, take out the command itself, and return the rest of the
    // parts as arguments.
    // TODO: Write this better.
    let mut split_cmd = unsplit_cmd.split(" ");
    let cmd = split_cmd.next().expect("Failed to get first command.");
    let args: Vec<String> = split_cmd.map(|arg| arg.to_string()).collect();

    (cmd, args)
}
