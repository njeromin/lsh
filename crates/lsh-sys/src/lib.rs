pub mod signals;

pub use signals::{
    SIGINT, STATUS_SIGINT, TerminalForegroundGuard, check_and_reset_interrupted,
    get_interrupt_flag, init_interactive_terminal, is_interactive_session, is_interactive_tty,
    is_interrupted, reset_interrupted, set_interrupted, signal_exit_code,
};
