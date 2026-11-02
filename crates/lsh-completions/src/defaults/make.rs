use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_make_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut make_rule = CompletionRule::new();
    make_rule.candidates = vec![
        CandidateSpec::from_raw("-j:Number of simultaneous jobs", CompletionKind::Flag),
        CandidateSpec::from_raw("--jobs:Number of simultaneous jobs", CompletionKind::Flag),
        CandidateSpec::from_raw("-f:Use specified file as a makefile", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--file:Use specified file as a makefile",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-B:Unconditionally make all targets", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--always-make:Unconditionally make all targets",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-k:Continue as much as possible after an error",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--keep-going:Continue as much as possible after an error",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-n:Dry run (print commands without running)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--dry-run:Dry run (print commands without running)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-s:Silent mode (do not echo commands)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--silent:Silent mode (do not echo commands)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--quiet:Silent mode (do not echo commands)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-C:Change directory before executing make",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--directory:Change directory before executing make",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-i:Ignore all errors in recipes", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--ignore-errors:Ignore all errors in recipes",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-q:Question mode (do not run commands; return 0 if up to date)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--question:Question mode (return 0 if up to date)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-r:Eliminate use of built-in implicit rules",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-builtin-rules:Eliminate use of built-in implicit rules",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-w:Print directory before and after other processing",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--print-directory:Print directory before and after other processing",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-print-directory:Turn off -w, even if it was turned on implicitly",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Print make version and exit", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--version:Print make version and exit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print make help and exit", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print make help and exit", CompletionKind::Flag),
    ];
    make_rule.exec = Some("awk -F':' '/^[a-zA-Z0-9_.-]+[[:space:]]*:/ && $1 !~ /^\\./ && $1 !~ /=/ {sub(/[[:space:]]+$/, \"\", $1); print $1\":Makefile target\"}' [mM]akefile GNUmakefile 2>/dev/null | sort -u".to_string());
    rules.insert("make".to_string(), make_rule);
}

pub use populate_make_rules as populate_rules;
