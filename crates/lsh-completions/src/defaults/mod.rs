pub mod builtin;
pub mod cargo;
pub mod git;
pub mod kill;
pub mod make;
pub mod node;
pub mod ssh;

pub use builtin::populate_builtin_rules;
pub use cargo::populate_cargo_rules;
pub use git::populate_git_rules;
pub use kill::populate_kill_rules;
pub use make::populate_make_rules;
pub use node::populate_node_rules;
pub use ssh::populate_ssh_rules;

pub use builtin as builtins;

use std::collections::HashMap;

use crate::types::CompletionRule;

pub fn populate_default_rules(rules: &mut HashMap<String, CompletionRule>) {
    populate_git_rules(rules);
    populate_cargo_rules(rules);
    populate_builtin_rules(rules);
    populate_kill_rules(rules);
    populate_node_rules(rules);
    populate_ssh_rules(rules);
    populate_make_rules(rules);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn populates_all_defaults() {
        let mut rules = HashMap::new();
        populate_default_rules(&mut rules);

        assert!(rules.contains_key("git"));
        assert!(rules.contains_key("cargo"));
        assert!(rules.contains_key("alias"));
        assert!(rules.contains_key("complete"));
        assert!(rules.contains_key("cd"));
        assert!(rules.contains_key("pushd"));
        assert!(rules.contains_key("kill"));
        assert!(rules.contains_key("npm"));
        assert!(rules.contains_key("yarn"));
        assert!(rules.contains_key("pnpm"));
        assert!(rules.contains_key("ssh"));
        assert!(rules.contains_key("make"));
    }
}
