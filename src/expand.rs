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
            glob: false,
        }
    }
}

pub fn expand_arg(arg: String, ctx: ExpandCtx) -> Vec<String> {
    let mut expanded = arg;

    if ctx.tilde {
        expanded = paths::expand_tilde(PathBuf::from(expanded))
            .to_string_lossy()
            .into_owned();
    }

    if ctx.glob {
        // Placeholder for future glob expansion.
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
}
