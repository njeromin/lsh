use std::io;

use crate::{eval::Evaluator, io::IoCtx};

pub fn jobs<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    ctx: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    let mut flag_l = false;
    let mut flag_p = false;
    let mut flag_r = false;
    let mut flag_s = false;

    for arg in args {
        match arg.as_str() {
            "-l" => flag_l = true,
            "-p" => flag_p = true,
            "-r" => flag_r = true,
            "-s" => flag_s = true,
            "-lp" | "-pl" => {
                flag_l = true;
                flag_p = true;
            }
            other if other.starts_with('-') => {
                let _ = writeln!(ctx.err, "jobs: unknown option: {other}");
                return 1;
            }
            _ => {}
        }
    }

    eval.job_manager.reap_jobs();
    let lines = eval.job_manager.format_jobs(flag_l, flag_p, flag_r, flag_s);
    for line in lines {
        let _ = writeln!(ctx.out, "{line}");
    }

    0
}
