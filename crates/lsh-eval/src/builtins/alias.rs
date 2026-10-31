use std::io;

use lsh_syntax::print_error;

use crate::{eval::Evaluator, io::IoCtx};

pub fn alias<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if let Some(subcmd) = args.first() {
        match subcmd.as_str() {
            "list" => {
                let aliases = eval.get_aliases();

                for (alias, cmd) in aliases {
                    let _ = writeln!(&mut io.out, "{alias}: {cmd}");
                }
                0
            }
            "set" => {
                if let Some(arg1) = args.get(1) {
                    if let Some(arg2) = args.get(2) {
                        eval.set_alias(arg1.to_string(), arg2.to_string());
                        0
                    } else {
                        print_error(&mut io.err, "A value is required.");
                        1
                    }
                } else {
                    print_error(&mut io.err, "A name is required.");
                    1
                }
            }
            _ => {
                print_error(&mut io.err, "A subcommand (set/list) is required.");
                1
            }
        }
    } else {
        print_error(&mut io.err, "A subcommand (set/list) is required.");
        1
    }
}
