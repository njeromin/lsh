use std::path::PathBuf;

use dirs;

pub fn expand_path(path: PathBuf) -> PathBuf {
    let mut expanded_path = path;

    // Expand the home directory if it starts with '~'.
    if let Some(stripped) = expanded_path.to_str().and_then(|s| s.strip_prefix('~'))
        && let Some(home_dir) = dirs::home_dir()
    {
        expanded_path = home_dir.join(stripped);
    }

    // Normalize the path to remove any redundant components.
    expanded_path = expanded_path.canonicalize().unwrap_or(expanded_path);

    expanded_path
}
