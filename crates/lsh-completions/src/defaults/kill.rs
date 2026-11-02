use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_kill_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut kill_rule = CompletionRule::new();
    kill_rule.candidates = vec![
        CandidateSpec::from_raw("-9:Kill signal (SIGKILL)", CompletionKind::Flag),
        CandidateSpec::from_raw("-KILL:Kill signal (SIGKILL)", CompletionKind::Flag),
        CandidateSpec::from_raw("-15:Termination signal (SIGTERM)", CompletionKind::Flag),
        CandidateSpec::from_raw("-TERM:Termination signal (SIGTERM)", CompletionKind::Flag),
        CandidateSpec::from_raw("-2:Interrupt signal (SIGINT)", CompletionKind::Flag),
        CandidateSpec::from_raw("-INT:Interrupt signal (SIGINT)", CompletionKind::Flag),
        CandidateSpec::from_raw("-1:Hangup signal (SIGHUP)", CompletionKind::Flag),
        CandidateSpec::from_raw("-HUP:Hangup signal (SIGHUP)", CompletionKind::Flag),
        CandidateSpec::from_raw("-3:Quit signal (SIGQUIT)", CompletionKind::Flag),
        CandidateSpec::from_raw("-QUIT:Quit signal (SIGQUIT)", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-STOP:Pause process execution (SIGSTOP)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-19:Pause process execution (SIGSTOP)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-CONT:Resume paused process (SIGCONT)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-18:Resume paused process (SIGCONT)", CompletionKind::Flag),
        CandidateSpec::from_raw("-TSTP:Terminal stop (SIGTSTP)", CompletionKind::Flag),
        CandidateSpec::from_raw("-20:Terminal stop (SIGTSTP)", CompletionKind::Flag),
        CandidateSpec::from_raw("-USR1:User defined signal 1", CompletionKind::Flag),
        CandidateSpec::from_raw("-10:User defined signal 1 (SIGUSR1)", CompletionKind::Flag),
        CandidateSpec::from_raw("-USR2:User defined signal 2", CompletionKind::Flag),
        CandidateSpec::from_raw("-12:User defined signal 2 (SIGUSR2)", CompletionKind::Flag),
        CandidateSpec::from_raw("-PIPE:Broken pipe (SIGPIPE)", CompletionKind::Flag),
        CandidateSpec::from_raw("-13:Broken pipe (SIGPIPE)", CompletionKind::Flag),
        CandidateSpec::from_raw("-ALRM:Alarm clock (SIGALRM)", CompletionKind::Flag),
        CandidateSpec::from_raw("-14:Alarm clock (SIGALRM)", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-CHLD:Child status has changed (SIGCHLD)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-17:Child status has changed (SIGCHLD)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-0:Check if process exists (signal 0)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-l:List all signal names", CompletionKind::Flag),
        CandidateSpec::from_raw("-s:Specify signal to be sent", CompletionKind::Flag),
    ];
    kill_rule.exec = Some(
        "ps -u \"$USER\" -o pid=,comm= 2>/dev/null | awk '{pid=$1; $1=\"\"; sub(/^[[:space:]]+/, \"\"); print pid \":\" $0}'"
            .to_string(),
    );
    rules.insert("kill".to_string(), kill_rule);
}

pub use populate_kill_rules as populate_rules;
