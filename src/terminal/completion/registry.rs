use std::collections::HashMap;
use std::process::Command;

use super::defaults::populate_default_rules;
use super::types::{CandidateSpec, CompletionResolution, CompletionRule};
use crate::terminal::menu::{CompletionItem, CompletionKind};

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
        populate_default_rules(&mut reg.rules);
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

    #[test]
    fn resolves_kill_signals_and_processes() {
        let reg = CompletionRegistry::with_defaults();

        // "kill -" returns signal flags
        let res_flags = reg.resolve("kill", &[], "-").expect("resolve kill - failed");
        match res_flags {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "-9"));
                assert!(items.iter().any(|it| it.replacement == "-15"));
                assert!(items.iter().any(|it| it.replacement == "-STOP"));
                assert!(items.iter().all(|it| it.kind == CompletionKind::Flag));
            }
            _ => panic!("Expected Items resolution for kill -"),
        }

        // "kill " returns non-flags (PIDs from ps exec)
        if let Some(CompletionResolution::Items(items)) = reg.resolve("kill", &[], "") {
            assert!(items.iter().all(|it| it.kind != CompletionKind::Flag));
        }
    }

    #[test]
    fn resolves_npm_yarn_pnpm_subcommands_and_flags() {
        let reg = CompletionRegistry::with_defaults();

        // "npm " returns subcommands only
        let res = reg.resolve("npm", &[], "").expect("resolve npm failed");
        match res {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "run"));
                assert!(items.iter().any(|it| it.replacement == "test"));
                assert!(items.iter().any(|it| it.replacement == "install"));
                assert!(!items.iter().any(|it| it.replacement == "-v"));
            }
            _ => panic!("Expected Items resolution for npm"),
        }

        // "npm -" returns flags only
        let res_flags = reg.resolve("npm", &[], "-").expect("resolve npm - failed");
        match res_flags {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "-v"));
                assert!(items.iter().any(|it| it.replacement == "--version"));
                assert!(!items.iter().any(|it| it.replacement == "run"));
            }
            _ => panic!("Expected Items resolution for npm -"),
        }

        // "yarn " and "pnpm " have subcommands
        assert!(reg.resolve("yarn", &[], "").is_some());
        assert!(reg.resolve("pnpm", &[], "").is_some());
    }

    #[test]
    fn resolves_ssh_and_make_flags() {
        let reg = CompletionRegistry::with_defaults();

        // "ssh -" returns flags
        let res_ssh = reg.resolve("ssh", &[], "-").expect("resolve ssh - failed");
        match res_ssh {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "-p"));
                assert!(items.iter().any(|it| it.replacement == "-i"));
                assert!(items.iter().any(|it| it.replacement == "-v"));
            }
            _ => panic!("Expected Items resolution for ssh -"),
        }

        // "make -" returns flags
        let res_make = reg.resolve("make", &[], "-").expect("resolve make - failed");
        match res_make {
            CompletionResolution::Items(items) => {
                assert!(items.iter().any(|it| it.replacement == "-j"));
                assert!(items.iter().any(|it| it.replacement == "-B"));
                assert!(items.iter().any(|it| it.replacement == "-C"));
            }
            _ => panic!("Expected Items resolution for make -"),
        }
    }
}
