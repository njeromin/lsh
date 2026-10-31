use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_kill_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut kill_rule = CompletionRule::new();
    kill_rule.candidates = vec![
        CandidateSpec::from_raw("-9:Kill signal (SIGKILL)", CompletionKind::Flag),
        CandidateSpec::from_raw("-15:Termination signal (SIGTERM)", CompletionKind::Flag),
        CandidateSpec::from_raw("-2:Interrupt signal (SIGINT)", CompletionKind::Flag),
        CandidateSpec::from_raw("-1:Hangup signal (SIGHUP)", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-STOP:Pause process execution (SIGSTOP)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-CONT:Resume paused process (SIGCONT)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-l:List all signal names", CompletionKind::Flag),
    ];
    kill_rule.exec =
        Some("ps -u \"$USER\" -o pid=,comm= 2>/dev/null | awk '{print $1 \":\" $2}'".to_string());
    rules.insert("kill".to_string(), kill_rule);
}

pub use populate_kill_rules as populate_rules;
