pub mod alias;
pub mod cd;
pub mod exec;
pub mod exit;
pub mod fmt_cwd;
pub mod source;

pub fn is_valid_builtin(cmd: &str) -> bool {
    matches!(cmd, "exit" | "source" | "exec" | "alias" | "cd" | "fmt-cwd")
}
