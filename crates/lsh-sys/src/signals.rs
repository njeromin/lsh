use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

static INTERRUPT_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static IS_INTERACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
pub const SIGINT: i32 = libc::SIGINT;
#[cfg(not(unix))]
pub const SIGINT: i32 = 2;

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
                save_shell_termios();
            }
        }
    }
}

#[cfg(unix)]
static SHELL_TERMIOS: OnceLock<libc::termios> = OnceLock::new();

#[cfg(unix)]
pub fn save_shell_termios() {
    if is_interactive_tty() {
        let mut tmodes = std::mem::MaybeUninit::uninit();
        let ret = unsafe { libc::tcgetattr(libc::STDIN_FILENO, tmodes.as_mut_ptr()) };
        if ret == 0 {
            let _ = SHELL_TERMIOS.set(unsafe { tmodes.assume_init() });
        }
    }
}

#[cfg(not(unix))]
pub fn save_shell_termios() {}

#[cfg(unix)]
pub fn restore_shell_termios() {
    if let Some(tmodes) = SHELL_TERMIOS.get() {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSADRAIN, tmodes);
        }
    }
}

#[cfg(not(unix))]
pub fn restore_shell_termios() {}

#[cfg(unix)]
pub fn get_terminal_attributes() -> Option<libc::termios> {
    if is_interactive_tty() {
        let mut tmodes = std::mem::MaybeUninit::uninit();
        let ret = unsafe { libc::tcgetattr(libc::STDIN_FILENO, tmodes.as_mut_ptr()) };
        if ret == 0 {
            return Some(unsafe { tmodes.assume_init() });
        }
    }
    None
}

#[cfg(not(unix))]
pub fn get_terminal_attributes() -> Option<()> {
    None
}

#[cfg(unix)]
pub fn set_terminal_attributes(tmodes: &libc::termios) {
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSADRAIN, tmodes);
    }
}

#[cfg(not(unix))]
pub fn set_terminal_attributes(_tmodes: &()) {}

pub fn reset_child_signals_pre_exec() -> std::io::Result<()> {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTSTP, libc::SIG_DFL);
        libc::signal(libc::SIGTTIN, libc::SIG_DFL);
        libc::signal(libc::SIGTTOU, libc::SIG_DFL);
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGQUIT, libc::SIG_DFL);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Exited(i32),
    Signaled(i32),
    Stopped(i32),
    Continued,
    StillRunning,
}

pub fn wait_pid_status(pid: i32, flags: i32) -> Result<(i32, ProcessStatus), std::io::Error> {
    #[cfg(unix)]
    {
        let mut status: i32 = 0;
        let res = unsafe { libc::waitpid(pid, &mut status, flags) };
        if res < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if res == 0 {
            return Ok((0, ProcessStatus::StillRunning));
        }

        if libc::WIFEXITED(status) {
            Ok((res, ProcessStatus::Exited(libc::WEXITSTATUS(status))))
        } else if libc::WIFSIGNALED(status) {
            Ok((res, ProcessStatus::Signaled(libc::WTERMSIG(status))))
        } else if libc::WIFSTOPPED(status) {
            Ok((res, ProcessStatus::Stopped(libc::WSTOPSIG(status))))
        } else if libc::WIFCONTINUED(status) {
            Ok((res, ProcessStatus::Continued))
        } else {
            Ok((res, ProcessStatus::Exited(1)))
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, flags);
        Ok((pid, ProcessStatus::Exited(0)))
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
    fn signal_exit_code_calculation() {
        assert_eq!(STATUS_SIGINT, 130);
        assert_eq!(signal_exit_code(2), 130);
        assert_eq!(signal_exit_code(9), 137);
        assert_eq!(signal_exit_code(15), 143);
    }
}
