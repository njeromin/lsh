use std::{io::Stdout, process};

use crate::{eval::Evaluator, ui::informer};

pub fn exit(_: &mut Evaluator, stdout: &mut Stdout, args: Vec<String>) {
    process::exit(if args.is_empty() {
        0
    } else {
        match args[0].parse::<i32>() {
            Ok(code) => code,
            Err(_) => {
                informer::print_error(stdout, "Invalid exit code.");
                1
            }
        }
    });
}
