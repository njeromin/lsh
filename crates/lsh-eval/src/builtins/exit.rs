use std::{io, process};

use lsh_syntax::print_error;

use crate::{eval::Evaluator, io::IoCtx};

pub fn exit<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if eval.job_manager.has_stopped_jobs() && !eval.warned_stopped_jobs {
        eval.warned_stopped_jobs = true;
        print_error(&mut io.err, "There are stopped jobs.");
        return 1;
    }

    process::exit(if args.is_empty() {
        0
    } else {
        match args[0].parse::<i32>() {
            Ok(code) => code,
            Err(_) => {
                print_error(&mut io.err, "Invalid exit code.");
                1
            }
        }
    });
}
