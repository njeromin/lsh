use std::path::PathBuf;

use crate::paths;

#[derive(Debug, Clone, Copy)]
pub struct ExpandCtx {
    pub tilde: bool,
    pub glob: bool,
}

impl Default for ExpandCtx {
    fn default() -> Self {
        Self {
            tilde: true,
            glob: true,
        }
    }
}

pub fn has_glob_metachars(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '*' | '?' | '['))
}

pub fn pattern_allows_dotfiles(pattern: &str) -> bool {
    let last_component = pattern.rsplit(['/', '\\']).next().unwrap_or(pattern);
    last_component.starts_with('.')
}

pub fn expand_arg(arg: String, ctx: ExpandCtx) -> Vec<String> {
    let mut expanded = arg;

    if ctx.tilde {
        expanded = paths::expand_tilde(PathBuf::from(expanded))
            .to_string_lossy()
            .into_owned();
    }

    if ctx.glob && has_glob_metachars(&expanded) {
        let allows_dotfiles = pattern_allows_dotfiles(&expanded);

        if let Ok(paths) = glob::glob(&expanded) {
            let mut matches: Vec<String> = paths
                .filter_map(|entry| entry.ok())
                .filter(|p| {
                    if allows_dotfiles {
                        true
                    } else {
                        !p.file_name()
                            .and_then(|f| f.to_str())
                            .is_some_and(|name| name.starts_with('.'))
                    }
                })
                .map(|p| p.to_string_lossy().into_owned())
                .collect();

            if !matches.is_empty() {
                matches.sort();
                return matches;
            }
        }
    }

    vec![expanded]
}

pub fn expand_args(args: Vec<String>, ctx: ExpandCtx) -> Vec<String> {
    args.into_iter()
        .flat_map(|arg| expand_arg(arg, ctx))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_tilde_in_args_by_default() {
        let args = vec!["~/tmp".to_string()];
        let expanded = expand_args(args, ExpandCtx::default());
        assert_eq!(expanded.len(), 1);
        assert!(!expanded[0].starts_with('~'));
    }

    #[test]
    fn keeps_literal_when_tilde_disabled() {
        let args = vec!["~/tmp".to_string()];
        let expanded = expand_args(
            args,
            ExpandCtx {
                tilde: false,
                glob: false,
            },
        );
        assert_eq!(expanded, vec!["~/tmp".to_string()]);
    }

    #[test]
    fn detects_glob_metacharacters() {
        assert!(has_glob_metachars("*.rs"));
        assert!(has_glob_metachars("src/**/*.rs"));
        assert!(has_glob_metachars("file_?.txt"));
        assert!(has_glob_metachars("test_[0-9].log"));
        assert!(!has_glob_metachars("plain_filename.txt"));
        assert!(!has_glob_metachars("~/tmp/test"));
    }

    #[test]
    fn expands_wildcard_patterns_and_sorts() {
        use std::fs::{self, File};

        let temp_dir = std::env::temp_dir().join("lsh_glob_test_sort");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("Failed to create test temp dir");

        File::create(temp_dir.join("c_file.txt")).unwrap();
        File::create(temp_dir.join("a_file.txt")).unwrap();
        File::create(temp_dir.join("b_file.txt")).unwrap();
        File::create(temp_dir.join("other.log")).unwrap();

        let pattern = temp_dir.join("*.txt").to_string_lossy().into_owned();
        let expanded = expand_arg(pattern, ExpandCtx::default());

        assert_eq!(expanded.len(), 3);
        assert!(expanded[0].ends_with("a_file.txt"));
        assert!(expanded[1].ends_with("b_file.txt"));
        assert!(expanded[2].ends_with("c_file.txt"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn respects_dotfile_isolation() {
        use std::fs::{self, File};

        let temp_dir = std::env::temp_dir().join("lsh_glob_test_dotfiles");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("Failed to create test temp dir");

        File::create(temp_dir.join("visible.txt")).unwrap();
        File::create(temp_dir.join(".hidden.txt")).unwrap();

        // Asterisk wildcard must not match leading dot.
        let pattern = temp_dir.join("*.txt").to_string_lossy().into_owned();
        let expanded = expand_arg(pattern, ExpandCtx::default());
        assert_eq!(expanded.len(), 1);
        assert!(expanded[0].ends_with("visible.txt"));

        // Explicit leading dot in wildcard must match dotfiles.
        let dot_pattern = temp_dir.join(".*.txt").to_string_lossy().into_owned();
        let dot_expanded = expand_arg(dot_pattern, ExpandCtx::default());
        assert_eq!(dot_expanded.len(), 1);
        assert!(dot_expanded[0].ends_with(".hidden.txt"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn fallback_to_literal_when_no_matches() {
        let pattern = "/nonexistent_path_12345/*.xyz".to_string();
        let expanded = expand_arg(pattern.clone(), ExpandCtx::default());
        assert_eq!(expanded, vec![pattern]);
    }

    #[test]
    fn preserves_wildcard_when_glob_disabled() {
        let pattern = "*.rs".to_string();
        let expanded = expand_arg(
            pattern.clone(),
            ExpandCtx {
                tilde: true,
                glob: false,
            },
        );
        assert_eq!(expanded, vec![pattern]);
    }
}
