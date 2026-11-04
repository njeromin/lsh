use std::io;

use lsh_syntax::print_error;

use crate::{eval::Evaluator, io::IoCtx, jobs::JobState};

fn parse_signal(s: &str) -> Option<i32> {
    let clean = s
        .trim()
        .trim_start_matches('-')
        .trim_start_matches("SIG")
        .trim_start_matches("sig");

    if let Ok(num) = clean.parse::<i32>() {
        return Some(num);
    }

    match clean.to_ascii_uppercase().as_str() {
        "HUP" => Some(libc::SIGHUP),
        "INT" => Some(libc::SIGINT),
        "QUIT" => Some(libc::SIGQUIT),
        "ILL" => Some(libc::SIGILL),
        "TRAP" => Some(libc::SIGTRAP),
        "ABRT" => Some(libc::SIGABRT),
        "BUS" => Some(libc::SIGBUS),
        "FPE" => Some(libc::SIGFPE),
        "KILL" => Some(libc::SIGKILL),
        "USR1" => Some(libc::SIGUSR1),
        "SEGV" => Some(libc::SIGSEGV),
        "USR2" => Some(libc::SIGUSR2),
        "PIPE" => Some(libc::SIGPIPE),
        "ALRM" => Some(libc::SIGALRM),
        "TERM" => Some(libc::SIGTERM),
        "CONT" => Some(libc::SIGCONT),
        "STOP" => Some(libc::SIGSTOP),
        "TSTP" => Some(libc::SIGTSTP),
        "TTIN" => Some(libc::SIGTTIN),
        "TTOU" => Some(libc::SIGTTOU),
        "CHLD" => Some(libc::SIGCHLD),
        _ => None,
    }
}

fn is_terminating_signal(sig: i32) -> bool {
    sig == libc::SIGTERM
        || sig == libc::SIGKILL
        || sig == libc::SIGINT
        || sig == libc::SIGHUP
        || sig == libc::SIGQUIT
}

pub fn kill<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    ctx: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if args.is_empty() {
        print_error(
            &mut ctx.err,
            "kill: usage: kill [-s sigspec | -signum] pid | jobspec ...",
        );
        return 1;
    }

    let mut sig = libc::SIGTERM;
    let mut targets = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg == "-l" {
            let _ = writeln!(
                ctx.out,
                " 1) SIGHUP   2) SIGINT   3) SIGQUIT  6) SIGABRT  9) SIGKILL\n\
                 10) SIGUSR1 12) SIGUSR2 13) SIGPIPE 14) SIGALRM 15) SIGTERM\n\
                 18) SIGCONT 19) SIGSTOP 20) SIGTSTP 21) SIGTTIN 22) SIGTTOU"
            );
            return 0;
        } else if arg == "-s" {
            i += 1;
            if i >= args.len() {
                print_error(&mut ctx.err, "kill: -s requires an argument");
                return 1;
            }
            if let Some(s) = parse_signal(&args[i]) {
                sig = s;
            } else {
                print_error(&mut ctx.err, &format!("kill: invalid signal: {}", args[i]));
                return 1;
            }
        } else if arg.starts_with('-')
            && arg.len() > 1
            && !arg.chars().nth(1).unwrap().is_ascii_digit()
            && parse_signal(arg).is_some()
        {
            sig = parse_signal(arg).unwrap();
        } else if arg.starts_with('-')
            && arg.len() > 1
            && arg[1..].parse::<i32>().is_ok()
            && targets.is_empty()
        {
            sig = arg[1..].parse::<i32>().unwrap();
        } else {
            targets.push(arg.clone());
        }
        i += 1;
    }

    if targets.is_empty() {
        print_error(&mut ctx.err, "kill: not enough arguments");
        return 1;
    }

    let mut exit_code = 0;

    for target in targets {
        // Check if target is a job spec
        if target.starts_with('%')
            || (eval.job_manager.find_by_spec(&target).is_ok() && target.parse::<i32>().is_err())
        {
            match eval.job_manager.find_by_spec(&target) {
                Ok(job_id) => {
                    let job = eval.job_manager.get_job(job_id).unwrap();
                    let pgid = job.pgid;
                    let was_stopped = matches!(job.state, JobState::Stopped(_));

                    #[cfg(unix)]
                    unsafe {
                        let ret = libc::killpg(pgid, sig);
                        if ret != 0 {
                            // Fallback to sending to individual pids
                            for &pid in &job.pids {
                                libc::kill(pid, sig);
                            }
                        }
                        // If job was stopped and we sent a terminating signal,
                        // wake it with SIGCONT so it can receive the signal and die
                        if was_stopped && is_terminating_signal(sig) {
                            libc::killpg(pgid, libc::SIGCONT);
                            for &pid in &job.pids {
                                libc::kill(pid, libc::SIGCONT);
                            }
                        }
                    }
                }
                Err(e) => {
                    print_error(&mut ctx.err, &format!("kill: {e}"));
                    exit_code = 1;
                }
            }
        } else if let Ok(pid) = target.parse::<i32>() {
            let was_stopped = eval
                .job_manager
                .find_by_pid_mut(pid)
                .is_some_and(|j| matches!(j.state, JobState::Stopped(_)))
                || eval
                    .job_manager
                    .find_by_pgid(pid.abs())
                    .is_some_and(|j| matches!(j.state, JobState::Stopped(_)));

            #[cfg(unix)]
            unsafe {
                let res = if pid < 0 {
                    libc::killpg(-pid, sig)
                } else {
                    libc::kill(pid, sig)
                };
                if res != 0 {
                    let err = io::Error::last_os_error();
                    print_error(&mut ctx.err, &format!("kill: ({pid}) - {err}"));
                    exit_code = 1;
                } else if was_stopped && is_terminating_signal(sig) {
                    if pid < 0 {
                        libc::killpg(-pid, libc::SIGCONT);
                    } else {
                        libc::kill(pid, libc::SIGCONT);
                    }
                }
            }
        } else {
            print_error(
                &mut ctx.err,
                &format!("kill: {target}: arguments must be process or job IDs"),
            );
            exit_code = 1;
        }
    }

    // Immediately reap any children that terminated
    eval.reap_and_notify_jobs(&mut ctx.err);

    exit_code
}
