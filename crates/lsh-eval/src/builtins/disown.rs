use std::io;

use crate::{eval::Evaluator, io::IoCtx};

pub fn disown<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    ctx: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    let mut flag_a = false;
    let mut specs = Vec::new();

    for arg in args {
        if arg == "-a" {
            flag_a = true;
        } else if arg == "-h" {
            // In full POSIX disown, -h marks the job not to receive SIGHUP.
            // In lsh, jobs not in the table won't receive SIGHUP on exit.
        } else {
            specs.push(arg);
        }
    }

    if flag_a {
        eval.job_manager.jobs.clear();
        eval.job_manager.current_job = None;
        eval.job_manager.previous_job = None;
        return 0;
    }

    if specs.is_empty() {
        if let Some(id) = eval.job_manager.current_job {
            eval.job_manager.remove_job(id);
            return 0;
        } else {
            let _ = writeln!(ctx.err, "disown: current: no such job");
            return 1;
        }
    }

    let mut status = 0;
    for spec in specs {
        match eval.job_manager.find_by_spec(&spec) {
            Ok(id) => {
                eval.job_manager.remove_job(id);
            }
            Err(e) => {
                let _ = writeln!(ctx.err, "disown: {e}");
                status = 1;
            }
        }
    }

    status
}
