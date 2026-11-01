use std::io;

use lsh_syntax::print_error;

use crate::{eval::Evaluator, io::IoCtx};

pub fn plugin<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    let subcommand = args.first().map(|s| s.as_str()).unwrap_or("list");

    match subcommand {
        "list" => {
            let plugins = eval.plugin_manager.list();
            if plugins.is_empty() {
                let _ = writeln!(io.out, "No plugins loaded.");
                return 0;
            }

            let _ = writeln!(
                io.out,
                "{:<16} {:<10} {:<20} {:<20} {:<16}",
                "NAME", "VERSION", "COMMANDS", "HOOKS", "SEGMENTS"
            );
            for p in plugins {
                let version = p.version.as_deref().unwrap_or("-");
                let commands = eval.plugin_manager.list_commands_for(&p.name).join(", ");
                let commands_str = if commands.is_empty() {
                    "-".to_string()
                } else {
                    commands
                };
                let hooks = eval.plugin_manager.list_hooks_for(&p.name).join(", ");
                let hooks_str = if hooks.is_empty() {
                    "-".to_string()
                } else {
                    hooks
                };
                let segments = eval
                    .plugin_manager
                    .list_prompt_segments_for(&p.name)
                    .join(", ");
                let segments_str = if segments.is_empty() {
                    "-".to_string()
                } else {
                    segments
                };

                let _ = writeln!(
                    io.out,
                    "{:<16} {:<10} {:<20} {:<20} {:<16}",
                    p.name, version, commands_str, hooks_str, segments_str
                );
            }
            0
        }
        "info" => {
            let Some(name) = args.get(1) else {
                print_error(&mut io.err, "Usage: plugin info <name>");
                return 1;
            };

            let Some(p) = eval.plugin_manager.get(name) else {
                print_error(&mut io.err, &format!("Plugin '{name}' is not loaded."));
                return 1;
            };

            let _ = writeln!(io.out, "Plugin:      {}", p.name);
            let _ = writeln!(
                io.out,
                "Version:     {}",
                p.version.as_deref().unwrap_or("-")
            );
            let _ = writeln!(
                io.out,
                "Description: {}",
                p.description.as_deref().unwrap_or("-")
            );
            let commands = eval.plugin_manager.list_commands_for(&p.name).join(", ");
            let _ = writeln!(
                io.out,
                "Commands:    {}",
                if commands.is_empty() { "-" } else { &commands }
            );
            let hooks = eval.plugin_manager.list_hooks_for(&p.name).join(", ");
            let _ = writeln!(
                io.out,
                "Hooks:       {}",
                if hooks.is_empty() { "-" } else { &hooks }
            );
            let segments = eval
                .plugin_manager
                .list_prompt_segments_for(&p.name)
                .join(", ");
            let _ = writeln!(
                io.out,
                "Segments:    {}",
                if segments.is_empty() { "-" } else { &segments }
            );
            0
        }
        "load" => {
            let Some(target) = args.get(1) else {
                print_error(&mut io.err, "Usage: plugin load <name_or_path>");
                return 1;
            };

            match eval.load_plugin(target) {
                Ok(_) => 0,
                Err(err) => {
                    print_error(&mut io.err, &format!("Failed to load plugin: {err}"));
                    1
                }
            }
        }
        "unload" => {
            let Some(name) = args.get(1) else {
                print_error(&mut io.err, "Usage: plugin unload <name>");
                return 1;
            };

            if eval.unload_plugin(name) {
                let _ = writeln!(io.out, "Unloaded plugin '{name}'.");
                0
            } else {
                print_error(&mut io.err, &format!("Plugin '{name}' is not loaded."));
                1
            }
        }
        _ => {
            print_error(
                &mut io.err,
                &format!("Unknown subcommand '{subcommand}'. Available: list, info, load, unload"),
            );
            1
        }
    }
}
