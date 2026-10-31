use std::{io, path::PathBuf};

use lsh_syntax::{paths, print_error};

use crate::{
    eval::{self, Evaluator},
    io::IoCtx,
};

pub fn source<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if args.len() > 1 {
        print_error(&mut io.err, "Too many arguments.");
        return 1;
    }

    if let Some(path_arg) = args.first() {
        let path = paths::expand_path(PathBuf::from(path_arg));

        eval::exec::exec_file(eval, path)
    } else {
        print_error(&mut io.err, "An argument is required.");
        1
    }
}
