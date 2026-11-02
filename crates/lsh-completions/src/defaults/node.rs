use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub const PKG_SCRIPTS_CMD: &str = "node -e \"const p=require('./package.json');Object.entries(p.scripts||{}).forEach(([k,v])=>console.log(k+':'+v.slice(0,50)))\" 2>/dev/null || awk -F'\"' '/\"scripts\"[[:space:]]*:[[:space:]]*\\{/,/\\}/ {if ($2 != \"\" && $2 != \"scripts\") print $2}' package.json 2>/dev/null";

pub fn populate_node_rules(rules: &mut HashMap<String, CompletionRule>) {
    // 1. npm
    let mut npm_rule = CompletionRule::new();
    npm_rule.candidates = vec![
        CandidateSpec::from_raw("-v:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--version:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help", CompletionKind::Flag),
        CandidateSpec::from_raw("-g:Operate in global mode", CompletionKind::Flag),
        CandidateSpec::from_raw("--global:Operate in global mode", CompletionKind::Flag),
        CandidateSpec::from_raw("-D:Save package to devDependencies", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--save-dev:Save package to devDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-S:Save package to dependencies", CompletionKind::Flag),
        CandidateSpec::from_raw("--save:Save package to dependencies", CompletionKind::Flag),
        CandidateSpec::from_raw("-E:Save packages with exact version", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--save-exact:Save packages with exact version",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--workspace:Enable running command in context of specified workspace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-w:Enable running command in context of specified workspace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--workspaces:Enable running command in context of all workspaces",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-ws:Enable running command in context of all workspaces",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--legacy-peer-deps:Bypass peerDependency auto-installation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--dry-run:Do not make changes, only report what would be done",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-f:Force action even if error occurs", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--force:Force action even if error occurs",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--silent:Suppress all output", CompletionKind::Flag),
        CandidateSpec::from_raw("--json:Show output in JSON format", CompletionKind::Flag),
    ];

    let mut npm_run = CompletionRule::new().with_description("Run arbitrary package scripts");
    npm_run.exec = Some(PKG_SCRIPTS_CMD.to_string());
    npm_rule.subcommands.insert("run".to_string(), npm_run);

    let subcmds = [
        ("test", "Run package test script"),
        ("start", "Start package"),
        ("stop", "Stop package"),
        ("restart", "Restart package"),
        ("build", "Run build script"),
        ("install", "Install dependencies"),
        ("i", "Install dependencies"),
        ("ci", "Install a project with a clean slate"),
        ("add", "Add a package to dependencies"),
        ("uninstall", "Remove a package"),
        ("remove", "Remove a package"),
        ("update", "Update packages"),
        ("outdated", "Check for outdated packages"),
        ("audit", "Run security audit"),
        ("publish", "Publish package to registry"),
        ("init", "Create a package.json file"),
        ("pack", "Create a tarball from a package"),
        ("link", "Symlink a package folder"),
        ("unlink", "Unlink a package"),
        ("version", "Bump a package version"),
        ("view", "View registry info"),
        ("explain", "Explain why a package is installed"),
        ("doctor", "Check your npm environment"),
        ("prune", "Remove extraneous packages"),
        ("list", "List installed packages"),
        ("clean-install", "Clean install dependencies"),
        ("exec", "Run arbitrary command from npm package"),
        ("config", "Manage the npm configuration files"),
        ("cache", "Manipulate the package cache"),
        ("whoami", "Display npm username"),
        ("login", "Log in to npm registry"),
        ("logout", "Log out of npm registry"),
    ];

    for (name, desc) in subcmds {
        npm_rule.subcommands.insert(
            name.to_string(),
            CompletionRule::new().with_description(desc),
        );
    }
    rules.insert("npm".to_string(), npm_rule);

    // 2. yarn
    let mut yarn_rule = CompletionRule::new();
    yarn_rule.candidates = vec![
        CandidateSpec::from_raw("-v:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--version:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-D:Install package as devDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--dev:Install package as devDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-P:Install package as peerDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--peer:Install package as peerDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-E:Install package with exact version",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--exact:Install package with exact version",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-W:Allow installing packages to workspace root",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--ignore-workspace-root-check:Allow installing packages to workspace root",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--offline:Trigger an error if needed packages are not in cache",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--json:Show output in JSON format", CompletionKind::Flag),
        CandidateSpec::from_raw("--silent:Suppress all output", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--cwd:Working directory to run yarn in",
            CompletionKind::Flag,
        ),
    ];
    yarn_rule.exec = Some(PKG_SCRIPTS_CMD.to_string());

    let mut yarn_run = CompletionRule::new().with_description("Run a defined package script");
    yarn_run.exec = Some(PKG_SCRIPTS_CMD.to_string());
    yarn_rule.subcommands.insert("run".to_string(), yarn_run);

    let yarn_subcmds = [
        ("add", "Install a package and dependencies"),
        ("remove", "Remove package from dependencies"),
        ("install", "Install all dependencies"),
        ("build", "Run package build script"),
        ("test", "Run package test script"),
        ("start", "Run package start script"),
        ("init", "Create a new package"),
        ("publish", "Publish package to npm registry"),
        ("upgrade", "Upgrade packages to latest version"),
        ("why", "Show why a package is installed"),
        ("cache", "List or clean cached packages"),
        (
            "create",
            "Create new projects from any create-* starter kit",
        ),
        ("link", "Symlink a package folder during development"),
        ("unlink", "Unlink a previously symlinked package"),
        ("workspace", "Run a command in a workspace"),
        ("workspaces", "Show information about workspaces"),
        ("info", "Show information about a package"),
        ("outdated", "Check for outdated dependencies"),
        (
            "audit",
            "Perform a vulnerability audit against dependencies",
        ),
        ("pack", "Create a compressed gzip archive of dependencies"),
    ];

    for (name, desc) in yarn_subcmds {
        yarn_rule.subcommands.insert(
            name.to_string(),
            CompletionRule::new().with_description(desc),
        );
    }
    rules.insert("yarn".to_string(), yarn_rule);

    // 3. pnpm
    let mut pnpm_rule = CompletionRule::new();
    pnpm_rule.candidates = vec![
        CandidateSpec::from_raw("-v:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("--version:Print version", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-D:Install package as devDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--save-dev:Install package as devDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-O:Install package as optionalDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--save-optional:Install package as optionalDependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-E:Install package with exact version",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--save-exact:Install package with exact version",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-g:Install packages globally", CompletionKind::Flag),
        CandidateSpec::from_raw("--global:Install packages globally", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-r:Run command recursively in all workspaces",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--recursive:Run command recursively in all workspaces",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-w:Run command in workspace root", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--workspace-root:Run command in workspace root",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--filter:Run command on selected packages",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-F:Run command on selected packages", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--offline:Run without accessing network",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--prefer-offline:Prefer local cached packages",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--frozen-lockfile:Do not update pnpm-lock.yaml",
            CompletionKind::Flag,
        ),
    ];
    pnpm_rule.exec = Some(PKG_SCRIPTS_CMD.to_string());

    let mut pnpm_run = CompletionRule::new().with_description("Run a defined package script");
    pnpm_run.exec = Some(PKG_SCRIPTS_CMD.to_string());
    pnpm_rule.subcommands.insert("run".to_string(), pnpm_run);

    let pnpm_subcmds = [
        ("add", "Install a package and dependencies"),
        ("install", "Install all dependencies"),
        ("i", "Install all dependencies"),
        ("remove", "Remove packages from package.json"),
        ("update", "Update packages to latest version"),
        ("test", "Run package test script"),
        ("start", "Run package start script"),
        ("build", "Run package build script"),
        ("init", "Create a package.json file"),
        ("publish", "Publish package to registry"),
        (
            "dlx",
            "Fetch and execute a package from registry without installing",
        ),
        ("create", "Create a project from a starter kit"),
        (
            "exec",
            "Execute a shell script within installed packages' environment",
        ),
        ("why", "Show why a package is installed"),
        ("outdated", "Check for outdated dependencies"),
        (
            "audit",
            "Perform a vulnerability audit against dependencies",
        ),
        ("prune", "Remove extraneous packages"),
        ("link", "Connect local package to another project"),
        ("unlink", "Unlink a package"),
        ("root", "Print the path to effective root directory"),
    ];

    for (name, desc) in pnpm_subcmds {
        pnpm_rule.subcommands.insert(
            name.to_string(),
            CompletionRule::new().with_description(desc),
        );
    }
    rules.insert("pnpm".to_string(), pnpm_rule);
}

pub use populate_node_rules as populate_rules;
