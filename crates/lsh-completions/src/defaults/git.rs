use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_git_rules(rules: &mut HashMap<String, CompletionRule>) {
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

pub use populate_git_rules as populate_rules;
