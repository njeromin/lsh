pub mod alias;
pub mod cd;
pub mod exec;
pub mod exit;
pub mod fmt_cwd;
pub mod source;

pub fn is_valid_builtin(cmd: &str) -> bool {
    match cmd {
        "exit" => true,
        "source" => true,
        "exec" => true,
        "alias" => true,
        "cd" => true,
        "fmt-cwd" => true,
        _ => false,
    }
}
