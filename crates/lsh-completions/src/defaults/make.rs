use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_make_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut make_rule = CompletionRule::new();
    make_rule.candidates = vec![
        CandidateSpec::from_raw("-j:Number of simultaneous jobs", CompletionKind::Flag),
        CandidateSpec::from_raw("-f:Use specified file as a makefile", CompletionKind::Flag),
        CandidateSpec::from_raw("-B:Unconditionally make all targets", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-k:Continue as much as possible after an error",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-n:Dry run (print commands without running)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-s:Silent mode (do not echo commands)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-C:Change directory before executing make",
            CompletionKind::Flag,
        ),
    ];
    make_rule.exec = Some("awk -F':' '/^[a-zA-Z0-9_-]+[[:space:]]*:/ && $1 !~ /^\\./ {sub(/[[:space:]]+$/, \"\", $1); print $1\":Makefile target\"}' Makefile makefile GNUmakefile 2>/dev/null | sort -u".to_string());
    rules.insert("make".to_string(), make_rule);
}

pub use populate_make_rules as populate_rules;
