use std::{io, path::PathBuf};

use crate::{
    eval::{Evaluator, exec::exec_file, value::Value},
    paths, ui,
};

pub fn exec<R: io::Read, W: io::Write, WE: io::Write>(
    _: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if let Some(path_arg) = args.first() {
        let path = paths::expand_path(PathBuf::from(path_arg));

        let mut exec_evaluator = Evaluator::new().with_globals();

        for (i, arg) in args.iter().enumerate() {
            exec_evaluator.push_var(0, i.to_string(), Value::String(arg.to_owned()));
        }

        exec_file(&mut exec_evaluator, path)
    } else {
        ui::print_error(&mut io.err, "An argument is required.");
        1
    }
}
