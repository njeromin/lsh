use std::{env, io::Stdout, path::PathBuf};

use crate::{eval::Evaluator, paths, ui::informer};

pub fn cd(_: &mut Evaluator, stdout: &mut Stdout, args: Vec<String>) {
    if args.len() > 1 {
        informer::print_error(stdout, "Too many arguments.");
        return;
    }

    if let Some(arg) = args.first() {
        let path = paths::expand_path(PathBuf::from(arg));

        env::set_current_dir(&path).unwrap_or_else(|_| {
            informer::print_error(
                stdout,
                format!("Failed to change directory to '{arg}'.").as_str(),
            );
        });
    } else {
        informer::print_error(stdout, "An argument is required.");
    }
}
