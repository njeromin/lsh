use std::io;

use crate::{eval::Evaluator, io::IoCtx, jobs::JobState};

pub fn bg<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    ctx: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    let spec = args.first().map(|s| s.as_str()).unwrap_or("");
    let job_id = match eval.job_manager.find_by_spec(spec) {
        Ok(id) => id,
        Err(e) => {
            let _ = writeln!(ctx.err, "bg: {e}");
            return 1;
        }
    };

    let Some(job) = eval.job_manager.get_job_mut(job_id) else {
        let _ = writeln!(ctx.err, "bg: job not found");
        return 1;
    };

    let pgid = job.pgid;
    let cmd = job.command.clone();

    if matches!(job.state, JobState::Running) {
        let _ = writeln!(ctx.err, "bg: job {job_id} already in background");
        return 0;
    }

    #[cfg(unix)]
    unsafe {
        libc::killpg(pgid, libc::SIGCONT);
    }

    job.state = JobState::Running;
    let _ = writeln!(ctx.out, "[{job_id}]+  {cmd} &");

    0
}
