use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_builtin_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut alias_rule = CompletionRule::new();
    alias_rule.candidates = vec![
        CandidateSpec::from_raw("set:Define or update an alias", CompletionKind::Subcommand),
        CandidateSpec::from_raw("list:List defined aliases", CompletionKind::Subcommand),
    ];
    rules.insert("alias".to_string(), alias_rule);

    let mut complete_rule = CompletionRule::new();
    complete_rule.candidates = vec![
        CandidateSpec::from_raw(
            "set:Define completion candidates for a command",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "list:List defined command completions",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "remove:Remove completion for a command",
            CompletionKind::Subcommand,
        ),
    ];
    rules.insert("complete".to_string(), complete_rule);

    let mut cd_rule = CompletionRule::new();
    cd_rule.dirs_only = true;
    rules.insert("cd".to_string(), cd_rule);

    let mut pushd_rule = CompletionRule::new();
    pushd_rule.dirs_only = true;
    rules.insert("pushd".to_string(), pushd_rule);
}

pub use populate_builtin_rules as populate_rules;
