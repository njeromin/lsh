use std::{io::Stdout, path::PathBuf};

use crate::{
    eval::{Evaluator, exec::exec_file},
    paths,
    ui::informer,
};

pub fn source(eval: &mut Evaluator, stdout: &mut Stdout, args: Vec<String>) {
    if args.len() > 1 {
        informer::print_error(stdout, "Too many arguments.");
        return;
    }

    if let Some(path_arg) = args.first() {
        let path = paths::expand_path(PathBuf::from(path_arg));

        exec_file(eval, path);
    } else {
        informer::print_error(stdout, "An argument is required.");
    }
}

