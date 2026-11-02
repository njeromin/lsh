use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_cargo_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut cargo_rule = CompletionRule::new();
    cargo_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--version:Print version info and exit",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-V:Print version info and exit", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--list:List installed commands", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--explain:Provide a detailed explanation of an error code",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--quiet:Do not print cargo log messages",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--color:Coloring: auto, always, never",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--frozen:Require Cargo.lock and cache are up to date",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--locked:Require Cargo.lock is up to date",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--offline:Run without accessing the network",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--config:Override a configuration value",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-Z:Unstable (nightly-only) flags", CompletionKind::Flag),
    ];

    // Common compilation flags helper
    let common_compile_flags = vec![
        CandidateSpec::from_raw(
            "--release:Build artifacts in release mode, with optimizations",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-r:Build artifacts in release mode, with optimizations",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Package to build/inspect", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to build/inspect", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--workspace:Build all packages in the workspace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all:Alias for --workspace (deprecated)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--exclude:Exclude packages from the build",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all-targets:Activate all available targets",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--lib:Build only this package's library",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--bins:Build all binaries", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--bin:Build only the specified binary",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--examples:Build all examples", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--example:Build only the specified example",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--tests:Build all test targets", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--test:Build only the specified test target",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--benches:Build all bench targets", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--bench:Build only the specified bench target",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-F:Space or comma separated list of features to activate",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--features:Space or comma separated list of features to activate",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all-features:Activate all available features",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-default-features:Do not activate the default feature",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--target:Build for the target triple", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--target-dir:Directory for all generated artifacts",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--profile:Build artifacts with specified profile",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-j:Number of parallel jobs", CompletionKind::Flag),
        CandidateSpec::from_raw("--jobs:Number of parallel jobs", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--keep-going:Do not abort build on error",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--locked:Assert Cargo.lock will remain unchanged",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--offline:Run without accessing the network",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--frozen:Equivalent to both --locked and --offline",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--message-format:Error format (human, short, json)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--timings:Output a build timing report",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--quiet:Do not print cargo log messages",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];

    // 1. build
    let mut build_rule = CompletionRule::new()
        .with_description("Compile a local package and all of its dependencies");
    build_rule.candidates = common_compile_flags.clone();
    cargo_rule
        .subcommands
        .insert("build".to_string(), build_rule);

    // 2. check
    let mut check_rule = CompletionRule::new().with_description(
        "Analyze the current package and report errors without building object files",
    );
    check_rule.candidates = common_compile_flags.clone();
    cargo_rule
        .subcommands
        .insert("check".to_string(), check_rule);

    // 3. clippy
    let mut clippy_rule = CompletionRule::new()
        .with_description("Checks a package to catch common mistakes and improve your Rust code");
    let mut clippy_flags = common_compile_flags.clone();
    clippy_flags.extend(vec![
        CandidateSpec::from_raw(
            "--fix:Automatically apply lint suggestions",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--allow-dirty:Fix code even if working directory is dirty",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--allow-staged:Fix code even if working directory has staged changes",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-deps:Run Clippy only on the given crate without dependencies",
            CompletionKind::Flag,
        ),
    ]);
    clippy_rule.candidates = clippy_flags;
    cargo_rule
        .subcommands
        .insert("clippy".to_string(), clippy_rule);

    // 4. run
    let mut run_rule =
        CompletionRule::new().with_description("Run a binary or example of the local package");
    run_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--release:Build and run in release mode",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-r:Build and run in release mode", CompletionKind::Flag),
        CandidateSpec::from_raw("--bin:Name of the bin target to run", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--example:Name of the example target to run",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Package with the target to run", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--package:Package with the target to run",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-F:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw("--features:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--all-features:Activate all available features",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-default-features:Do not activate the default feature",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--target:Target triple for which code is compiled",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--profile:Build artifacts with specified profile",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--locked:Assert Cargo.lock will remain unchanged",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--offline:Run without accessing the network",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--quiet:Do not print cargo log messages",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("run".to_string(), run_rule);

    // 5. test
    let mut test_rule = CompletionRule::new().with_description(
        "Execute all unit and integration tests and build examples of a local package",
    );
    test_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--release:Build and run in release mode",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-r:Build and run in release mode", CompletionKind::Flag),
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
        CandidateSpec::from_raw(
            "--no-run:Compile, but do not run tests",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-fail-fast:Run all tests regardless of failure",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Package to test", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to test", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--workspace:Test all packages in the workspace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all:Alias for --workspace (deprecated)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--lib:Test only this package's library",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--bins:Test all binaries", CompletionKind::Flag),
        CandidateSpec::from_raw("--bin:Test only the specified binary", CompletionKind::Flag),
        CandidateSpec::from_raw("--examples:Test all examples", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--example:Test only the specified example",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--tests:Test all targets that have test = true set",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--benches:Test all bench targets", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--bench:Test only the specified bench target",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--all-targets:Test all targets", CompletionKind::Flag),
        CandidateSpec::from_raw("-F:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw("--features:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--all-features:Activate all available features",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-default-features:Do not activate the default feature",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--target:Build for the target triple", CompletionKind::Flag),
        CandidateSpec::from_raw("-j:Number of parallel jobs", CompletionKind::Flag),
        CandidateSpec::from_raw("--jobs:Number of parallel jobs", CompletionKind::Flag),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--locked:Assert Cargo.lock will remain unchanged",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--offline:Run without accessing the network",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("--verbose:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--quiet:Do not print cargo log messages",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("test".to_string(), test_rule);

    // 6. clean
    let mut clean_rule = CompletionRule::new()
        .with_description("Remove artifacts that cargo has generated in the past");
    clean_rule.candidates = vec![
        CandidateSpec::from_raw("-p:Package to clean artifacts for", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--package:Package to clean artifacts for",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--doc:Clean only documentation", CompletionKind::Flag),
        CandidateSpec::from_raw("--release:Clean release artifacts", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--profile:Clean artifacts of specified profile",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--target:Target triple to clean artifacts for",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--target-dir:Directory for all generated artifacts",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("clean".to_string(), clean_rule);

    // 7. new
    let mut new_rule =
        CompletionRule::new().with_description("Create a new cargo package at <path>");
    new_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--bin:Use a binary (application) template [default]",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--lib:Use a library template", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--edition:Edition to set for crate (2015, 2018, 2021, 2024)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--name:Set the resulting package name",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--vcs:Initialize a new VCS repository (git, hg, none)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("new".to_string(), new_rule);

    // 8. init
    let mut init_rule = CompletionRule::new()
        .with_description("Create a new cargo package in an existing directory");
    init_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--bin:Use a binary (application) template [default]",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--lib:Use a library template", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--edition:Edition to set for crate (2015, 2018, 2021, 2024)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--name:Set the resulting package name",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--vcs:Initialize a new VCS repository (git, hg, none)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("init".to_string(), init_rule);

    // 9. doc
    let mut doc_rule = CompletionRule::new().with_description("Build a package's documentation");
    doc_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--open:Opens the docs in a browser after the operation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-deps:Do not build documentation for dependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--document-private-items:Document private items",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Package to document", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to document", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--workspace:Document all packages in workspace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--lib:Document only this package's library",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--bins:Document all binaries", CompletionKind::Flag),
        CandidateSpec::from_raw("--bin:Document only specified binary", CompletionKind::Flag),
        CandidateSpec::from_raw("--examples:Document all examples", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--example:Document only specified example",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--all-targets:Document all targets", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--release:Build artifacts in release mode",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-r:Build artifacts in release mode", CompletionKind::Flag),
        CandidateSpec::from_raw("-F:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw("--features:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--all-features:Activate all available features",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-default-features:Do not activate default feature",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("doc".to_string(), doc_rule);

    // 10. update
    let mut update_rule = CompletionRule::new()
        .with_description("Update dependencies as recorded in the local lock file");
    update_rule.candidates = vec![
        CandidateSpec::from_raw("-p:Package to update", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to update", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--aggressive:Force updating all dependencies of <SPEC> as well",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--precise:Update to exact version", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--dry-run:Don't actually write lockfile",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--workspace:Only update specified package and its dependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("update".to_string(), update_rule);

    // 11. metadata
    let mut metadata_rule = CompletionRule::new().with_description(
        "Output the resolved dependencies of a package in machine-readable format",
    );
    metadata_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--no-deps:Do not include dependencies in output",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--format-version:Format version (currently 1)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--locked:Assert Cargo.lock will remain unchanged",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--offline:Run without accessing network",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("metadata".to_string(), metadata_rule);

    // 12. tree
    let mut tree_rule = CompletionRule::new()
        .with_description("Display a tree visualization of a dependency graph");
    tree_rule.candidates = vec![
        CandidateSpec::from_raw("-p:Package to inspect", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to inspect", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--workspace:Inspect all packages in workspace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--all:Alias for --workspace (deprecated)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-i:Invert tree graph", CompletionKind::Flag),
        CandidateSpec::from_raw("--invert:Invert tree graph", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--prefix:Display prefix (depth, indent, none)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-dedupe:Do not deduplicate repeated dependencies",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-d:Show only dependencies which come in multiple versions",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--duplicates:Show only dependencies with multiple versions",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-e:Dependency kinds to include (normal, build, dev, all)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--edges:Dependency kinds to include", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--target:Filter dependencies matching target triple",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-F:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw("--features:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--all-features:Activate all available features",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-default-features:Do not activate default feature",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--depth:Maximum display depth of dependency tree",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("tree".to_string(), tree_rule);

    // 13. bench
    let mut bench_rule =
        CompletionRule::new().with_description("Execute all benchmarks of a local package");
    bench_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--bench:Benchmark only specified target",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-run:Compile, but do not run benchmarks",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Package to run benchmarks for", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--package:Package to run benchmarks for",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--workspace:Benchmark all packages in workspace",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-F:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw("--features:Features to activate", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--all-features:Activate all available features",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("bench".to_string(), bench_rule);

    // 14. publish
    let mut publish_rule =
        CompletionRule::new().with_description("Package and upload this package to the registry");
    publish_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--dry-run:Perform all checks without uploading",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--token:Token to use when authenticating with registry",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-verify:Don't verify contents by building them",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--allow-dirty:Allow dirty working directories to be packaged",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Package to publish", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to publish", CompletionKind::Flag),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-v:Use verbose output", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Do not print cargo log messages", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("publish".to_string(), publish_rule);

    // 15. add
    let mut add_rule =
        CompletionRule::new().with_description("Add dependencies to a Cargo.toml manifest file");
    add_rule.candidates = vec![
        CandidateSpec::from_raw("-D:Add as development dependency", CompletionKind::Flag),
        CandidateSpec::from_raw("--dev:Add as development dependency", CompletionKind::Flag),
        CandidateSpec::from_raw("-B:Add as build dependency", CompletionKind::Flag),
        CandidateSpec::from_raw("--build:Add as build dependency", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--optional:Add as optional dependency",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-default-features:Do not activate default features",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-F:Space-separated list of features to add",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--features:Space-separated list of features to add",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--path:Filesystem path to local crate to add",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--git:Git URL to add dependency from", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--branch:Branch to use when adding from git",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--tag:Tag to use when adding from git",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--rev:Specific commit to use when adding from git",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-p:Package to modify", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to modify", CompletionKind::Flag),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule.subcommands.insert("add".to_string(), add_rule);

    // 16. remove
    let mut remove_rule = CompletionRule::new()
        .with_description("Remove dependencies from a Cargo.toml manifest file");
    remove_rule.candidates = vec![
        CandidateSpec::from_raw("-D:Remove as development dependency", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--dev:Remove as development dependency",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-B:Remove as build dependency", CompletionKind::Flag),
        CandidateSpec::from_raw("--build:Remove as build dependency", CompletionKind::Flag),
        CandidateSpec::from_raw("-p:Package to modify", CompletionKind::Flag),
        CandidateSpec::from_raw("--package:Package to modify", CompletionKind::Flag),
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("remove".to_string(), remove_rule);

    // 17. install
    let mut install_rule = CompletionRule::new().with_description("Install a Rust binary");
    install_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--version:Specify a version to install",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--git:Git URL to install crate from", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--branch:Branch to use when installing from git",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--tag:Tag to use when installing from git",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--path:Filesystem path to local crate to install",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("--bin:Install only specified binary", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--root:Directory to install packages into",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-f:Force overwriting existing binaries",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--force:Force overwriting existing binaries",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--locked:Assert Cargo.lock will remain unchanged",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("install".to_string(), install_rule);

    // 18. uninstall
    let mut uninstall_rule = CompletionRule::new().with_description("Uninstall a Rust binary");
    uninstall_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--bin:Only uninstall specified binary",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--root:Directory to uninstall packages from",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("uninstall".to_string(), uninstall_rule);

    // 19. search
    let mut search_rule = CompletionRule::new().with_description("Search packages in crates.io");
    search_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--limit:Limit number of results (default 10, max 100)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("search".to_string(), search_rule);

    // 20. login
    let mut login_rule =
        CompletionRule::new().with_description("Save an API token from the registry locally");
    login_rule.candidates = vec![
        CandidateSpec::from_raw("--registry:Registry to use", CompletionKind::Flag),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("login".to_string(), login_rule);

    // 21. package
    let mut package_rule = CompletionRule::new()
        .with_description("Assemble the local package into a distributable tarball");
    package_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-l:Print files included in package without packaging",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--list:Print files included in package without packaging",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--no-verify:Don't verify contents by building them",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--allow-dirty:Allow dirty working directories to be packaged",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("package".to_string(), package_rule);

    // 22. locate-project
    let mut locate_rule = CompletionRule::new()
        .with_description("Print a JSON representation of a Cargo.toml file's location");
    locate_rule.candidates = vec![
        CandidateSpec::from_raw(
            "--message-format:Output format (json, plain)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "--workspace:Locate Cargo.toml at workspace root",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("locate-project".to_string(), locate_rule);

    // 23. generate-lockfile
    let mut lockfile_rule =
        CompletionRule::new().with_description("Generate the lockfile without building anything");
    lockfile_rule.candidates = vec![
        CandidateSpec::from_raw("--manifest-path:Path to Cargo.toml", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "--offline:Run without accessing network",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-h:Print help information", CompletionKind::Flag),
        CandidateSpec::from_raw("--help:Print help information", CompletionKind::Flag),
    ];
    cargo_rule
        .subcommands
        .insert("generate-lockfile".to_string(), lockfile_rule);

    // 24. version
    let version_rule = CompletionRule::new().with_description("Show version information");
    cargo_rule
        .subcommands
        .insert("version".to_string(), version_rule);

    // 25. help
    let help_rule = CompletionRule::new()
        .with_description("Print this message or the help of the given subcommand(s)");
    cargo_rule.subcommands.insert("help".to_string(), help_rule);

    rules.insert("cargo".to_string(), cargo_rule);
}

pub use populate_cargo_rules as populate_rules;
