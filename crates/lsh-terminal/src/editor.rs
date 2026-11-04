use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use lsh_completions::{CompletionItem, CompletionKind, CompletionRegistry, CompletionResolution};
use lsh_eval::builtins::BUILTIN_NAMES;

use crate::{menu, options::CompletionType};

#[derive(Debug, PartialEq, Eq)]
pub enum CompletionContext<'a> {
    Command { prefix: &'a str, start: usize },
    Variable { prefix: &'a str, start: usize },
    Path { command: &'a str },
}

pub fn extract_command_name(trimmed: &str) -> &str {
    let mut words = trimmed.split_whitespace();
    while let Some(word) = words.next() {
        if word.contains('=') && !word.starts_with("./") && !word.starts_with('/') {
            continue;
        }
        if (word == "builtin" || word == "command")
            && let Some(next) = words.next()
        {
            return next;
        }
        return word;
    }
    ""
}

pub fn find_command_segment(sub: &str) -> &str {
    let mut last_sep = 0;
    let mut in_single = false;
    let mut in_double = false;
    let bytes = sub.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'|' | b';' if !in_single && !in_double => {
                if bytes[i] == b'|' && i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    i += 1;
                }
                last_sep = i + 1;
            }
            b'&' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'&' => {
                i += 1;
                last_sep = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    &sub[last_sep..]
}

pub fn split_path_argument(sub: &str) -> (usize, &str, &str) {
    let arg_start = sub
        .rfind(|c: char| c.is_whitespace() || matches!(c, '=' | '"' | '\''))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let path_arg = &sub[arg_start..];

    if let Some(slash_idx) = path_arg.rfind('/') {
        let dir_part = &path_arg[..=slash_idx];
        let file_prefix = &path_arg[slash_idx + 1..];
        (arg_start, dir_part, file_prefix)
    } else {
        (arg_start, "", path_arg)
    }
}

pub fn parse_command_args(segment: &str) -> (Option<&str>, Vec<String>, &str, usize) {
    let mut tokens: Vec<(&str, usize)> = Vec::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut token_start: Option<usize> = None;

    let chars: Vec<(usize, char)> = segment.char_indices().collect();
    for &(idx, c) in &chars {
        if c == '\'' && !in_double {
            in_single = !in_single;
            if token_start.is_none() {
                token_start = Some(idx);
            }
        } else if c == '"' && !in_single {
            in_double = !in_double;
            if token_start.is_none() {
                token_start = Some(idx);
            }
        } else if c.is_whitespace() && !in_single && !in_double {
            if let Some(start) = token_start {
                tokens.push((&segment[start..idx], start));
                token_start = None;
            }
        } else if token_start.is_none() {
            token_start = Some(idx);
        }
    }

    let ends_with_ws = segment.ends_with(char::is_whitespace) && !in_single && !in_double;
    let (cur_word, cur_word_start) = if ends_with_ws {
        ("", segment.len())
    } else if let Some(start) = token_start {
        (&segment[start..], start)
    } else {
        ("", segment.len())
    };

    let mut words = tokens.into_iter();
    let mut cmd_name = None;

    while let Some((word, _)) = words.next() {
        let clean = word.trim_matches(|c| c == '"' || c == '\'');
        if clean.contains('=')
            && !clean.starts_with('-')
            && !clean.starts_with("./")
            && !clean.starts_with('/')
        {
            continue;
        }
        if (clean == "builtin" || clean == "command")
            && let Some((next, _)) = words.next()
        {
            cmd_name = Some(next.trim_matches(|c| c == '"' || c == '\''));
            break;
        }
        cmd_name = Some(clean);
        break;
    }

    let Some(cmd) = cmd_name else {
        return (None, Vec::new(), cur_word, cur_word_start);
    };

    let args: Vec<String> = words
        .map(|(w, _)| w.trim_matches(|c| c == '"' || c == '\'').to_string())
        .collect();

    (Some(cmd), args, cur_word, cur_word_start)
}

pub fn classify_completion_context(line: &str, pos: usize) -> CompletionContext<'_> {
    let sub = if pos <= line.len() {
        &line[..pos]
    } else {
        line
    };

    // Check for variable completion: find the current word start.
    let word_start = sub
        .rfind(|c: char| c.is_whitespace() || matches!(c, '=' | '"' | '\''))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let current_word = &sub[word_start..];

    if let Some(var_prefix) = current_word.strip_prefix('$') {
        return CompletionContext::Variable {
            prefix: var_prefix,
            start: word_start,
        };
    }

    // Check for command position in current command segment.
    let segment = find_command_segment(sub);
    let leading_spaces = segment.len() - segment.trim_start().len();
    let trimmed = segment.trim_start();

    // If there is no whitespace in the current command segment, cursor is at command position
    // (unless typing an explicit path such as ./script, /bin/sh, or ../foo).
    if !trimmed.contains(char::is_whitespace) {
        if trimmed.starts_with("./") || trimmed.starts_with('/') || trimmed.starts_with("../") {
            return CompletionContext::Path { command: "" };
        }

        let segment_start_in_sub = sub.len() - segment.len();
        let start = segment_start_in_sub + leading_spaces;
        return CompletionContext::Command {
            prefix: trimmed,
            start,
        };
    }

    let command = extract_command_name(trimmed);
    CompletionContext::Path { command }
}

pub fn scan_path_binaries() -> Vec<String> {
    let mut binaries = BTreeSet::new();

    if let Some(path_val) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_val) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type()
                        && (file_type.is_file() || file_type.is_symlink())
                        && let Ok(name) = entry.file_name().into_string()
                    {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(meta) = entry.metadata()
                                && meta.permissions().mode() & 0o111 != 0
                            {
                                binaries.insert(name);
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            binaries.insert(name);
                        }
                    }
                }
            }
        }
    }

    binaries.into_iter().collect()
}

pub fn complete_filesystem_paths(dir_part: &str) -> Vec<(String, String, bool)> {
    let search_dir = if dir_part.is_empty() {
        PathBuf::from(".")
    } else if let Some(stripped) = dir_part.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(stripped.trim_start_matches('/'))
        } else {
            PathBuf::from(dir_part)
        }
    } else {
        PathBuf::from(dir_part)
    };

    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let display = if is_dir {
                format!("{file_name}/")
            } else {
                file_name.clone()
            };
            let replacement = if dir_part.is_empty() {
                display.clone()
            } else {
                format!("{dir_part}{display}")
            };
            results.push((replacement, display, is_dir));
        }
    }
    results.sort_by(|a, b| a.1.cmp(&b.1));
    results
}

pub fn badge_for_kind(kind: &CompletionKind, nerd_icons: bool) -> String {
    if nerd_icons {
        match kind {
            CompletionKind::Directory => " dir".to_string(),
            CompletionKind::File => " file".to_string(),
            CompletionKind::Builtin => "󰅩 builtin".to_string(),
            CompletionKind::Alias => "󰌑 alias".to_string(),
            CompletionKind::Binary => "󰆧 bin".to_string(),
            CompletionKind::Variable => "󰫧 var".to_string(),
            CompletionKind::Flag => "󰮢 flag".to_string(),
            _ => "  cmd".to_string(),
        }
    } else {
        match kind {
            CompletionKind::Directory => "[dir]".to_string(),
            CompletionKind::File => "[file]".to_string(),
            CompletionKind::Builtin => "[builtin]".to_string(),
            CompletionKind::Alias => "[alias]".to_string(),
            CompletionKind::Binary => "[bin]".to_string(),
            CompletionKind::Variable => "[var]".to_string(),
            CompletionKind::Flag => "[flag]".to_string(),
            _ => "[cmd]".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct LshCompleter {
    cached_binaries: Arc<RwLock<Vec<String>>>,
    cached_aliases: Arc<RwLock<Vec<String>>>,
    cached_vars: Arc<RwLock<Vec<String>>>,
    last_path: Arc<RwLock<Option<std::ffi::OsString>>>,
    pub completion_type: Arc<RwLock<CompletionType>>,
    pub nerd_icons: Arc<RwLock<bool>>,
    pub completion_registry: Arc<RwLock<CompletionRegistry>>,
}

pub type LineEditorHelper = LshCompleter;

impl Default for LshCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl LshCompleter {
    pub fn new() -> Self {
        Self {
            cached_binaries: Arc::new(RwLock::new(scan_path_binaries())),
            cached_aliases: Arc::new(RwLock::new(Vec::new())),
            cached_vars: Arc::new(RwLock::new(Vec::new())),
            last_path: Arc::new(RwLock::new(std::env::var_os("PATH"))),
            completion_type: Arc::new(RwLock::new(CompletionType::Menu)),
            nerd_icons: Arc::new(RwLock::new(false)),
            completion_registry: Arc::new(RwLock::new(CompletionRegistry::with_defaults())),
        }
    }

    pub fn set_completion_registry(&mut self, registry: Arc<RwLock<CompletionRegistry>>) {
        self.completion_registry = registry;
    }

    pub fn set_completion_type(&self, completion_type: CompletionType) {
        *self.completion_type.write().unwrap() = completion_type;
    }

    pub fn completion_type(&self) -> CompletionType {
        *self.completion_type.read().unwrap()
    }

    pub fn set_nerd_icons(&self, nerd_icons: bool) {
        *self.nerd_icons.write().unwrap() = nerd_icons;
    }

    pub fn nerd_icons(&self) -> bool {
        *self.nerd_icons.read().unwrap()
    }

    pub fn set_aliases(&self, aliases: Vec<String>) {
        *self.cached_aliases.write().unwrap() = aliases;
    }

    pub fn set_vars(&self, vars: Vec<String>) {
        *self.cached_vars.write().unwrap() = vars;
    }

    pub fn set_binaries(&self, binaries: Vec<String>) {
        *self.cached_binaries.write().unwrap() = binaries;
    }

    pub fn cached_binaries(&self) -> Vec<String> {
        self.cached_binaries.read().unwrap().clone()
    }

    pub fn set_last_path(&self, path: Option<std::ffi::OsString>) {
        *self.last_path.write().unwrap() = path;
    }

    pub fn refresh_binaries_if_path_changed(&self) {
        let current_path = std::env::var_os("PATH");
        let mut last_path = self.last_path.write().unwrap();
        if current_path != *last_path {
            *self.cached_binaries.write().unwrap() = scan_path_binaries();
            *last_path = current_path;
        }
    }

    pub fn reset_prompt_marks(&self) {}

    pub fn get_candidates(&self, line: &str, pos: usize) -> (usize, Vec<CompletionItem>) {
        match classify_completion_context(line, pos) {
            CompletionContext::Command { prefix, start } => {
                let mut map = BTreeMap::new();

                for binary in self.cached_binaries.read().unwrap().iter() {
                    map.insert(binary.clone(), CompletionKind::Binary);
                }

                for alias in self.cached_aliases.read().unwrap().iter() {
                    map.insert(alias.clone(), CompletionKind::Alias);
                }

                for &builtin in BUILTIN_NAMES {
                    map.insert(builtin.to_string(), CompletionKind::Builtin);
                }

                let all_items: Vec<CompletionItem> = map
                    .into_iter()
                    .map(|(name, kind)| CompletionItem::new(name.clone(), name, kind))
                    .collect();

                let items = menu::filter_completion_items(&all_items, prefix);

                (start, items)
            }
            CompletionContext::Variable { prefix, start } => {
                let mut candidates = BTreeSet::new();

                for var in self.cached_vars.read().unwrap().iter() {
                    candidates.insert(var.clone());
                }

                for (key, _) in std::env::vars() {
                    candidates.insert(key);
                }

                let all_items: Vec<CompletionItem> = candidates
                    .into_iter()
                    .map(|name| {
                        CompletionItem::new(
                            format!("${name}"),
                            format!("${name}"),
                            CompletionKind::Variable,
                        )
                    })
                    .collect();

                let query = if prefix.starts_with('$') {
                    prefix.to_string()
                } else {
                    format!("${prefix}")
                };
                let items = menu::filter_completion_items(&all_items, &query);

                (start, items)
            }
            CompletionContext::Path { command: _ } => {
                let sub = if pos <= line.len() {
                    &line[..pos]
                } else {
                    line
                };
                let segment = find_command_segment(sub);
                let segment_start_in_sub = sub.len() - segment.len();
                let (cmd_opt, args, cur_word, cur_word_start_in_segment) =
                    parse_command_args(segment);
                let abs_cur_word_start = segment_start_in_sub + cur_word_start_in_segment;

                let mut dirs_only_override = false;
                let mut exts_filter: Option<Vec<String>> = None;

                if let Some(cmd) = cmd_opt {
                    let reg = self.completion_registry.read().unwrap();
                    if let Some(resolution) = reg.resolve(cmd, &args, cur_word) {
                        match resolution {
                            CompletionResolution::Items(items) => {
                                let filtered = if cur_word.is_empty() {
                                    items
                                } else {
                                    menu::filter_completion_items(&items, cur_word)
                                };
                                return (abs_cur_word_start, filtered);
                            }
                            CompletionResolution::DirectoriesOnly => {
                                dirs_only_override = true;
                            }
                            CompletionResolution::FileExtensions(exts) => {
                                exts_filter = Some(exts);
                            }
                        }
                    }
                }

                let (arg_start, dir_part, file_prefix) = split_path_argument(sub);
                let pairs = complete_filesystem_paths(dir_part);

                let base_cmd = cmd_opt.unwrap_or("").rsplit('/').next().unwrap_or("");
                let dirs_only = dirs_only_override || base_cmd == "cd" || base_cmd == "pushd";

                let all_items: Vec<CompletionItem> = pairs
                    .into_iter()
                    .filter_map(|(replacement, display, is_dir)| {
                        if dirs_only && !is_dir {
                            return None;
                        }
                        if let Some(exts) = &exts_filter
                            && !is_dir
                        {
                            let p_ext = replacement.rsplit('.').next().unwrap_or("");
                            if !exts.iter().any(|e| e == p_ext) {
                                return None;
                            }
                        }
                        let kind = if is_dir {
                            CompletionKind::Directory
                        } else {
                            CompletionKind::File
                        };
                        Some(CompletionItem::new(replacement, display, kind))
                    })
                    .collect();

                let items = if file_prefix.is_empty() {
                    all_items
                } else {
                    menu::filter_completion_items(&all_items, file_prefix)
                };

                (arg_start, items)
            }
        }
    }
}

impl lsh_input::Completer for LshCompleter {
    fn complete(&self, line: &str, cursor: usize) -> (usize, Vec<lsh_input::Candidate>) {
        let (start, items) = self.get_candidates(line, cursor);
        let nerd_icons = *self.nerd_icons.read().unwrap();
        let candidates = items
            .into_iter()
            .map(|item| {
                let is_dir = matches!(item.kind, CompletionKind::Directory);
                let badge = badge_for_kind(&item.kind, nerd_icons);
                let display = format!("{} {}", badge, item.display);
                let mut candidate = lsh_input::Candidate::new(item.replacement)
                    .with_display(display)
                    .with_append_space(!is_dir);
                if let Some(desc) = item.description {
                    candidate = candidate.with_description(desc);
                }
                candidate
            })
            .collect();
        (start, candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_initial_command_context() {
        assert_eq!(
            classify_completion_context("gi", 2),
            CompletionContext::Command {
                prefix: "gi",
                start: 0,
            }
        );
        assert_eq!(
            classify_completion_context("  c", 3),
            CompletionContext::Command {
                prefix: "c",
                start: 2,
            }
        );
        assert_eq!(
            classify_completion_context("", 0),
            CompletionContext::Command {
                prefix: "",
                start: 0,
            }
        );
    }

    #[test]
    fn classifies_pipeline_and_chained_command_context() {
        assert_eq!(
            classify_completion_context("echo hello | gr", 15),
            CompletionContext::Command {
                prefix: "gr",
                start: 13,
            }
        );
        assert_eq!(
            classify_completion_context("git status; ec", 14),
            CompletionContext::Command {
                prefix: "ec",
                start: 12,
            }
        );
        assert_eq!(
            classify_completion_context("cmd1 && cmd2 || cm", 18),
            CompletionContext::Command {
                prefix: "cm",
                start: 16,
            }
        );
    }

    #[test]
    fn classifies_path_context_after_command() {
        assert_eq!(
            classify_completion_context("ls ", 3),
            CompletionContext::Path { command: "ls" }
        );
        assert_eq!(
            classify_completion_context("cat src/m", 9),
            CompletionContext::Path { command: "cat" }
        );
        assert_eq!(
            classify_completion_context("./script", 8),
            CompletionContext::Path { command: "" }
        );
        assert_eq!(
            classify_completion_context("/bin/s", 6),
            CompletionContext::Path { command: "" }
        );
        assert_eq!(
            classify_completion_context("cd /tmp", 7),
            CompletionContext::Path { command: "cd" }
        );
        assert_eq!(
            classify_completion_context("FOO=bar pushd /tmp", 18),
            CompletionContext::Path { command: "pushd" }
        );
    }

    #[test]
    fn classifies_variable_context() {
        assert_eq!(
            classify_completion_context("echo $P", 7),
            CompletionContext::Variable {
                prefix: "P",
                start: 5,
            }
        );
        assert_eq!(
            classify_completion_context("FOO=$B", 6),
            CompletionContext::Variable {
                prefix: "B",
                start: 4,
            }
        );
        assert_eq!(
            classify_completion_context("echo \"$FOO", 10),
            CompletionContext::Variable {
                prefix: "FOO",
                start: 6,
            }
        );
    }

    #[test]
    fn completes_commands_including_builtins_aliases_and_binaries() {
        let helper = LineEditorHelper::new();
        helper.set_aliases(vec!["my_alias".to_string()]);
        helper.set_binaries(vec!["custom_bin".to_string()]);

        let (start, items) = helper.get_candidates("my_", 3);
        assert_eq!(start, 0);
        assert!(items.iter().any(|i| i.replacement == "my_alias"));

        let (start, items) = helper.get_candidates("custom_", 7);
        assert_eq!(start, 0);
        assert!(items.iter().any(|i| i.replacement == "custom_bin"));

        let (start, items) = helper.get_candidates("al", 2);
        assert_eq!(start, 0);
        assert!(items.iter().any(|i| i.replacement == "alias"));
    }

    #[test]
    fn candidates_have_correct_category_badges() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["cargo".to_string()]);
        helper.set_aliases(vec!["co".to_string()]);
        helper.set_vars(vec!["MY_VAR".to_string()]);

        let (_, items) = helper.get_candidates("c", 1);
        let builtin_item = items.iter().find(|it| it.replacement == "cd").unwrap();
        assert_eq!(builtin_item.kind, CompletionKind::Builtin);

        let alias_item = items.iter().find(|it| it.replacement == "co").unwrap();
        assert_eq!(alias_item.kind, CompletionKind::Alias);

        let bin_item = items.iter().find(|it| it.replacement == "cargo").unwrap();
        assert_eq!(bin_item.kind, CompletionKind::Binary);

        let (_, var_items) = helper.get_candidates("$MY_VAR", 7);
        let var_item = var_items
            .iter()
            .find(|it| it.replacement == "$MY_VAR")
            .unwrap();
        assert_eq!(var_item.kind, CompletionKind::Variable);
    }

    #[test]
    fn completes_with_menu_type_returns_suggestions() {
        use lsh_input::Completer;

        let helper = LineEditorHelper::new();
        helper.set_binaries(vec![]);
        helper.set_aliases(vec!["gco".to_string()]);
        let (start, candidates) = helper.complete("gc", 2);
        assert_eq!(start, 0);
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].replacement, "gco");
    }

    #[test]
    fn completes_variables_with_dollar_prefix() {
        let helper = LineEditorHelper::new();
        helper.set_vars(vec!["MY_SHELL_VAR".to_string()]);

        let (start, items) = helper.get_candidates("echo $MY_SHELL", 14);
        assert_eq!(start, 5);

        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert_eq!(names, vec!["$MY_SHELL_VAR".to_string()]);
    }

    #[test]
    fn completes_files_in_path_context() {
        let helper = LineEditorHelper::new();
        let (start, candidates) = helper.get_candidates("cat src/ed", 10);
        assert_eq!(start, 4);
        assert!(
            candidates
                .iter()
                .any(|item| item.replacement == "src/editor.rs")
        );
    }

    #[test]
    fn completes_commands_with_direct_fuzzy_matching() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec![
            "git".to_string(),
            "grep".to_string(),
            "cargo".to_string(),
            "curl".to_string(),
        ]);

        let (start, items) = helper.get_candidates("cr", 2);
        assert_eq!(start, 0);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert!(names.contains(&"cargo".to_string()));
        assert!(names.contains(&"curl".to_string()));
    }

    #[test]
    fn completes_single_fuzzy_match_directly() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec![
            "something_very_unique".to_string(),
            "other_command".to_string(),
        ]);

        let (start, items) = helper.get_candidates("svu", 3);
        assert_eq!(start, 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].replacement, "something_very_unique");
    }

    #[test]
    fn completes_variables_with_direct_fuzzy_matching() {
        let helper = LineEditorHelper::new();
        helper.set_vars(vec![
            "CARGO_HOME".to_string(),
            "CONFIG_DIR".to_string(),
            "OTHER_VAR".to_string(),
        ]);

        let (start, items) = helper.get_candidates("$cg", 3);
        assert_eq!(start, 0);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert!(names.contains(&"$CARGO_HOME".to_string()));
        assert!(names.contains(&"$CONFIG_DIR".to_string()));
        assert!(!names.contains(&"$OTHER_VAR".to_string()));
    }

    #[test]
    fn completes_paths_with_direct_fuzzy_matching() {
        let helper = LineEditorHelper::new();
        let (start, candidates) = helper.get_candidates("cat ct", 6);
        assert_eq!(start, 4);
        assert!(
            candidates
                .iter()
                .any(|item| item.replacement.contains("Cargo.toml"))
        );
    }

    #[test]
    fn completes_directories_only_for_cd_and_pushd_commands() {
        let helper = LineEditorHelper::new();

        let (start, candidates) = helper.get_candidates("cd src/", 7);
        assert_eq!(start, 3);
        for item in &candidates {
            assert!(
                item.replacement.ends_with('/'),
                "Expected directory for cd, got {}",
                item.replacement
            );
        }

        let (start, candidates) = helper.get_candidates("pushd src/", 10);
        assert_eq!(start, 6);
        for item in &candidates {
            assert!(
                item.replacement.ends_with('/'),
                "Expected directory for pushd, got {}",
                item.replacement
            );
        }
    }

    #[test]
    fn completes_both_files_and_directories_for_general_commands() {
        let helper = LineEditorHelper::new();
        let (start, candidates) = helper.get_candidates("cat src/", 8);
        assert_eq!(start, 4);
        let has_files = candidates.iter().any(|c| !c.replacement.ends_with('/'));
        let has_dirs = candidates.iter().any(|c| c.replacement.ends_with('/'));
        assert!(has_files, "Expected files for cat");
        assert!(has_dirs, "Expected directories for cat");
    }

    #[test]
    fn parse_command_args_extracts_command_subcommands_and_cur_word() {
        let (cmd, args, cur_word, _) = parse_command_args("git checkout -b feature");
        assert_eq!(cmd, Some("git"));
        assert_eq!(args, vec!["checkout", "-b"]);
        assert_eq!(cur_word, "feature");

        let (cmd, args, cur_word, _) = parse_command_args("git commit ");
        assert_eq!(cmd, Some("git"));
        assert_eq!(args, vec!["commit"]);
        assert_eq!(cur_word, "");

        let (cmd, args, cur_word, _) = parse_command_args("git ");
        assert_eq!(cmd, Some("git"));
        assert_eq!(args, Vec::<String>::new());
        assert_eq!(cur_word, "");
    }

    #[test]
    fn completes_git_subcommands_and_flags() {
        let helper = LineEditorHelper::new();

        let (start, items) = helper.get_candidates("git c", 5);
        assert_eq!(start, 4);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert!(names.contains(&"commit".to_string()));
        assert!(names.contains(&"checkout".to_string()));
        assert!(names.contains(&"clone".to_string()));

        let (start, items) = helper.get_candidates("git checkout -", 14);
        assert_eq!(start, 13);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert!(names.contains(&"-b".to_string()));
        assert!(names.contains(&"-B".to_string()));
        assert!(names.contains(&"--track".to_string()));
    }

    #[test]
    fn completes_git_commit_flags() {
        let helper = LineEditorHelper::new();
        let (start, items) = helper.get_candidates("git commit -", 12);
        assert_eq!(start, 11);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert!(names.contains(&"-m".to_string()));
        assert!(names.contains(&"-a".to_string()));
        assert!(names.contains(&"--amend".to_string()));
        assert!(names.contains(&"--message".to_string()));
    }

    #[test]
    fn completes_nested_subcommands_and_subcommand_flags() {
        let helper = LineEditorHelper::new();

        let (start, items) = helper.get_candidates("git remote ", 11);
        assert_eq!(start, 11);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert!(names.contains(&"add".to_string()));
        assert!(names.contains(&"rename".to_string()));
        assert!(names.contains(&"remove".to_string()));

        let (start, items) = helper.get_candidates("git remote -", 12);
        assert_eq!(start, 11);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert!(names.contains(&"-v".to_string()));
        assert!(names.contains(&"--verbose".to_string()));
    }

    #[test]
    fn completes_custom_registered_command() {
        use lsh_completions::{CandidateSpec, CompletionKind, CompletionRule};

        let helper = LineEditorHelper::new();
        {
            let mut reg = helper.completion_registry.write().unwrap();
            let mut rule = CompletionRule::new();
            rule.candidates = vec![CandidateSpec::from_raw(
                "test-cmd",
                CompletionKind::Subcommand,
            )];
            reg.register("my-cli", rule);
        }

        let (start, items) = helper.get_candidates("my-cli t", 8);
        assert_eq!(start, 7);
        let names: Vec<String> = items.into_iter().map(|i| i.replacement).collect();
        assert_eq!(names, vec!["test-cmd".to_string()]);
    }

    #[test]
    fn refresh_binaries_on_path_change() {
        let helper = LineEditorHelper::new();
        let initial_len = helper.cached_binaries().len();

        helper.set_last_path(Some(std::ffi::OsString::from("/nonexistent_path_xyz")));
        helper.refresh_binaries_if_path_changed();

        assert_eq!(helper.cached_binaries().len(), initial_len);
    }
}
