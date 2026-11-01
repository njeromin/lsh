use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::eval::value::{FunctionValue, Value};

#[derive(Clone, Debug)]
pub struct Plugin {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub manifest: Value,
}

#[derive(Clone, Debug)]
pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
    pub search_paths: Vec<PathBuf>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        let mut search_paths = Vec::new();
        if let Ok(env_path) = std::env::var("LSH_PLUGIN_DIR") {
            search_paths.push(PathBuf::from(env_path));
        }
        if let Some(cfg) = dirs::config_dir() {
            search_paths.push(cfg.join("lsh").join("plugins"));
        }
        if let Some(data) = dirs::data_dir() {
            search_paths.push(data.join("lsh").join("plugins"));
        }
        search_paths.push(PathBuf::from("./config/plugins"));
        search_paths.push(PathBuf::from("config/plugins"));
        search_paths.push(PathBuf::from("./plugins"));

        Self {
            plugins: HashMap::new(),
            search_paths,
        }
    }

    pub fn register(&mut self, manifest: Value) -> Result<String, String> {
        let name = match manifest.get_property("name") {
            Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return Err("Plugin manifest must contain a non-empty 'name' string.".to_string()),
        };

        let version = manifest.get_property("version").and_then(|v| match v {
            Value::String(s) => Some(s),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        });

        let description = manifest.get_property("description").and_then(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        });

        let plugin = Plugin {
            name: name.clone(),
            version,
            description,
            manifest,
        };

        self.plugins.insert(name.clone(), plugin);
        Ok(name)
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        self.plugins.remove(name).is_some()
    }

    pub fn get(&self, name: &str) -> Option<&Plugin> {
        self.plugins.get(name)
    }

    pub fn list(&self) -> Vec<&Plugin> {
        let mut list: Vec<&Plugin> = self.plugins.values().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub fn get_command(&self, cmd_name: &str) -> Option<(Arc<FunctionValue>, Value)> {
        for plugin in self.plugins.values() {
            if let Some(commands_val) = plugin.manifest.get_property("commands")
                && let Some(Value::Function(f)) = commands_val.get_property(cmd_name)
            {
                return Some((f, plugin.manifest.clone()));
            }
        }
        None
    }

    pub fn list_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();
        for plugin in self.plugins.values() {
            commands.extend(self.list_commands_for(&plugin.name));
        }
        commands.sort();
        commands.dedup();
        commands
    }

    pub fn list_commands_for(&self, plugin_name: &str) -> Vec<String> {
        let mut commands = Vec::new();
        if let Some(plugin) = self.plugins.get(plugin_name)
            && let Some(Value::Object(ref map)) = plugin.manifest.get_property("commands")
        {
            let guard = map.read().unwrap_or_else(|p| p.into_inner());
            for (cmd_name, val) in guard.iter() {
                if matches!(val, Value::Function(_)) {
                    commands.push(cmd_name.clone());
                }
            }
        }
        commands.sort();
        commands
    }

    pub fn get_hooks_for(&self, hook_name: &str) -> Vec<(Arc<FunctionValue>, Value)> {
        let alt_name = if let Some(stripped) = hook_name.strip_prefix("on_") {
            stripped
        } else {
            hook_name
        };
        let full_name = if !hook_name.starts_with("on_") {
            format!("on_{hook_name}")
        } else {
            hook_name.to_string()
        };

        let mut result = Vec::new();
        for plugin in self.plugins.values() {
            let mut found = None;

            if let Some(hooks_val) = plugin.manifest.get_property("hooks")
                && let Some(Value::Function(f)) = hooks_val
                    .get_property(&full_name)
                    .or_else(|| hooks_val.get_property(alt_name))
            {
                found = Some(f);
            }

            if found.is_none()
                && let Some(Value::Function(f)) = plugin
                    .manifest
                    .get_property(&full_name)
                    .or_else(|| plugin.manifest.get_property(alt_name))
            {
                found = Some(f);
            }

            if let Some(f) = found {
                result.push((f, plugin.manifest.clone()));
            }
        }
        result
    }

    pub fn list_hooks_for(&self, plugin_name: &str) -> Vec<String> {
        let mut hooks = Vec::new();
        let candidate_hooks = ["on_pre_prompt", "on_pre_exec", "on_post_exec", "on_chpwd"];
        if let Some(plugin) = self.plugins.get(plugin_name) {
            for hook in candidate_hooks {
                let alt = hook.strip_prefix("on_").unwrap();
                let mut has_hook = false;
                if let Some(hooks_val) = plugin.manifest.get_property("hooks")
                    && (hooks_val.get_property(hook).is_some()
                        || hooks_val.get_property(alt).is_some())
                {
                    has_hook = true;
                }
                if !has_hook
                    && (plugin.manifest.get_property(hook).is_some()
                        || plugin.manifest.get_property(alt).is_some())
                {
                    has_hook = true;
                }
                if has_hook {
                    hooks.push(hook.to_string());
                }
            }
        }
        hooks
    }

    pub fn get_prompt_segment(&self, segment_name: &str) -> Option<(Arc<FunctionValue>, Value)> {
        for plugin in self.plugins.values() {
            if let Some(segments_val) = plugin
                .manifest
                .get_property("prompt_segments")
                .or_else(|| plugin.manifest.get_property("segments"))
                && let Some(Value::Function(f)) = segments_val.get_property(segment_name)
            {
                return Some((f, plugin.manifest.clone()));
            }
        }
        None
    }

    pub fn list_prompt_segments(&self) -> Vec<String> {
        let mut segments = Vec::new();
        for plugin in self.plugins.values() {
            segments.extend(self.list_prompt_segments_for(&plugin.name));
        }
        segments.sort();
        segments.dedup();
        segments
    }

    pub fn list_prompt_segments_for(&self, plugin_name: &str) -> Vec<String> {
        let mut segments = Vec::new();
        if let Some(plugin) = self.plugins.get(plugin_name)
            && let Some(segments_val) = plugin
                .manifest
                .get_property("prompt_segments")
                .or_else(|| plugin.manifest.get_property("segments"))
            && let Value::Object(ref map) = segments_val
        {
            let guard = map.read().unwrap_or_else(|p| p.into_inner());
            for (name, val) in guard.iter() {
                if matches!(val, Value::Function(_)) {
                    segments.push(name.clone());
                }
            }
        }
        segments.sort();
        segments
    }

    pub fn find_plugin_file(&self, name_or_path: &str) -> Option<PathBuf> {
        let p = Path::new(name_or_path);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
        if p.is_dir() {
            let candidate = p.join("plugin.lsh");
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        let with_ext = if name_or_path.ends_with(".lsh") {
            PathBuf::from(name_or_path)
        } else {
            PathBuf::from(format!("{name_or_path}.lsh"))
        };
        if with_ext.is_file() {
            return Some(with_ext);
        }

        for base in &self.search_paths {
            let dir_candidate = base.join(name_or_path).join("plugin.lsh");
            if dir_candidate.is_file() {
                return Some(dir_candidate);
            }
            let file_candidate = base.join(format!("{name_or_path}.lsh"));
            if file_candidate.is_file() {
                return Some(file_candidate);
            }
            let exact_candidate = base.join(name_or_path);
            if exact_candidate.is_file() {
                return Some(exact_candidate);
            }
        }

        None
    }
}
