use std::io;

use crate::{eval::Evaluator, io::IoCtx};
use lsh_sys::signals::{ProcessStatus, wait_pid_status};

pub fn wait<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    ctx: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if args.is_empty() {
        let job_ids: Vec<usize> = eval.job_manager.jobs.keys().copied().collect();
        for id in job_ids {
            if let Some(job) = eval.job_manager.get_job(id) {
                let pgid = job.pgid;
                let mut remaining_pids = job.pids.clone();
                while !remaining_pids.is_empty() {
                    match wait_pid_status(-pgid, 0) {
                        Ok((pid, ProcessStatus::Exited(_) | ProcessStatus::Signaled(_))) => {
                            remaining_pids.retain(|&p| p != pid);
                        }
                        _ => break,
                    }
                }
                eval.job_manager.remove_job(id);
            }
        }
        return 0;
    }

    let mut last_code = 0;
    for arg in args {
        let job_id = match eval.job_manager.find_by_spec(&arg) {
            Ok(id) => id,
            Err(e) => {
                let _ = writeln!(ctx.err, "wait: {e}");
                last_code = 127;
                continue;
            }
        };

        if let Some(job) = eval.job_manager.get_job(job_id) {
            let pgid = job.pgid;
            let mut remaining_pids = job.pids.clone();
            while !remaining_pids.is_empty() {
                match wait_pid_status(-pgid, 0) {
                    Ok((pid, ProcessStatus::Exited(code))) => {
                        remaining_pids.retain(|&p| p != pid);
                        last_code = code;
                    }
                    Ok((pid, ProcessStatus::Signaled(sig))) => {
                        remaining_pids.retain(|&p| p != pid);
                        last_code = 128 + sig;
                    }
                    _ => break,
                }
            }
            eval.job_manager.remove_job(job_id);
        }
    }

    last_code
}
