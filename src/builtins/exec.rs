use std::{io::Stdout, path::PathBuf};

use crate::{
    eval::{Evaluator, exec::exec_file, value::Value},
    paths,
    ui::informer,
};

pub fn exec(_: &mut Evaluator, stdout: &mut Stdout, args: Vec<String>) {
    if let Some(path_arg) = args.first() {
        let path = paths::expand_path(PathBuf::from(path_arg));

        let mut exec_evaluator = Evaluator::new();

        for (i, arg) in args.iter().enumerate() {
            exec_evaluator.push_var(0, i.to_string(), Value::String(arg.to_owned()));
        }

        exec_file(&mut exec_evaluator, path);
    } else {
        informer::print_error(stdout, "An argument is required.");
    }
}
