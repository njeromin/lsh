use std::{io, path::PathBuf};

use crate::{
    eval::{Evaluator, exec::exec_file},
    paths, ui,
};

pub fn source<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) {
    if args.len() > 1 {
        ui::print_error(&mut io.err, "Too many arguments.");
        return;
    }

    if let Some(path_arg) = args.first() {
        let path = paths::expand_path(PathBuf::from(path_arg));

        exec_file(eval, path);
    } else {
        ui::print_error(&mut io.err, "An argument is required.");
    }
}
