use std::collections::HashMap;

use crate::terminal::completion::types::{CandidateSpec, CompletionRule};
use crate::terminal::menu::CompletionKind;

pub fn populate_default_rules(rules: &mut HashMap<String, CompletionRule>) {
    populate_git_rules(rules);
    populate_cargo_rules(rules);
    populate_builtin_rules(rules);
    populate_kill_rules(rules);
    populate_node_rules(rules);
    populate_ssh_rules(rules);
    populate_make_rules(rules);
}

fn populate_git_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut git_rule = CompletionRule::new();
    git_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--version:Display version information",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--help:Display help information", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-C:Run as if git was started in given directory",
            CompletionKind::Flag,
        ),
    ];

    let mut add_rule = CompletionRule::new();
    add_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-A:Add changes from all tracked and untracked files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-p:Interactively choose hunks of patch between the index and the work tree",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-u:Update the index just where it already has an entry matching <pathspec>",
            CompletionKind::Flag,
        ),
    ];
    git_rule.subcommands.insert("add".to_string(), add_rule);

    let mut commit_rule = CompletionRule::new();
    commit_rule.candidates = vec![
        CandidateSpec::from_raw("-m:Commit message", CompletionKind::Flag),
        CandidateSpec::from_raw("--amend:Amend previous commit", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-a:Automatically stage modified/deleted files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-s:Add Signed-off-by trailer", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--no-verify:Bypass pre-commit and commit-msg hooks",
            CompletionKind::Flag,
        ),
    ];
    git_rule
        .subcommands
        .insert("commit".to_string(), commit_rule);

    let mut status_rule = CompletionRule::new();
    status_rule.candidates = vec![
        CandidateSpec::from_raw("-s:Give output in short-format", CompletionKind::Flag),
        CandidateSpec::from_raw("--short:Give output in short-format", CompletionKind::Flag),
        CandidateSpec::from_raw("-b:Show branch information", CompletionKind::Flag),
        CandidateSpec::from_raw("--branch:Show branch information", CompletionKind::Flag),
    ];
    git_rule
        .subcommands
        .insert("status".to_string(), status_rule);

    let mut checkout_rule = CompletionRule::new();
    checkout_rule.candidates = vec![
        CandidateSpec::from_raw("-b:Create and checkout a new branch", CompletionKind::Flag),
        CandidateSpec::from_raw("--detach:Detach HEAD at named commit", CompletionKind::Flag),
    ];
    checkout_rule.exec = Some("git branch --format='%(refname:short)'".to_string());
    git_rule
        .subcommands
        .insert("checkout".to_string(), checkout_rule);

    let mut switch_rule = CompletionRule::new();
    switch_rule.candidates = vec![
        CandidateSpec::from_raw("-c:Create and switch to a new branch", CompletionKind::Flag),
        CandidateSpec::from_raw("-d:Detach HEAD", CompletionKind::Flag),
    ];
    switch_rule.exec = Some("git branch --format='%(refname:short)'".to_string());
    git_rule
        .subcommands
        .insert("switch".to_string(), switch_rule);

    let mut branch_rule = CompletionRule::new();
    branch_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-a:List both remote-tracking and local branches",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-d:Delete a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("-D:Force delete a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("-m:Move/rename a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Show sha1 and commit subject line", CompletionKind::Flag),
    ];
    git_rule
        .subcommands
        .insert("branch".to_string(), branch_rule);

    let mut diff_rule = CompletionRule::new();
    diff_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--staged:View changes staged for the next commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--cached:Synonym for --staged", CompletionKind::Flag),
        CandidateSpec::from_raw("--stat:Generate a diffstat", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("diff".to_string(), diff_rule);

    let mut log_rule = CompletionRule::new();
    log_rule.candidates = vec![
        CandidateSpec::from_raw("-n:Limit number of commits", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--oneline:Shorthand for --pretty=oneline --abbrev-commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--graph:Draw a text-based graphical representation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Generate patch", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("log".to_string(), log_rule);

    let mut push_rule = CompletionRule::new();
    push_rule.candidates = vec![
        CandidateSpec::from_raw("-u:Set upstream branch", CompletionKind::Flag),
        CandidateSpec::from_raw("--force:Force updates", CompletionKind::Flag),
        CandidateSpec::from_raw("--tags:Push all tags", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("push".to_string(), push_rule);

    let mut pull_rule = CompletionRule::new();
    pull_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--rebase:Rebase current branch on top of upstream",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--no-rebase:Do not rebase", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("pull".to_string(), pull_rule);

    let mut fetch_rule = CompletionRule::new();
    fetch_rule.candidates = vec![
        CandidateSpec::from_raw("--all:Fetch all remotes", CompletionKind::Flag),
        CandidateSpec::from_raw("-p:Prune tracking branches", CompletionKind::Flag),
        CandidateSpec::from_raw("--prune:Prune tracking branches", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("fetch".to_string(), fetch_rule);

    let clone_rule = CompletionRule::new();
    git_rule.subcommands.insert("clone".to_string(), clone_rule);

    let mut merge_rule = CompletionRule::new();
    merge_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--abort:Abort the current conflict resolution",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--continue:Continue after resolving conflicts",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-ff:Create a merge commit even when fast-forward is possible",
            CompletionKind::Flag,
        ),
    ];
    git_rule.subcommands.insert("merge".to_string(), merge_rule);

    let mut rebase_rule = CompletionRule::new();
    rebase_rule.candidates = vec![
        CandidateSpec::from_raw("-i:Interactive rebase", CompletionKind::Flag),
        CandidateSpec::from_raw("--abort:Abort the rebase operation", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--continue:Continue the rebase operation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--skip:Skip the current patch", CompletionKind::Flag),
    ];
    git_rule
        .subcommands
        .insert("rebase".to_string(), rebase_rule);

    let mut reset_rule = CompletionRule::new();
    reset_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--soft:Do not touch the index file or the working tree",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--mixed:Reset the index but not the working tree",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--hard:Reset the index and working tree",
            CompletionKind::Flag,
        ),
    ];
    git_rule.subcommands.insert("reset".to_string(), reset_rule);

    let mut stash_rule = CompletionRule::new();
    stash_rule.candidates = vec![
        CandidateSpec::from_raw(
            "pop:Apply and remove single stashed state",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "apply:Apply single stashed state",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw("list:List stashed states", CompletionKind::Subcommand),
        CandidateSpec::from_raw(
            "drop:Remove single stashed state",
            CompletionKind::Subcommand,
        ),
        CandidateSpec::from_raw(
            "clear:Remove all stashed states",
            CompletionKind::Subcommand,
        ),
    ];
    git_rule.subcommands.insert("stash".to_string(), stash_rule);

    let mut tag_rule = CompletionRule::new();
    tag_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-a:Make an unsigned, annotated tag object",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-d:Delete tags with given names", CompletionKind::Flag),
        CandidateSpec::from_raw("-l:List tags", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("tag".to_string(), tag_rule);

    let mut remote_rule = CompletionRule::new();
    remote_rule.candidates = vec![
        CandidateSpec::from_raw("add:Add a new remote", CompletionKind::Subcommand),
        CandidateSpec::from_raw("remove:Remove a remote", CompletionKind::Subcommand),
        CandidateSpec::from_raw("-v:Be a little more verbose", CompletionKind::Flag),
    ];
    git_rule
        .subcommands
        .insert("remote".to_string(), remote_rule);

    let mut restore_rule = CompletionRule::new();
    restore_rule.candidates = vec![
        CandidateSpec::from_raw("--staged:Restore the index", CompletionKind::Flag),
        CandidateSpec::from_raw("--worktree:Restore the working tree", CompletionKind::Flag),
    ];
    git_rule
        .subcommands
        .insert("restore".to_string(), restore_rule);

    rules.insert("git".to_string(), git_rule);
}

fn populate_cargo_rules(rules: &mut HashMap<String, CompletionRule>) {
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

fn populate_builtin_rules(rules: &mut HashMap<String, CompletionRule>) {
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

fn populate_kill_rules(rules: &mut HashMap<String, CompletionRule>) {
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

const PKG_SCRIPTS_CMD: &str = "node -e \"const p=require('./package.json');Object.entries(p.scripts||{}).forEach(([k,v])=>console.log(k+':'+v.slice(0,50)))\" 2>/dev/null || awk -F'\"' '/\"scripts\"[[:space:]]*:[[:space:]]*\\{/,/\\}/ {if ($2 != \"\" && $2 != \"scripts\") print $2}' package.json 2>/dev/null";

fn populate_node_rules(rules: &mut HashMap<String, CompletionRule>) {
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
        CandidateSpec::from_raw("init:Create a package.json file", CompletionKind::Subcommand),
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
        CandidateSpec::from_raw("init:Create a package.json file", CompletionKind::Subcommand),
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

fn populate_ssh_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut ssh_rule = CompletionRule::new();
    ssh_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-p:Port to connect to on the remote host",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-i:Select identity (private key) file",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Verbose mode (debug connection)", CompletionKind::Flag),
        CandidateSpec::from_raw("-X:Enable X11 forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw("-Y:Enable trusted X11 forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw("-N:Do not execute a remote command", CompletionKind::Flag),
        CandidateSpec::from_raw("-L:Local port forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw("-R:Remote port forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-D:Dynamic application-level port forwarding",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-C:Enable compression", CompletionKind::Flag),
    ];
    ssh_rule.exec = Some("sh -c \"{ awk '/^[[:space:]]*Host[[:space:]]+/ {for (i=2; i<=NF; i++) if ($i !~ /[*?]/) print $i\\\":SSH Config Host\\\"}' ~/.ssh/config 2>/dev/null; awk '{split($1, a, \\\",\\\"); for (i in a) if (a[i] !~ /^\\\\|/ && a[i] !~ /[*?]/) print a[i]\\\":Known Host\\\"}' ~/.ssh/known_hosts 2>/dev/null; } | awk -F: '!seen[$1]++'\"".to_string());
    rules.insert("ssh".to_string(), ssh_rule);
}

fn populate_make_rules(rules: &mut HashMap<String, CompletionRule>) {
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
        CandidateSpec::from_raw("-s:Silent mode (do not echo commands)", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-C:Change directory before executing make",
            CompletionKind::Flag,
        ),
    ];
    make_rule.exec = Some("awk -F':' '/^[a-zA-Z0-9_-]+[[:space:]]*:/ && $1 !~ /^\\./ {sub(/[[:space:]]+$/, \"\", $1); print $1\":Makefile target\"}' Makefile makefile GNUmakefile 2>/dev/null | sort -u".to_string());
    rules.insert("make".to_string(), make_rule);
}
