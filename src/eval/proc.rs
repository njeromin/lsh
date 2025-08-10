use std::process::Command;

pub fn spawn_proc(cmd: &str, args: Vec<String>) {
    Command::new(cmd)
        .args(args)
        .spawn()
        .expect("Ts not working")
        .wait()
        .expect("Ts not working");
}
