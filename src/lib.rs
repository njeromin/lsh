use std::io;

pub enum IoStreamMode {
    Inherit,
    Pipe,
    Null,
}

pub struct IoCtx<R: io::Read, W: io::Write, WE: io::Write = W> {
    pub input: R,
    pub out: W,
    pub err: WE,
    pub input_mode: IoStreamMode,
    pub out_mode: IoStreamMode,
    pub err_mode: IoStreamMode,
}

impl Default for IoCtx<std::io::Stdin, std::io::Stdout, std::io::Stderr> {
    fn default() -> Self {
        Self {
            input: std::io::stdin(),
            out: std::io::stdout(),
            err: std::io::stderr(),
            input_mode: IoStreamMode::Inherit,
            out_mode: IoStreamMode::Inherit,
            err_mode: IoStreamMode::Inherit,
        }
    }
}
