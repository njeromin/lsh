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
        CandidateSpec::from_raw(
            "-c:Pass a configuration parameter to the command",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Pipe all output into less", CompletionKind::Flag),
        CandidateSpec::from_raw("--paginate:Pipe all output into less", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--no-pager:Do not pipe git output into a pager",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--bare:Treat the repository as a bare repository",
            CompletionKind::Flag,
        ),
    ];

    let branch_exec = "git branch -a --format='%(refname:short)' 2>/dev/null";
    let local_branch_exec = "git branch --format='%(refname:short)' 2>/dev/null";
    let remote_exec = "git remote 2>/dev/null | awk '{print $1 \":Remote repository\"}'";
    let stash_exec = "git stash list --format='%gd:%gs' 2>/dev/null";
    let tag_exec = "git tag -l 2>/dev/null | awk '{print $1 \":Git tag\"}'";

    // 1. add
    let mut add_rule = CompletionRule::new().with_description("Add file contents to the index");
    add_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-A:Add changes from all tracked and untracked files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all:Add changes from all tracked and untracked files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-p:Interactively choose hunks of patch between index and worktree",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--patch:Interactively choose hunks of patch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-u:Update the index where it already has an entry",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--update:Update the index where it already has an entry",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-f:Allow adding otherwise ignored files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--force:Allow adding otherwise ignored files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-n:Dry run (do not actually add files)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--dry-run:Dry run (do not actually add files)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-e:Open diff vs index in an editor and let user edit it",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-N:Record only the fact that the path will be added later",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--intent-to-add:Record only the fact that the path will be added later",
            CompletionKind::Flag,
        ),
    ];
    git_rule.subcommands.insert("add".to_string(), add_rule);

    // 2. commit
    let mut commit_rule =
        CompletionRule::new().with_description("Record changes to the repository");
    commit_rule.candidates = vec![
        CandidateSpec::from_raw("-m:Commit message", CompletionKind::Flag),
        CandidateSpec::from_raw("--message:Commit message", CompletionKind::Flag),
        CandidateSpec::from_raw("--amend:Amend previous commit", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-a:Automatically stage modified/deleted files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all:Automatically stage modified/deleted files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-s:Add Signed-off-by trailer", CompletionKind::Flag),
        CandidateSpec::from_raw("--signoff:Add Signed-off-by trailer", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--no-verify:Bypass pre-commit and commit-msg hooks",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-n:Bypass pre-commit and commit-msg hooks",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--allow-empty:Allow recording an empty commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--reset-author:Declare that the author is now the committer",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--fixup:Construct a fixup commit for use with rebase --autosquash",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--squash:Construct a squash commit for use with rebase --autosquash",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-v:Show unified diff of all changes to be committed",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--verbose:Show unified diff of all changes to be committed",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-q:Suppress commit summary message", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--quiet:Suppress commit summary message",
            CompletionKind::Flag,
        ),
    ];
    git_rule
        .subcommands
        .insert("commit".to_string(), commit_rule);

    // 3. status
    let mut status_rule = CompletionRule::new().with_description("Show the working tree status");
    status_rule.candidates = vec![
        CandidateSpec::from_raw("-s:Give output in short-format", CompletionKind::Flag),
        CandidateSpec::from_raw("--short:Give output in short-format", CompletionKind::Flag),
        CandidateSpec::from_raw("-b:Show branch information", CompletionKind::Flag),
        CandidateSpec::from_raw("--branch:Show branch information", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-u:Show untracked files (no, normal, all)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--untracked-files:Show untracked files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--ignored:Show ignored files as well", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--porcelain:Give output in an easy-to-parse format",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
    ];
    git_rule
        .subcommands
        .insert("status".to_string(), status_rule);

    // 4. checkout
    let mut checkout_rule =
        CompletionRule::new().with_description("Switch branches or restore working tree files");
    checkout_rule.candidates = vec![
        CandidateSpec::from_raw("-b:Create and checkout a new branch", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-B:Create/reset and checkout a branch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--detach:Detach HEAD at named commit", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-f:Force checkout (throw away local modifications)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--force:Force checkout", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-t:Set up tracking mode while creating a branch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--track:Set up tracking mode", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-p:Interactively select hunks in difference",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--patch:Interactively select hunks in difference",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--ours:Checkout stage #2 for unmerged paths",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--theirs:Checkout stage #3 for unmerged paths",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-q:Suppress feedback messages", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Suppress feedback messages", CompletionKind::Flag),
    ];
    checkout_rule.exec = Some(branch_exec.to_string());
    git_rule
        .subcommands
        .insert("checkout".to_string(), checkout_rule);

    // 5. switch
    let mut switch_rule = CompletionRule::new().with_description("Switch branches");
    switch_rule.candidates = vec![
        CandidateSpec::from_raw("-c:Create and switch to a new branch", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-C:Create/reset and switch to a new branch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-d:Detach HEAD", CompletionKind::Flag),
        CandidateSpec::from_raw("--detach:Detach HEAD", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-t:Set upstream tracking for new branch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--track:Set upstream tracking", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-f:Proceed even if index or working tree differs",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--force:Proceed even if index differs",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-q:Suppress feedback messages", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Suppress feedback messages", CompletionKind::Flag),
    ];
    switch_rule.exec = Some(local_branch_exec.to_string());
    git_rule
        .subcommands
        .insert("switch".to_string(), switch_rule);

    // 6. branch
    let mut branch_rule =
        CompletionRule::new().with_description("List, create, or delete branches");
    branch_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-a:List both remote-tracking and local branches",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all:List both remote-tracking and local branches",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-r:List remote-tracking branches", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--remotes:List remote-tracking branches",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-d:Delete a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("--delete:Delete a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("-D:Force delete a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("-m:Move/rename a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("--move:Move/rename a branch", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-M:Move/rename a branch, even if target exists",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-c:Copy a branch", CompletionKind::Flag),
        CandidateSpec::from_raw("--copy:Copy a branch", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-C:Copy a branch, even if target exists",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Show sha1 and commit subject line", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--verbose:Show sha1 and commit subject line",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-l:List branches", CompletionKind::Flag),
        CandidateSpec::from_raw("--list:List branches", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--show-current:Print name of current branch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--merged:List branches whose tips are reachable from HEAD",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-merged:List branches whose tips are not reachable from HEAD",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--contains:Print only branches containing the commit",
            CompletionKind::Flag,
        ),
    ];
    branch_rule.exec = Some(local_branch_exec.to_string());
    git_rule
        .subcommands
        .insert("branch".to_string(), branch_rule);

    // 7. diff
    let mut diff_rule = CompletionRule::new()
        .with_description("Show changes between commits, commit and working tree, etc.");
    diff_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--staged:View changes staged for the next commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--cached:Synonym for --staged", CompletionKind::Flag),
        CandidateSpec::from_raw("--stat:Generate a diffstat", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--name-only:Show only names of changed files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--name-status:Show only names and status of changed files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Generate patch", CompletionKind::Flag),
        CandidateSpec::from_raw("-u:Generate patch format", CompletionKind::Flag),
        CandidateSpec::from_raw("-w:Ignore all whitespace", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--ignore-all-space:Ignore all whitespace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--ignore-space-change:Ignore changes in amount of whitespace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--color-words:Show colored word diff", CompletionKind::Flag),
        CandidateSpec::from_raw("-s:Suppress diff output", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--summary:Output a condensed summary of extended header",
            CompletionKind::Flag,
        ),
    ];
    diff_rule.exec = Some(branch_exec.to_string());
    git_rule.subcommands.insert("diff".to_string(), diff_rule);

    // 8. log
    let mut log_rule = CompletionRule::new().with_description("Show commit logs");
    log_rule.candidates = vec![
        CandidateSpec::from_raw("-n:Limit number of commits", CompletionKind::Flag),
        CandidateSpec::from_raw("--max-count:Limit number of commits", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--oneline:Shorthand for --pretty=oneline --abbrev-commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--graph:Draw a text-based graphical representation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Generate patch", CompletionKind::Flag),
        CandidateSpec::from_raw("--stat:Generate a diffstat", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--all:Pretend as if all refs in refs/ are listed",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--decorate:Print out ref names of any commits shown",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--reverse:Output commits in reverse order",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--author:Limit commits to author matching pattern",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--grep:Limit commits to log message matching pattern",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--merges:Print only merge commits", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--no-merges:Do not print commits with more than one parent",
            CompletionKind::Flag,
        ),
    ];
    log_rule.exec = Some(branch_exec.to_string());
    git_rule.subcommands.insert("log".to_string(), log_rule);

    // 9. push
    let mut push_rule =
        CompletionRule::new().with_description("Update remote refs along with associated objects");
    push_rule.candidates = vec![
        CandidateSpec::from_raw("-u:Set upstream branch", CompletionKind::Flag),
        CandidateSpec::from_raw("--set-upstream:Set upstream branch", CompletionKind::Flag),
        CandidateSpec::from_raw("--force:Force updates", CompletionKind::Flag),
        CandidateSpec::from_raw("-f:Force updates", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--force-with-lease:Force update safely if remote ref matches expectation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--tags:Push all tags", CompletionKind::Flag),
        CandidateSpec::from_raw("--all:Push all branches", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--prune:Remove remote branches that do not exist locally",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-d:Delete remote branch or tag", CompletionKind::Flag),
        CandidateSpec::from_raw("--delete:Delete remote branch or tag", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--dry-run:Do everything except actually send updates",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-n:Dry run", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    push_rule.exec = Some(format!("{remote_exec}; {local_branch_exec}"));
    git_rule.subcommands.insert("push".to_string(), push_rule);

    // 10. pull
    let mut pull_rule = CompletionRule::new()
        .with_description("Fetch from and integrate with another repository or a local branch");
    pull_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--rebase:Rebase current branch on top of upstream",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--no-rebase:Do not rebase", CompletionKind::Flag),
        CandidateSpec::from_raw("--ff-only:Fast-forward only", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--no-ff:Create a merge commit even if fast-forward is possible",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--autostash:Automatically stash before rebase and pop after",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--tags:Fetch all tags", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    pull_rule.exec = Some(format!("{remote_exec}; {local_branch_exec}"));
    git_rule.subcommands.insert("pull".to_string(), pull_rule);

    // 11. fetch
    let mut fetch_rule =
        CompletionRule::new().with_description("Download objects and refs from another repository");
    fetch_rule.candidates = vec![
        CandidateSpec::from_raw("--all:Fetch all remotes", CompletionKind::Flag),
        CandidateSpec::from_raw("-p:Prune tracking branches", CompletionKind::Flag),
        CandidateSpec::from_raw("--prune:Prune tracking branches", CompletionKind::Flag),
        CandidateSpec::from_raw("--tags:Fetch all tags", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--no-tags:Disable automatic tag following",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--dry-run:Show what would be done without making changes",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    fetch_rule.exec = Some(remote_exec.to_string());
    git_rule.subcommands.insert("fetch".to_string(), fetch_rule);

    // 12. clone
    let mut clone_rule =
        CompletionRule::new().with_description("Clone a repository into a new directory");
    clone_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--depth:Create a shallow clone with limited history",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--branch:Point HEAD to specified branch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-b:Point HEAD to specified branch", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--single-branch:Clone only the history of a single branch",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--recurse-submodules:Initialize and clone submodules",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--recursive:Initialize and clone submodules",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--bare:Make a bare Git repository", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--mirror:Set up a mirror of source repository",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("clone".to_string(), clone_rule);

    // 13. merge
    let mut merge_rule =
        CompletionRule::new().with_description("Join two or more development histories together");
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
        CandidateSpec::from_raw(
            "--ff-only:Refuse to merge unless fast-forward is possible",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--squash:Produce working tree and index state as if merge happened",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-commit:Perform merge but do not autocommit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-m:Set the commit message", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    merge_rule.exec = Some(local_branch_exec.to_string());
    git_rule.subcommands.insert("merge".to_string(), merge_rule);

    // 14. rebase
    let mut rebase_rule =
        CompletionRule::new().with_description("Reapply commits on top of another base tip");
    rebase_rule.candidates = vec![
        CandidateSpec::from_raw("-i:Interactive rebase", CompletionKind::Flag),
        CandidateSpec::from_raw("--interactive:Interactive rebase", CompletionKind::Flag),
        CandidateSpec::from_raw("--abort:Abort the rebase operation", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--continue:Continue the rebase operation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--skip:Skip the current patch", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--onto:Starting point at which to create new commits",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--autosquash:Automatically apply fixup and squash commits",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--autostash:Automatically stash before rebase and pop after",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-autostash:Do not automatically stash",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    rebase_rule.exec = Some(local_branch_exec.to_string());
    git_rule
        .subcommands
        .insert("rebase".to_string(), rebase_rule);

    // 15. reset
    let mut reset_rule =
        CompletionRule::new().with_description("Reset current HEAD to the specified state");
    reset_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--soft:Do not touch index file or working tree",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--mixed:Reset index but not working tree",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--hard:Reset index and working tree", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--merge:Resets index and updates files in working tree that differ",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--keep:Resets index and updates files in working tree, keeping local changes",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-p:Interactively select hunks in difference",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--patch:Interactively select hunks in difference",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    reset_rule.exec = Some(local_branch_exec.to_string());
    git_rule.subcommands.insert("reset".to_string(), reset_rule);

    // 16. stash
    let mut stash_rule = CompletionRule::new()
        .with_description("Stash the changes in a dirty working directory away");
    stash_rule.candidates = vec![
        CandidateSpec::from_raw("-u:Include untracked files", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--include-untracked:Include untracked files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-a:Include all files (untracked and ignored)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--all:Include all files", CompletionKind::Flag),
        CandidateSpec::from_raw("-m:Stash message", CompletionKind::Flag),
        CandidateSpec::from_raw("--message:Stash message", CompletionKind::Flag),
        CandidateSpec::from_raw("-p:Interactively choose hunks", CompletionKind::Flag),
        CandidateSpec::from_raw("--patch:Interactively choose hunks", CompletionKind::Flag),
    ];

    let mut stash_pop =
        CompletionRule::new().with_description("Apply and remove single stashed state");
    stash_pop.exec = Some(stash_exec.to_string());
    stash_rule.subcommands.insert("pop".to_string(), stash_pop);

    let mut stash_apply = CompletionRule::new().with_description("Apply single stashed state");
    stash_apply.exec = Some(stash_exec.to_string());
    stash_rule
        .subcommands
        .insert("apply".to_string(), stash_apply);

    let stash_list = CompletionRule::new().with_description("List stashed states");
    stash_rule
        .subcommands
        .insert("list".to_string(), stash_list);

    let mut stash_drop = CompletionRule::new().with_description("Remove single stashed state");
    stash_drop.exec = Some(stash_exec.to_string());
    stash_rule
        .subcommands
        .insert("drop".to_string(), stash_drop);

    let stash_clear = CompletionRule::new().with_description("Remove all stashed states");
    stash_rule
        .subcommands
        .insert("clear".to_string(), stash_clear);

    let mut stash_show =
        CompletionRule::new().with_description("Show the changes recorded in the stash");
    stash_show.exec = Some(stash_exec.to_string());
    stash_rule
        .subcommands
        .insert("show".to_string(), stash_show);

    let stash_push =
        CompletionRule::new().with_description("Save local modifications to a new stash");
    stash_rule
        .subcommands
        .insert("push".to_string(), stash_push);

    let stash_create =
        CompletionRule::new().with_description("Create a stash entry without storing it");
    stash_rule
        .subcommands
        .insert("create".to_string(), stash_create);

    let stash_store =
        CompletionRule::new().with_description("Store a given stash created via stash create");
    stash_rule
        .subcommands
        .insert("store".to_string(), stash_store);

    git_rule.subcommands.insert("stash".to_string(), stash_rule);

    // 17. tag
    let mut tag_rule =
        CompletionRule::new().with_description("Create, list, delete or verify a tag object");
    tag_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-a:Make an unsigned, annotated tag object",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-s:Make a GPG-signed tag", CompletionKind::Flag),
        CandidateSpec::from_raw("-d:Delete tags with given names", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--delete:Delete tags with given names",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-l:List tags", CompletionKind::Flag),
        CandidateSpec::from_raw("--list:List tags", CompletionKind::Flag),
        CandidateSpec::from_raw("-m:Tag message", CompletionKind::Flag),
        CandidateSpec::from_raw("--message:Tag message", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-v:Verify GPG signature of given tags",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--verify:Verify GPG signature", CompletionKind::Flag),
    ];
    tag_rule.exec = Some(tag_exec.to_string());
    git_rule.subcommands.insert("tag".to_string(), tag_rule);

    // 18. remote
    let mut remote_rule =
        CompletionRule::new().with_description("Manage set of tracked repositories");
    remote_rule.candidates = vec![
        CandidateSpec::from_raw("-v:Be a little more verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be a little more verbose", CompletionKind::Flag),
    ];

    let remote_add = CompletionRule::new().with_description("Add a new remote");
    remote_rule
        .subcommands
        .insert("add".to_string(), remote_add);

    let mut remote_remove = CompletionRule::new().with_description("Remove a remote");
    remote_remove.exec = Some(remote_exec.to_string());
    remote_rule
        .subcommands
        .insert("remove".to_string(), remote_remove);

    let mut remote_rename = CompletionRule::new().with_description("Rename a remote");
    remote_rename.exec = Some(remote_exec.to_string());
    remote_rule
        .subcommands
        .insert("rename".to_string(), remote_rename);

    let mut remote_set_url = CompletionRule::new().with_description("Change URLs for the remote");
    remote_set_url.exec = Some(remote_exec.to_string());
    remote_rule
        .subcommands
        .insert("set-url".to_string(), remote_set_url);

    let mut remote_show =
        CompletionRule::new().with_description("Give some information about the remote");
    remote_show.exec = Some(remote_exec.to_string());
    remote_rule
        .subcommands
        .insert("show".to_string(), remote_show);

    let mut remote_prune =
        CompletionRule::new().with_description("Delete stale references associated with remote");
    remote_prune.exec = Some(remote_exec.to_string());
    remote_rule
        .subcommands
        .insert("prune".to_string(), remote_prune);

    let mut remote_get_url = CompletionRule::new().with_description("Retrieves URLs for a remote");
    remote_get_url.exec = Some(remote_exec.to_string());
    remote_rule
        .subcommands
        .insert("get-url".to_string(), remote_get_url);

    git_rule
        .subcommands
        .insert("remote".to_string(), remote_rule);

    // 19. restore
    let mut restore_rule = CompletionRule::new().with_description("Restore working tree files");
    restore_rule.candidates = vec![
        CandidateSpec::from_raw("--staged:Restore the index", CompletionKind::Flag),
        CandidateSpec::from_raw("-S:Restore the index", CompletionKind::Flag),
        CandidateSpec::from_raw("--worktree:Restore the working tree", CompletionKind::Flag),
        CandidateSpec::from_raw("-W:Restore the working tree", CompletionKind::Flag),
        CandidateSpec::from_raw("--source:Restore from specified tree", CompletionKind::Flag),
        CandidateSpec::from_raw("-s:Restore from specified tree", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-p:Interactively select hunks in difference",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--patch:Interactively select hunks in difference",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    git_rule
        .subcommands
        .insert("restore".to_string(), restore_rule);

    // 20. show
    let mut show_rule = CompletionRule::new().with_description("Show various types of objects");
    show_rule.candidates = vec![
        CandidateSpec::from_raw("--stat:Generate a diffstat", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--oneline:Shorthand for --pretty=oneline --abbrev-commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--name-only:Show only names of changed files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Generate patch", CompletionKind::Flag),
        CandidateSpec::from_raw("-s:Suppress diff output", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Suppress diff output", CompletionKind::Flag),
    ];
    show_rule.exec = Some(branch_exec.to_string());
    git_rule.subcommands.insert("show".to_string(), show_rule);

    // 21. blame
    let mut blame_rule = CompletionRule::new()
        .with_description("Show what revision and author last modified each line of a file");
    blame_rule.candidates = vec![
        CandidateSpec::from_raw("-L:Annotate only given line range", CompletionKind::Flag),
        CandidateSpec::from_raw("-l:Show long rev", CompletionKind::Flag),
        CandidateSpec::from_raw("-t:Show raw timestamp", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-s:Suppress author name and timestamp",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-w:Ignore whitespace when comparing parent and child",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-C:Detect lines moved or copied in the same commit",
            CompletionKind::Flag,
        ),
    ];
    git_rule.subcommands.insert("blame".to_string(), blame_rule);

    // 22. cherry-pick
    let mut cherry_pick_rule = CompletionRule::new()
        .with_description("Apply the changes introduced by some existing commits");
    cherry_pick_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-e:Edit commit message prior to committing",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--edit:Edit commit message prior to committing",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-x:Append cherry-picked from message", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-n:Apply changes without making commits",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-commit:Apply changes without making commits",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-s:Add Signed-off-by trailer", CompletionKind::Flag),
        CandidateSpec::from_raw("--signoff:Add Signed-off-by trailer", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--abort:Cancel operation and return to pre-sequence state",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--continue:Continue operation in progress",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--skip:Skip the current commit", CompletionKind::Flag),
    ];
    cherry_pick_rule.exec = Some(branch_exec.to_string());
    git_rule
        .subcommands
        .insert("cherry-pick".to_string(), cherry_pick_rule);

    // 23. revert
    let mut revert_rule = CompletionRule::new().with_description("Revert some existing commits");
    revert_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-e:Edit commit message prior to committing",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--edit:Edit commit message prior to committing",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-m:Specify parent number for merge commit revert",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--mainline:Specify parent number for merge commit revert",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-n:Apply changes without making commits",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-commit:Apply changes without making commits",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--abort:Cancel operation and return to pre-sequence state",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--continue:Continue operation in progress",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--skip:Skip current commit", CompletionKind::Flag),
    ];
    revert_rule.exec = Some(branch_exec.to_string());
    git_rule
        .subcommands
        .insert("revert".to_string(), revert_rule);

    // 24. clean
    let mut clean_rule =
        CompletionRule::new().with_description("Remove untracked files from the working tree");
    clean_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-d:Remove untracked directories in addition to files",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-f:Force clean (required by configuration)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--force:Force clean", CompletionKind::Flag),
        CandidateSpec::from_raw("-i:Interactive mode", CompletionKind::Flag),
        CandidateSpec::from_raw("--interactive:Interactive mode", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-n:Dry run (show what would be removed)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--dry-run:Dry run", CompletionKind::Flag),
        CandidateSpec::from_raw("-x:Remove ignored files as well", CompletionKind::Flag),
        CandidateSpec::from_raw("-X:Remove only ignored files", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("clean".to_string(), clean_rule);

    // 25. rm
    let mut rm_rule =
        CompletionRule::new().with_description("Remove files from working tree and index");
    rm_rule.candidates = vec![
        CandidateSpec::from_raw("-f:Override the up-to-date check", CompletionKind::Flag),
        CandidateSpec::from_raw("--force:Override up-to-date check", CompletionKind::Flag),
        CandidateSpec::from_raw("-r:Allow recursive removal", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--cached:Unstage and remove paths only from the index",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-n:Dry run", CompletionKind::Flag),
        CandidateSpec::from_raw("--dry-run:Dry run", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("rm".to_string(), rm_rule);

    // 26. mv
    let mut mv_rule =
        CompletionRule::new().with_description("Move or rename a file, directory, or symlink");
    mv_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-f:Force renaming or moving even if target exists",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--force:Force renaming or moving", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-k:Skip move or rename actions that lead to error",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-n:Dry run", CompletionKind::Flag),
        CandidateSpec::from_raw("--dry-run:Dry run", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("mv".to_string(), mv_rule);

    // 27. reflog
    let mut reflog_rule = CompletionRule::new().with_description("Manage reflog information");
    reflog_rule.candidates = vec![
        CandidateSpec::from_raw("--all:Show reflog of all refs", CompletionKind::Flag),
        CandidateSpec::from_raw("-n:Limit number of entries", CompletionKind::Flag),
    ];
    reflog_rule.subcommands.insert(
        "show".to_string(),
        CompletionRule::new().with_description("Show log of ref updates"),
    );
    reflog_rule.subcommands.insert(
        "expire".to_string(),
        CompletionRule::new().with_description("Prune older reflog entries"),
    );
    reflog_rule.subcommands.insert(
        "delete".to_string(),
        CompletionRule::new().with_description("Delete single entry from reflog"),
    );
    reflog_rule.subcommands.insert(
        "exists".to_string(),
        CompletionRule::new().with_description("Check if ref has a reflog"),
    );
    git_rule
        .subcommands
        .insert("reflog".to_string(), reflog_rule);

    // 28. init
    let mut init_rule = CompletionRule::new()
        .with_description("Create an empty Git repository or reinitialize an existing one");
    init_rule.candidates = vec![
        CandidateSpec::from_raw("--bare:Create a bare repository", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-b:Use specified branch name for initial commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--initial-branch:Use specified branch name for initial commit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-q:Be quiet", CompletionKind::Flag),
        CandidateSpec::from_raw("--quiet:Be quiet", CompletionKind::Flag),
    ];
    git_rule.subcommands.insert("init".to_string(), init_rule);

    // 29. config
    let mut config_rule =
        CompletionRule::new().with_description("Get and set repository or global options");
    config_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--global:Use global configuration file (~/.gitconfig)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--local:Use repository configuration file (.git/config)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--system:Use system-wide configuration file (/etc/gitconfig)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-l:List all variables set in config file",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--list:List all variables set in config file",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--get:Get value for a given key", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--unset:Remove variable from config file",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-e:Open an editor to modify config file",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--edit:Open an editor to modify config file",
            CompletionKind::Flag,
        ),
    ];
    git_rule
        .subcommands
        .insert("config".to_string(), config_rule);

    // 30. worktree
    let mut worktree_rule = CompletionRule::new().with_description("Manage multiple working trees");
    worktree_rule.candidates = vec![
        CandidateSpec::from_raw("-f:Force operation", CompletionKind::Flag),
        CandidateSpec::from_raw("--force:Force operation", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Be verbose", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Be verbose", CompletionKind::Flag),
    ];
    worktree_rule.subcommands.insert(
        "add".to_string(),
        CompletionRule::new().with_description("Create a new working tree"),
    );
    worktree_rule.subcommands.insert(
        "list".to_string(),
        CompletionRule::new().with_description("List details of each working tree"),
    );
    worktree_rule.subcommands.insert(
        "lock".to_string(),
        CompletionRule::new().with_description("Lock a working tree"),
    );
    worktree_rule.subcommands.insert(
        "move".to_string(),
        CompletionRule::new().with_description("Move a working tree to a new location"),
    );
    worktree_rule.subcommands.insert(
        "prune".to_string(),
        CompletionRule::new().with_description("Prune working tree information"),
    );
    worktree_rule.subcommands.insert(
        "remove".to_string(),
        CompletionRule::new().with_description("Remove a working tree"),
    );
    worktree_rule.subcommands.insert(
        "unlock".to_string(),
        CompletionRule::new().with_description("Unlock a working tree"),
    );
    git_rule
        .subcommands
        .insert("worktree".to_string(), worktree_rule);

    // 31. submodule
    let mut submodule_rule =
        CompletionRule::new().with_description("Initialize, update or inspect submodules");
    submodule_rule.subcommands.insert(
        "add".to_string(),
        CompletionRule::new().with_description("Add a submodule"),
    );
    submodule_rule.subcommands.insert(
        "status".to_string(),
        CompletionRule::new().with_description("Show submodule status"),
    );
    submodule_rule.subcommands.insert(
        "init".to_string(),
        CompletionRule::new().with_description("Initialize submodules"),
    );
    submodule_rule.subcommands.insert(
        "deinit".to_string(),
        CompletionRule::new().with_description("Unregister a submodule"),
    );
    submodule_rule.subcommands.insert(
        "update".to_string(),
        CompletionRule::new().with_description("Update registered submodules"),
    );
    submodule_rule.subcommands.insert(
        "summary".to_string(),
        CompletionRule::new().with_description("Show commit summary between module and index"),
    );
    submodule_rule.subcommands.insert(
        "foreach".to_string(),
        CompletionRule::new().with_description("Evaluate arbitrary command in each submodule"),
    );
    submodule_rule.subcommands.insert(
        "sync".to_string(),
        CompletionRule::new().with_description("Synchronize submodules' remote URL"),
    );
    git_rule
        .subcommands
        .insert("submodule".to_string(), submodule_rule);

    // 32. bisect
    let mut bisect_rule = CompletionRule::new()
        .with_description("Use binary search to find the commit that introduced a bug");
    bisect_rule.subcommands.insert(
        "start".to_string(),
        CompletionRule::new().with_description("Start bisect session"),
    );
    bisect_rule.subcommands.insert(
        "bad".to_string(),
        CompletionRule::new().with_description("Mark current/named commit as bad"),
    );
    bisect_rule.subcommands.insert(
        "good".to_string(),
        CompletionRule::new().with_description("Mark current/named commit as good"),
    );
    bisect_rule.subcommands.insert(
        "skip".to_string(),
        CompletionRule::new().with_description("Skip current/named commit"),
    );
    bisect_rule.subcommands.insert(
        "reset".to_string(),
        CompletionRule::new().with_description("Finish bisect and return to original HEAD"),
    );
    bisect_rule.subcommands.insert(
        "log".to_string(),
        CompletionRule::new().with_description("Show bisect log"),
    );
    bisect_rule.subcommands.insert(
        "replay".to_string(),
        CompletionRule::new().with_description("Replay a bisect log file"),
    );
    git_rule
        .subcommands
        .insert("bisect".to_string(), bisect_rule);

    rules.insert("git".to_string(), git_rule);
}

pub use populate_git_rules as populate_rules;
