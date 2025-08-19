use std::io::Stdout;

use crate::{eval::Evaluator, ui::informer};

pub fn alias(eval: &mut Evaluator, stdout: &mut Stdout, args: Vec<String>) {
    if let Some(subcmd) = args.first() {
        match subcmd.as_str() {
            "list" => {
                let aliases = eval.get_aliases();

                aliases.iter().for_each(|(alias, cmd)| {
                    println!("{alias}: {cmd}");
                });
            }
            "set" => {
                if let Some(arg1) = args.get(1) {
                    if let Some(arg2) = args.get(2) {
                        eval.set_alias(arg1.to_string(), arg2.to_string());
                    } else {
                        informer::print_error(stdout, "A value is required.");
                    }
                } else {
                    informer::print_error(stdout, "A name is required.");
                }
            }
            _ => informer::print_error(stdout, "A subcommand (set/list) is required."),
        }
    } else {
        informer::print_error(stdout, "A subcommand (set/list) is required.");
    }
}
