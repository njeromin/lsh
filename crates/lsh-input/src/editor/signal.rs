/// The outcome of reading a line of input from the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// The user typed a command and pressed Enter.
    Success(String),

    /// The user pressed Ctrl+C.
    /// In a shell, this means: cancel the current input, print a newline,
    /// and show a fresh prompt (do not execute anything).
    CtrlC,

    /// The user pressed Ctrl+D on an empty line.
    /// In Unix shells, this signals EOF (End of File), meaning "exit the shell".
    CtrlD,
}

impl Signal {
    /// Returns `true` if the signal is `Success`.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Returns `true` if the signal is `CtrlC`.
    pub fn is_ctrl_c(&self) -> bool {
        matches!(self, Self::CtrlC)
    }

    /// Returns `true` if the signal is `CtrlD`.
    pub fn is_ctrl_d(&self) -> bool {
        matches!(self, Self::CtrlD)
    }

    /// Extracts the command line string if this is `Success`.
    pub fn into_success(self) -> Option<String> {
        match self {
            Self::Success(line) => Some(line),
            _ => None,
        }
    }
}
