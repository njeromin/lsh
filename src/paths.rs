use std::path::PathBuf;

use dirs;

pub fn expand_path(path: PathBuf) -> PathBuf {
    let mut expanded_path = expand_tilde(path);

    // Normalize the path to remove any redundant components.
    expanded_path = expanded_path.canonicalize().unwrap_or(expanded_path);

    expanded_path
}

pub fn expand_tilde(path: PathBuf) -> PathBuf {
    let mut expanded_path = path;

    // Expand the home directory if it starts with '~'.
    if let Some(stripped) = expanded_path.to_str().and_then(|s| s.strip_prefix('~'))
        && let Some(home_dir) = dirs::home_dir()
    {
        if stripped.is_empty() {
            expanded_path = home_dir;
        } else {
            // `PathBuf::join` discards the left side if the right side is absolute.
            // For `~/foo`, strip the leading slash so it stays relative to `$HOME`.
            let rel = stripped.trim_start_matches('/');
            expanded_path = home_dir.join(rel);
        }
    }

    expanded_path
}
