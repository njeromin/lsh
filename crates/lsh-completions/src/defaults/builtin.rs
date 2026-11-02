use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_builtin_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut alias_rule = CompletionRule::new();
    alias_rule.subcommands.insert(
        "set".to_string(),
        CompletionRule::new().with_description("Define or update an alias"),
    );
    alias_rule.subcommands.insert(
        "list".to_string(),
        CompletionRule::new().with_description("List defined aliases"),
    );
    rules.insert("alias".to_string(), alias_rule);

    let mut complete_rule = CompletionRule::new();
    let mut complete_set =
        CompletionRule::new().with_description("Define completion candidates for a command");
    complete_set.candidates = vec![
        CandidateSpec::from_raw("--dirs:Complete directories only", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--exec:Run a shell command to generate dynamic completion items",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--ext:Filter path completion by file extension",
            CompletionKind::Flag,
        ),
    ];
    complete_rule
        .subcommands
        .insert("set".to_string(), complete_set);
    complete_rule.subcommands.insert(
        "list".to_string(),
        CompletionRule::new().with_description("List defined command completions"),
    );
    complete_rule.subcommands.insert(
        "remove".to_string(),
        CompletionRule::new().with_description("Remove completion for a command"),
    );
    rules.insert("complete".to_string(), complete_rule);

    let mut cd_rule = CompletionRule::new();
    cd_rule.dirs_only = true;
    cd_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-L:Force symbolic links to be followed",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-P:Use physical directory structure without following symlinks",
            CompletionKind::Flag,
        ),
    ];
    rules.insert("cd".to_string(), cd_rule);

    let mut pushd_rule = CompletionRule::new();
    pushd_rule.dirs_only = true;
    rules.insert("pushd".to_string(), pushd_rule);

    let mut plugin_rule = CompletionRule::new();
    plugin_rule.subcommands.insert(
        "list".to_string(),
        CompletionRule::new().with_description("List all loaded plugins"),
    );
    plugin_rule.subcommands.insert(
        "info".to_string(),
        CompletionRule::new().with_description("Display details for a loaded plugin"),
    );
    plugin_rule.subcommands.insert(
        "load".to_string(),
        CompletionRule::new().with_description("Load a plugin from directory or path"),
    );
    plugin_rule.subcommands.insert(
        "unload".to_string(),
        CompletionRule::new().with_description("Unload a plugin by name"),
    );
    rules.insert("plugin".to_string(), plugin_rule);

    let mut source_rule = CompletionRule::new();
    source_rule.file_extensions = vec!["lsh".to_string(), "sh".to_string()];
    rules.insert("source".to_string(), source_rule);

    let mut exit_rule = CompletionRule::new();
    exit_rule.candidates = vec![CandidateSpec::from_raw(
        "--help:Exit the shell with optional status code",
        CompletionKind::Flag,
    )];
    rules.insert("exit".to_string(), exit_rule);
}

pub use populate_builtin_rules as populate_rules;
