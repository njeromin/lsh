use std::process::Command;

fn spawn_proc(cmd: &str, args: Vec<&str>) {
    Command::new(cmd).args(args).spawn();
}
