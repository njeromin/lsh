use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_cargo_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut cargo_rule = CompletionRule::new();
    cargo_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--version:Print version info and exit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--help:Print this message", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
    ];

    let mut build_rule = CompletionRule::new();
    build_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--release:Build artifacts in release mode",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--bin:Build only the specified binary",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--target:Build for the target triple", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("build".to_string(), build_rule);

    let mut run_rule = CompletionRule::new();
    run_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--release:Build and run in release mode",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--bin:Name of the bin target to run", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--example:Name of the example target to run",
            CompletionKind::Flag,
        ),
    ];
    cargo_rule.subcommands.insert("run".to_string(), run_rule);

    let mut test_rule = CompletionRule::new();
    test_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--release:Build and run in release mode",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--test:Test only the specified test target",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--doc:Test only this library's documentation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--nocapture:Do not capture stdout/stderr",
            CompletionKind::Flag,
        ),
    ];
    cargo_rule.subcommands.insert("test".to_string(), test_rule);

    let mut check_rule = CompletionRule::new();
    check_rule.candidates = vec![
        CandidateSpec::from_raw("--all-targets:Check all targets", CompletionKind::Flag),
        CandidateSpec::from_raw("--release:Check in release mode", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("check".to_string(), check_rule);

    let mut clippy_rule = CompletionRule::new();
    clippy_rule.candidates = vec![
        CandidateSpec::from_raw("--all-targets:Check all targets", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--fix:Automatically apply lint suggestions",
            CompletionKind::Flag,
        ),
    ];
    cargo_rule
        .subcommands
        .insert("clippy".to_string(), clippy_rule);

    let clean_rule = CompletionRule::new();
    cargo_rule
        .subcommands
        .insert("clean".to_string(), clean_rule);

    let mut new_rule = CompletionRule::new();
    new_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--bin:Use a binary (application) template",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--lib:Use a library template", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("new".to_string(), new_rule);

    let mut init_rule = CompletionRule::new();
    init_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--bin:Use a binary (application) template",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--lib:Use a library template", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("init".to_string(), init_rule);

    let mut doc_rule = CompletionRule::new();
    doc_rule.candidates = vec![
        CandidateSpec::from_raw("--open:Opens the docs in a browser", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--no-deps:Do not build documentation for dependencies",
            CompletionKind::Flag,
        ),
    ];
    cargo_rule.subcommands.insert("doc".to_string(), doc_rule);

    let update_rule = CompletionRule::new();
    cargo_rule
        .subcommands
        .insert("update".to_string(), update_rule);

    let metadata_rule = CompletionRule::new();
    cargo_rule
        .subcommands
        .insert("metadata".to_string(), metadata_rule);

    let tree_rule = CompletionRule::new();
    cargo_rule.subcommands.insert("tree".to_string(), tree_rule);

    rules.insert("cargo".to_string(), cargo_rule);
}

pub use populate_cargo_rules as populate_rules;
