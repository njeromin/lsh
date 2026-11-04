use std::io;

use crate::{eval::Evaluator, io::IoCtx};

pub fn fg<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    ctx: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    let spec = args.first().map(|s| s.as_str()).unwrap_or("");
    let job_id = match eval.job_manager.find_by_spec(spec) {
        Ok(id) => id,
        Err(e) => {
            let _ = writeln!(ctx.err, "fg: {e}");
            return 1;
        }
    };

    eval.resume_foreground_job(job_id, ctx)
}
