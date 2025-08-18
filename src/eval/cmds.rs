use std::{
    env, fs,
    io::{self, stdout},
    path::PathBuf,
    process::{self, Command},
};

use which::which;

use crate::{
    debug,
    eval::Evaluator,
    lexer::{self},
    parser::Parser,
    paths,
    ui::informer,
};

// TODO: Is there a better way to do this?
pub fn is_valid_cmd(cmd: &str) -> bool {
    match cmd {
        "exit" => true,
        "source" => true,
        "alias" => true,
        "cd" => true,
        _ => which(cmd).is_ok(),
    }
}

// TODO: This could probably really just go in evaluator, stop mixing str and string, stop repeating stdout calls
pub fn exec_cmd(evaluator: &mut Evaluator, cmd: &str, args: Vec<String>) {
    match cmd {
        "exit" => {
            // NOTE: This should not be how the shell exits, it should be cleaner.
            process::exit(0);
        }
        "source" => {
            if args.len() > 1 {
                informer::print_error(&mut stdout(), "Too many arguments.");
                return;
            }

            if let Some(arg) = args.first() {
                // NOTE: Does this need to happen?
                let path = paths::expand_path(PathBuf::from(arg));

                match fs::read_to_string(path) {
                    Ok(input) => {
                        let tokens = lexer::lex_input(input.as_str());

                        debug::print_lexer(&tokens);

                        let mut parser = Parser::new(tokens);
                        let prog = parser.parse_program();

                        debug::print_parser(&prog);

                        evaluator.evaluate(prog);
                    }
                    Err(e) => {
                        informer::print_error(&mut stdout(), e.to_string().as_str());
                    }
                }
            } else {
                informer::print_error(&mut stdout(), "An argument is required.");
            }
        }
        // TODO: Rewrite some of this - so many branches for what.
        "alias" => {
            if let Some(subcmd) = args.first() {
                match subcmd.as_str() {
                    "list" => {
                        evaluator.aliases.iter().for_each(|(alias, cmd)| {
                            println!("{alias}: {cmd}");
                        });
                    }
                    "set" => {
                        if let Some(arg1) = args.get(1) {
                            if let Some(arg2) = args.get(2) {
                                evaluator.set_alias(arg1.to_string(), arg2.to_string());
                            } else {
                                informer::print_error(
                                    &mut stdout(),
                                    "A second argument is required.",
                                );
                            }
                        } else {
                            informer::print_error(&mut stdout(), "A second argument is required.");
                        }
                    }
                    _ => {
                        informer::print_error(&mut stdout(), "A subcommand (set/list) is required.")
                    }
                }
            } else {
                informer::print_error(&mut stdout(), "A subcommand (set/list) is required.");
            }
        }
        "cd" => {
            // TODO: Implement cd correctly.
            if args.len() > 1 {
                informer::print_error(&mut stdout(), "Too many arguments.");
                return;
            }

            if let Some(arg) = args.first() {
                let path = paths::expand_path(PathBuf::from(arg));

                env::set_current_dir(&path).unwrap_or_else(|_| {
                    informer::print_error(
                        &mut stdout(),
                        format!("Failed to change directory to '{arg}'.").as_str(),
                    );
                });
            } else {
                informer::print_error(&mut stdout(), "An argument is required.");
            }
        }
        _ => {
            spawn_proc(cmd, args);
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
