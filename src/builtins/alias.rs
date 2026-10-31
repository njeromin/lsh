use std::io;

use crate::{eval::Evaluator, terminal};

pub fn alias<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
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
                        terminal::print_error(&mut io.err, "A value is required.");
                        1
                    }
                } else {
                    terminal::print_error(&mut io.err, "A name is required.");
                    1
                }
            }
            _ => {
                terminal::print_error(&mut io.err, "A subcommand (set/list) is required.");
                1
            }
        }
    } else {
        terminal::print_error(&mut io.err, "A subcommand (set/list) is required.");
        1
    }
}
