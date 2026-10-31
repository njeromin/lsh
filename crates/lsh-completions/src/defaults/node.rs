use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub const PKG_SCRIPTS_CMD: &str = "node -e \"const p=require('./package.json');Object.entries(p.scripts||{}).forEach(([k,v])=>console.log(k+':'+v.slice(0,50)))\" 2>/dev/null || awk -F'\"' '/\"scripts\"[[:space:]]*:[[:space:]]*\\{/,/\\}/ {if ($2 != \"\" && $2 != \"scripts\") print $2}' package.json 2>/dev/null";

pub fn populate_node_rules(rules: &mut HashMap<String, CompletionRule>) {
    // 1. npm
    let mut npm_rule = CompletionRule::new();
    npm_rule.candidates = vec![
        CandidateSpec::from_raw(
            "run:Run arbitrary package scripts",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("test:Run package test script", CompletionKind::Subcommand),
        CandidateSpec::from_raw("start:Start package", CompletionKind::Subcommand),
        CandidateSpec::from_raw("build:Run build script", CompletionKind::Subcommand),
        CandidateSpec::from_raw("install:Install dependencies", CompletionKind::Subcommand),
        CandidateSpec::from_raw("i:Install dependencies", CompletionKind::Subcommand),
        CandidateSpec::from_raw("add:Add a package", CompletionKind::Subcommand),
        CandidateSpec::from_raw(
            "publish:Publish package to registry",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "init:Create a package.json file",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "outdated:Check for outdated packages",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("update:Update packages", CompletionKind::Subcommand),
        CandidateSpec::from_raw("audit:Run security audit", CompletionKind::Subcommand),
        CandidateSpec::from_raw("-v:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--version:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help", CompletionKind::Flag),
    ];
    let mut npm_run = CompletionRule::new();
    npm_run.exec = Some(PKG_SCRIPTS_CMD.to_string());
    npm_rule.subcommands.insert("run".to_string(), npm_run);
    rules.insert("npm".to_string(), npm_rule);

    // 2. yarn
    let mut yarn_rule = CompletionRule::new();
    yarn_rule.candidates = vec![
        CandidateSpec::from_raw(
            "add:Install a package and dependencies",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "remove:Remove package from dependencies",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "install:Install all dependencies",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "run:Run a defined package script",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("build:Run package build script", CompletionKind::Subcommand),
        CandidateSpec::from_raw("test:Run package test script", CompletionKind::Subcommand),
        CandidateSpec::from_raw("start:Run package start script", CompletionKind::Subcommand),
        CandidateSpec::from_raw("init:Create a new package", CompletionKind::Subcommand),
        CandidateSpec::from_raw(
            "publish:Publish package to npm registry",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "upgrade:Upgrade packages to latest version",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "why:Show why a package is installed",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("-v:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--version:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help", CompletionKind::Flag),
    ];
    yarn_rule.exec = Some(PKG_SCRIPTS_CMD.to_string());
    let mut yarn_run = CompletionRule::new();
    yarn_run.exec = Some(PKG_SCRIPTS_CMD.to_string());
    yarn_rule.subcommands.insert("run".to_string(), yarn_run);
    rules.insert("yarn".to_string(), yarn_rule);

    // 3. pnpm
    let mut pnpm_rule = CompletionRule::new();
    pnpm_rule.candidates = vec![
        CandidateSpec::from_raw(
            "add:Install a package and dependencies",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "install:Install all dependencies",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("i:Install all dependencies", CompletionKind::Subcommand),
        CandidateSpec::from_raw(
            "remove:Remove packages from package.json",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "run:Run a defined package script",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("test:Run package test script", CompletionKind::Subcommand),
        CandidateSpec::from_raw("start:Run package start script", CompletionKind::Subcommand),
        CandidateSpec::from_raw("build:Run package build script", CompletionKind::Subcommand),
        CandidateSpec::from_raw(
            "init:Create a package.json file",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "publish:Publish package to registry",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "update:Update packages to latest version",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("-v:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--version:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help", CompletionKind::Flag),
    ];
    pnpm_rule.exec = Some(PKG_SCRIPTS_CMD.to_string());
    let mut pnpm_run = CompletionRule::new();
    pnpm_run.exec = Some(PKG_SCRIPTS_CMD.to_string());
    pnpm_rule.subcommands.insert("run".to_string(), pnpm_run);
    rules.insert("pnpm".to_string(), pnpm_rule);
}

pub use populate_node_rules as populate_rules;
