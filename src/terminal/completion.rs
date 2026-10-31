use std::collections::HashMap;
use std::process::Command;

use super::menu::{CompletionItem, CompletionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSpec {
    pub replacement: String,
    pub display: String,
    pub description: Option<String>,
    pub kind: CompletionKind,
}

impl CandidateSpec {
    pub fn new(
        replacement: impl Into<String>,
        display: impl Into<String>,
        kind: CompletionKind,
    ) -> Self {
        Self {
            replacement: replacement.into(),
            display: display.into(),
            description: None,
            kind,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn from_raw(raw: &str, default_kind: CompletionKind) -> Self {
        let (spec, desc) = match raw.split_once(':') {
            Some((s, d)) => (s, Some(d.to_string())),
            None => (raw, None),
        };
        let kind = if spec.starts_with('-') {
            CompletionKind::Flag
        } else {
            default_kind
        };
        Self {
            replacement: spec.to_string(),
            display: spec.to_string(),
            description: desc,
            kind,
        }
    }

    pub fn to_completion_item(&self) -> CompletionItem {
        let mut it = CompletionItem::new(&self.replacement, &self.display, self.kind);
        it.description = self.description.clone();
        it
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionRule {
    pub candidates: Vec<CandidateSpec>,
    pub subcommands: HashMap<String, CompletionRule>,
    pub exec: Option<String>,
    pub dirs_only: bool,
    pub file_extensions: Vec<String>,
}

impl CompletionRule {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionResolution {
    Items(Vec<CompletionItem>),
    DirectoriesOnly,
    FileExtensions(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct CompletionRegistry {
    rules: HashMap<String, CompletionRule>,
}

impl CompletionRegistry {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut reg = Self::new();

        // 1. Git completions
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

        reg.rules.insert("git".to_string(), git_rule);

        // 2. Cargo completions
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

        reg.rules.insert("cargo".to_string(), cargo_rule);

        // 3. Shell builtins
        let mut alias_rule = CompletionRule::new();
        alias_rule.candidates = vec![
            CandidateSpec::from_raw("set:Define or update an alias", CompletionKind::Subcommand),
            CandidateSpec::from_raw("list:List defined aliases", CompletionKind::Subcommand),
        ];
        reg.rules.insert("alias".to_string(), alias_rule);

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
        reg.rules.insert("complete".to_string(), complete_rule);

        let mut cd_rule = CompletionRule::new();
        cd_rule.dirs_only = true;
        reg.rules.insert("cd".to_string(), cd_rule);

        let mut pushd_rule = CompletionRule::new();
        pushd_rule.dirs_only = true;
        reg.rules.insert("pushd".to_string(), pushd_rule);

        reg
    }

    pub fn register(&mut self, cmd_path: &str, rule: CompletionRule) {
        let parts: Vec<&str> = cmd_path.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        let root_cmd = parts[0];
        let root_rule = self.rules.entry(root_cmd.to_string()).or_default();

        if parts.len() == 1 {
            *root_rule = rule;
            return;
        }

        let mut current = root_rule;
        for part in &parts[1..parts.len() - 1] {
            current = current.subcommands.entry((*part).to_string()).or_default();
        }

        let last_part = parts[parts.len() - 1];
        current.subcommands.insert(last_part.to_string(), rule);
    }

    pub fn remove(&mut self, cmd_path: &str) -> bool {
        let parts: Vec<&str> = cmd_path.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        if parts.len() == 1 {
            return self.rules.remove(parts[0]).is_some();
        }

        let root_cmd = parts[0];
        if let Some(mut current) = self.rules.get_mut(root_cmd) {
            for part in &parts[1..parts.len() - 1] {
                if let Some(next) = current.subcommands.get_mut(*part) {
                    current = next;
                } else {
                    return false;
                }
            }
            current.subcommands.remove(parts[parts.len() - 1]).is_some()
        } else {
            false
        }
    }

    pub fn list(&self) -> Vec<(String, &CompletionRule)> {
        let mut result = Vec::new();

        fn collect_rules<'a>(
            path: &str,
            rule: &'a CompletionRule,
            acc: &mut Vec<(String, &'a CompletionRule)>,
        ) {
            acc.push((path.to_string(), rule));
            for (sub, child) in &rule.subcommands {
                let sub_path = format!("{path} {sub}");
                collect_rules(&sub_path, child, acc);
            }
        }

        let mut sorted_keys: Vec<_> = self.rules.keys().collect();
        sorted_keys.sort();

        for key in sorted_keys {
            if let Some(rule) = self.rules.get(key) {
                collect_rules(key, rule, &mut result);
            }
        }

        result
    }

    pub fn resolve(
        &self,
        cmd: &str,
        args: &[String],
        cur_word: &str,
    ) -> Option<CompletionResolution> {
        let base_cmd = cmd.rsplit('/').next().unwrap_or(cmd);
        let mut current = self.rules.get(base_cmd)?;

        // Walk through recognized subcommands in args
        for arg in args {
            if let Some(child) = current.subcommands.get(arg) {
                current = child;
            }
        }

        if current.dirs_only
            && current.candidates.is_empty()
            && current.subcommands.is_empty()
            && current.exec.is_none()
        {
            return Some(CompletionResolution::DirectoriesOnly);
        }

        if !current.file_extensions.is_empty()
            && current.candidates.is_empty()
            && current.subcommands.is_empty()
            && current.exec.is_none()
        {
            return Some(CompletionResolution::FileExtensions(
                current.file_extensions.clone(),
            ));
        }

        let mut items = Vec::new();

        // 1. Subcommands of the current level
        let mut subcmd_keys: Vec<_> = current.subcommands.keys().collect();
        subcmd_keys.sort();
        for key in subcmd_keys {
            let child = &current.subcommands[key];
            let desc = child
                .candidates
                .iter()
                .find(|c| c.kind == CompletionKind::Subcommand && &c.replacement == key)
                .and_then(|c| c.description.clone());
            let mut item = CompletionItem::new(key, key, CompletionKind::Subcommand);
            item.description = desc;
            items.push(item);
        }

        // 2. Static candidates (flags, values, etc.)
        for spec in &current.candidates {
            items.push(spec.to_completion_item());
        }

        // 3. Dynamic exec
        if let Some(cmd_str) = &current.exec {
            let dynamic = run_exec_command(cmd_str);
            for spec in dynamic {
                items.push(spec.to_completion_item());
            }
        }

        if cur_word.starts_with('-') {
            items.retain(|it| it.kind == CompletionKind::Flag || it.replacement.starts_with('-'));
        } else {
            items.retain(|it| it.kind != CompletionKind::Flag && !it.replacement.starts_with('-'));
        }

        if items.is_empty() {
            if current.dirs_only {
                Some(CompletionResolution::DirectoriesOnly)
            } else if !current.file_extensions.is_empty() {
                Some(CompletionResolution::FileExtensions(
                    current.file_extensions.clone(),
                ))
            } else {
                None
            }
        } else {
            Some(CompletionResolution::Items(items))
        }
    }
}

fn run_exec_command(cmd_str: &str) -> Vec<CandidateSpec> {
    let output = Command::new("sh").arg("-c").arg(cmd_str).output();
    if let Ok(out) = output
        && out.status.success()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        text.lines()
            .map(|l| l.trim().trim_start_matches('*').trim())
            .filter(|l| !l.is_empty())
            .map(|l| CandidateSpec::from_raw(l, CompletionKind::Value))
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_candidate_spec() {
        let flag = CandidateSpec::from_raw("-m:Commit message", CompletionKind::Subcommand);
        assert_eq!(flag.kind, CompletionKind::Flag);
        assert_eq!(flag.replacement, "-m");
        assert_eq!(flag.description, Some("Commit message".to_string()));

        let subcmd = CandidateSpec::from_raw("commit:Record changes", CompletionKind::Subcommand);
        assert_eq!(subcmd.kind, CompletionKind::Subcommand);
        assert_eq!(subcmd.replacement, "commit");
        assert_eq!(subcmd.description, Some("Record changes".to_string()));
    }

    #[test]
    fn resolves_git_subcommands_and_flags() {
        let reg = CompletionRegistry::with_defaults();

        // Empty prefix returns subcommands only, flags excluded
        let res = reg.resolve("git", &[], "").expect("resolve git failed");
        match res {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "commit"));
                assert!(items.iter().any(|it| it.replacement == "checkout"));
                assert!(!items.iter().any(|it| it.replacement == "--version"));
            }
            _ => panic!("Expected Items resolution"),
        }

        // Prefix starting with '-' returns flags only, subcommands excluded
        let res_flags = reg.resolve("git", &[], "-").expect("resolve git - failed");
        match res_flags {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "--version"));
                assert!(items.iter().any(|it| it.replacement == "--help"));
                assert!(!items.iter().any(|it| it.replacement == "commit"));
            }
            _ => panic!("Expected Items resolution"),
        }
    }

    #[test]
    fn resolves_git_commit_flags() {
        let reg = CompletionRegistry::with_defaults();

        // Empty prefix for git commit returns None (flags excluded, falls back to files)
        let res_empty = reg.resolve("git", &["commit".to_string()], "");
        assert_eq!(res_empty, None);

        // Prefix '-' returns git commit flags
        let res = reg
            .resolve("git", &["commit".to_string()], "-")
            .expect("resolve git commit failed");
        match res {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "-m"));
                assert!(items.iter().any(|it| it.replacement == "--amend"));
            }
            _ => panic!("Expected Items resolution"),
        }
    }

    #[test]
    fn resolves_cd_as_directories_only() {
        let reg = CompletionRegistry::with_defaults();
        let res = reg.resolve("cd", &[], "").expect("resolve cd failed");
        assert_eq!(res, CompletionResolution::DirectoriesOnly);
    }

    #[test]
    fn registers_and_removes_custom_completion() {
        let mut reg = CompletionRegistry::new();
        let mut rule = CompletionRule::new();
        rule.candidates = vec![CandidateSpec::from_raw(
            "start:Start the service",
            CompletionKind::Subcommand,
        )];
        reg.register("myctl", rule);

        let res = reg.resolve("myctl", &[], "").expect("resolve myctl failed");
        match res {
            CompletionResolution::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].replacement, "start");
                assert_eq!(items[0].description, Some("Start the service".to_string()));
            }
            _ => panic!("Expected Items resolution"),
        }

        assert!(reg.remove("myctl"));
        assert!(reg.resolve("myctl", &[], "").is_none());
    }
}
