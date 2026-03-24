use std::{io, process};

use crate::{eval::Evaluator, ui};

pub fn exit<R: io::Read, W: io::Write, WE: io::Write>(
    _: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) {
    process::exit(if args.is_empty() {
        0
    } else {
        match args[0].parse::<i32>() {
            Ok(code) => code,
            Err(_) => {
                ui::print_error(&mut io.err, "Invalid exit code.");
                1
            }
        }
    });
}
