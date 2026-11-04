use std::io;

use crate::{eval::Evaluator, io::IoCtx};

pub mod alias;
pub mod bg;
pub mod cd;
pub mod complete;
pub mod disown;
pub mod exec;
pub mod exit;
pub mod export;
pub mod fg;
pub mod fmt_cwd;
pub mod jobs;
pub mod kill;
pub mod plugin;
pub mod source;
pub mod wait;

// pub type BuiltinFn<R, W, WE> = fn(&mut Evaluator, &mut lsh::IoCtx<R, W, WE>, Vec<String>);

pub const BUILTIN_NAMES: &[&str] = &[
    "exit", "source", "exec", "alias", "cd", "fmt-cwd", "complete", "plugin", "export", "jobs",
    "fg", "bg", "wait", "disown", "kill",
];

pub enum BuiltinKind {
    Exit,
    Source,
    Exec,
    Alias,
    Cd,
    FmtCwd,
    Complete,
    Plugin,
    Export,
    Jobs,
    Fg,
    Bg,
    Wait,
    Disown,
    Kill,
}

pub struct BuiltinSpec {
    pub name: &'static str,
    pub kind: BuiltinKind,
}

pub fn lookup(name: &str) -> Option<BuiltinSpec> {
    match name {
        "exit" => Some(BuiltinSpec {
            name: "exit",
            kind: BuiltinKind::Exit,
        }),
        "source" => Some(BuiltinSpec {
            name: "source",
            kind: BuiltinKind::Source,
        }),
        "exec" => Some(BuiltinSpec {
            name: "exec",
            kind: BuiltinKind::Exec,
        }),
        "alias" => Some(BuiltinSpec {
            name: "alias",
            kind: BuiltinKind::Alias,
        }),
        "cd" => Some(BuiltinSpec {
            name: "cd",
            kind: BuiltinKind::Cd,
        }),
        "fmt-cwd" => Some(BuiltinSpec {
            name: "fmt-cwd",
            kind: BuiltinKind::FmtCwd,
        }),
        "complete" => Some(BuiltinSpec {
            name: "complete",
            kind: BuiltinKind::Complete,
        }),
        "plugin" => Some(BuiltinSpec {
            name: "plugin",
            kind: BuiltinKind::Plugin,
        }),
        "export" => Some(BuiltinSpec {
            name: "export",
            kind: BuiltinKind::Export,
        }),
        "jobs" => Some(BuiltinSpec {
            name: "jobs",
            kind: BuiltinKind::Jobs,
        }),
        "fg" => Some(BuiltinSpec {
            name: "fg",
            kind: BuiltinKind::Fg,
        }),
        "bg" => Some(BuiltinSpec {
            name: "bg",
            kind: BuiltinKind::Bg,
        }),
        "wait" => Some(BuiltinSpec {
            name: "wait",
            kind: BuiltinKind::Wait,
        }),
        "disown" => Some(BuiltinSpec {
            name: "disown",
            kind: BuiltinKind::Disown,
        }),
        "kill" => Some(BuiltinSpec {
            name: "kill",
            kind: BuiltinKind::Kill,
        }),
        _ => None,
    }
}

pub fn dispatch<R: io::Read, W: io::Write, WE: io::Write>(
    builtin: BuiltinKind,
    eval: &mut Evaluator,
    ctx: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    match builtin {
        BuiltinKind::Exit => exit::exit(eval, ctx, args),
        BuiltinKind::Source => source::source(eval, ctx, args),
        BuiltinKind::Exec => exec::exec(eval, ctx, args),
        BuiltinKind::Alias => alias::alias(eval, ctx, args),
        BuiltinKind::Cd => cd::cd(eval, ctx, args),
        BuiltinKind::FmtCwd => fmt_cwd::fmt_cwd(eval, ctx, args),
        BuiltinKind::Complete => complete::complete(eval, ctx, args),
        BuiltinKind::Plugin => plugin::plugin(eval, ctx, args),
        BuiltinKind::Export => export::export(eval, ctx, args),
        BuiltinKind::Jobs => jobs::jobs(eval, ctx, args),
        BuiltinKind::Fg => fg::fg(eval, ctx, args),
        BuiltinKind::Bg => bg::bg(eval, ctx, args),
        BuiltinKind::Wait => wait::wait(eval, ctx, args),
        BuiltinKind::Disown => disown::disown(eval, ctx, args),
        BuiltinKind::Kill => kill::kill(eval, ctx, args),
    }
}
