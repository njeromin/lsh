use std::io;

use crate::eval::Evaluator;

pub mod alias;
pub mod cd;
pub mod exec;
pub mod exit;
pub mod fmt_cwd;
pub mod source;

// pub type BuiltinFn<R, W, WE> = fn(&mut Evaluator, &mut lsh::IoCtx<R, W, WE>, Vec<String>);

pub enum BuiltinKind {
    Exit,
    Source,
    Exec,
    Alias,
    Cd,
    FmtCwd,
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
        _ => None,
    }
}

pub fn dispatch<R: io::Read, W: io::Write, WE: io::Write>(
    builtin: BuiltinKind,
    eval: &mut Evaluator,
    ctx: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    match builtin {
        BuiltinKind::Exit => exit::exit(eval, ctx, args),
        BuiltinKind::Source => source::source(eval, ctx, args),
        BuiltinKind::Exec => exec::exec(eval, ctx, args),
        BuiltinKind::Alias => alias::alias(eval, ctx, args),
        BuiltinKind::Cd => cd::cd(eval, ctx, args),
        BuiltinKind::FmtCwd => fmt_cwd::fmt_cwd(eval, ctx, args),
    }
}
