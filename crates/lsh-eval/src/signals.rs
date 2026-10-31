use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

static INTERRUPT_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static IS_INTERACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
pub const STATUS_SIGINT: i32 = 128 + libc::SIGINT;
#[cfg(not(unix))]
pub const STATUS_SIGINT: i32 = 130;

pub const fn signal_exit_code(sig: i32) -> i32 {
    128 + sig
}

pub fn get_interrupt_flag() -> Arc<AtomicBool> {
    INTERRUPT_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

pub fn is_interrupted() -> bool {
    get_interrupt_flag().load(Ordering::Relaxed)
}

pub fn check_and_reset_interrupted() -> bool {
    get_interrupt_flag().swap(false, Ordering::Relaxed)
}

pub fn set_interrupted() {
    get_interrupt_flag().store(true, Ordering::Relaxed);
}

pub fn reset_interrupted() {
    get_interrupt_flag().store(false, Ordering::Relaxed);
}

pub fn is_interactive_session() -> bool {
    IS_INTERACTIVE.load(Ordering::Relaxed)
}

pub fn is_interactive_tty() -> bool {
    if !is_interactive_session() {
        return false;
    }

    #[cfg(unix)]
    {
        unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn init_interactive_terminal() {
    IS_INTERACTIVE.store(true, Ordering::Relaxed);

    let flag = get_interrupt_flag();
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, flag);

    #[cfg(unix)]
    {
        unsafe {
            // Ignore background terminal signals so tcsetpgrp does not suspend the shell.
            libc::signal(libc::SIGTTOU, libc::SIG_IGN);
            libc::signal(libc::SIGTTIN, libc::SIG_IGN);
            libc::signal(libc::SIGTSTP, libc::SIG_IGN);

            if libc::isatty(libc::STDIN_FILENO) == 1 {
                let shell_pgid = libc::getpgrp();
                libc::tcsetpgrp(libc::STDIN_FILENO, shell_pgid);
            }
        }
    }
}

/// RAII guard that temporarily gives controlling terminal foreground ownership (tcsetpgrp)
/// to a child process group, restoring ownership back to the shell process group when dropped.
pub struct TerminalForegroundGuard {
    #[cfg(unix)]
    active: bool,
    #[cfg(unix)]
    shell_pgid: i32,
}

impl TerminalForegroundGuard {
    pub fn give_to_pgid(pgid: i32) -> Self {
        #[cfg(unix)]
        {
            if is_interactive_tty() && pgid > 0 {
                let shell_pgid = unsafe { libc::getpgrp() };
                unsafe {
                    libc::tcsetpgrp(libc::STDIN_FILENO, pgid);
                }
                Self {
                    active: true,
                    shell_pgid,
                }
            } else {
                Self {
                    active: false,
                    shell_pgid: 0,
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = pgid;
            Self {}
        }
    }

    pub fn inactive() -> Self {
        #[cfg(unix)]
        {
            Self {
                active: false,
                shell_pgid: 0,
            }
        }
        #[cfg(not(unix))]
        {
            Self {}
        }
    }
}

impl Drop for TerminalForegroundGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.active {
            unsafe {
                libc::tcsetpgrp(libc::STDIN_FILENO, self.shell_pgid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupt_flag_set_and_reset() {
        reset_interrupted();
        assert!(!is_interrupted());

        set_interrupted();
        assert!(is_interrupted());

        assert!(check_and_reset_interrupted());
        assert!(!is_interrupted());
    }

    #[test]
    fn inactive_guard_does_not_panic() {
        let guard = TerminalForegroundGuard::inactive();
        drop(guard);
    }

    #[test]
    fn signal_exit_code_calculation() {
        assert_eq!(STATUS_SIGINT, 130);
        assert_eq!(signal_exit_code(2), 130);
        assert_eq!(signal_exit_code(9), 137);
        assert_eq!(signal_exit_code(15), 143);
    }
}
