use std::collections::HashMap;
use std::fmt;

use lsh_eval::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Default,
    Emacs,
    Vi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionType {
    Menu,
    List,
    Circular,
}

impl fmt::Display for CompletionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompletionType::Menu => write!(f, "menu"),
            CompletionType::List => write!(f, "list"),
            CompletionType::Circular => write!(f, "circular"),
        }
    }
}

impl From<&str> for CompletionType {
    fn from(s: &str) -> Self {
        match s {
            "circular" => CompletionType::Circular,
            "list" => CompletionType::List,
            _ => CompletionType::Menu,
        }
    }
}

impl From<CompletionType> for rustyline::CompletionType {
    fn from(value: CompletionType) -> Self {
        match value {
            CompletionType::Menu | CompletionType::List => rustyline::CompletionType::List,
            CompletionType::Circular => rustyline::CompletionType::Circular,
        }
    }
}

impl fmt::Display for EditMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditMode::Default => write!(f, "default"),
            EditMode::Emacs => write!(f, "emacs"),
            EditMode::Vi => write!(f, "vi"),
        }
    }
}

impl From<&str> for EditMode {
    fn from(s: &str) -> Self {
        match s {
            "default" => EditMode::Default,
            "emacs" => EditMode::Emacs,
            "vi" => EditMode::Vi,
            _ => EditMode::Default,
        }
    }
}

impl From<EditMode> for rustyline::EditMode {
    fn from(value: EditMode) -> Self {
        match value {
            EditMode::Default => rustyline::EditMode::Vi,
            EditMode::Emacs => rustyline::EditMode::Emacs,
            EditMode::Vi => rustyline::EditMode::Vi,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub tab_stop: u8,
    pub edit_mode: EditMode,
    pub completion_type: CompletionType,
    pub nerd_icons: bool,
}

impl Default for Options {
    fn default() -> Self {
        let nerd_icons = std::env::var("LSH_NERD_ICONS")
            .or_else(|_| std::env::var("NERD_ICONS"))
            .map(|v| {
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("yes")
                    || v.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false);

        Self {
            tab_stop: 4,
            edit_mode: EditMode::Default,
            completion_type: CompletionType::Menu,
            nerd_icons,
        }
    }
}

impl From<Options> for HashMap<String, Value> {
    fn from(value: Options) -> Self {
        let mut map = HashMap::new();

        map.insert("tab_stop".to_string(), Value::Number(value.tab_stop as f64));
        map.insert(
            "edit_mode".to_string(),
            Value::String(value.edit_mode.to_string()),
        );
        map.insert(
            "completion_type".to_string(),
            Value::String(value.completion_type.to_string()),
        );
        map.insert("nerd_icons".to_string(), Value::Bool(value.nerd_icons));

        map
    }
}

impl From<Options> for Value {
    fn from(value: Options) -> Self {
        let map: HashMap<String, Value> = value.into();
        Value::from(map)
    }
}

impl From<&Value> for Options {
    fn from(value: &Value) -> Self {
        match value {
            Value::Object(map) => {
                let map = match map.read() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                Options::from(&*map)
            }
            _ => Options::default(),
        }
    }
}

impl From<Value> for Options {
    fn from(value: Value) -> Self {
        Options::from(&value)
    }
}

impl From<&HashMap<String, Value>> for Options {
    fn from(map: &HashMap<String, Value>) -> Self {
        let tab_stop = map
            .get("tab_stop")
            .and_then(|v| match v {
                Value::Number(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(4.0) as u8;

        let edit_mode = map
            .get("edit_mode")
            .and_then(|v| match v {
                Value::String(s) => Some(EditMode::from(s.as_str())),
                _ => None,
            })
            .unwrap_or(EditMode::Default);

        let completion_type = map
            .get("completion_type")
            .and_then(|v| match v {
                Value::String(s) => Some(CompletionType::from(s.as_str())),
                _ => None,
            })
            .unwrap_or(CompletionType::Menu);

        let nerd_icons = map
            .get("nerd_icons")
            .or_else(|| map.get("icons"))
            .and_then(|v| match v {
                Value::Bool(b) => Some(*b),
                Value::String(s) => match s.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => Some(true),
                    "false" | "0" | "no" | "off" => Some(false),
                    _ => None,
                },
                Value::Number(n) => Some(*n != 0.0),
                _ => None,
            })
            .unwrap_or_else(|| {
                std::env::var("LSH_NERD_ICONS")
                    .or_else(|_| std::env::var("NERD_ICONS"))
                    .map(|v| {
                        v == "1"
                            || v.eq_ignore_ascii_case("true")
                            || v.eq_ignore_ascii_case("yes")
                            || v.eq_ignore_ascii_case("on")
                    })
                    .unwrap_or(false)
            });

        Self {
            tab_stop,
            edit_mode,
            completion_type,
            nerd_icons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_has_nerd_icons_field() {
        let default_opts = Options::default();
        assert_eq!(default_opts.tab_stop, 4);
        assert_eq!(default_opts.edit_mode, EditMode::Default);
        assert_eq!(default_opts.completion_type, CompletionType::Menu);
    }

    #[test]
    fn options_converts_to_and_from_hashmap() {
        let opts = Options {
            tab_stop: 8,
            edit_mode: EditMode::Vi,
            completion_type: CompletionType::List,
            nerd_icons: true,
        };

        let map: HashMap<String, Value> = opts.clone().into();
        assert_eq!(map.get("tab_stop"), Some(&Value::Number(8.0)));
        assert_eq!(map.get("edit_mode"), Some(&Value::String("vi".to_string())));
        assert_eq!(
            map.get("completion_type"),
            Some(&Value::String("list".to_string()))
        );
        assert_eq!(map.get("nerd_icons"), Some(&Value::Bool(true)));

        let parsed = Options::from(&map);
        assert_eq!(parsed, opts);
    }

    #[test]
    fn options_parses_nerd_icons_from_strings_and_numbers() {
        let mut map = HashMap::new();
        map.insert("nerd_icons".to_string(), Value::String("true".to_string()));
        assert!(Options::from(&map).nerd_icons);

        map.insert("nerd_icons".to_string(), Value::String("on".to_string()));
        assert!(Options::from(&map).nerd_icons);

        map.insert("nerd_icons".to_string(), Value::String("0".to_string()));
        assert!(!Options::from(&map).nerd_icons);

        map.insert("nerd_icons".to_string(), Value::Number(1.0));
        assert!(Options::from(&map).nerd_icons);

        map.insert("nerd_icons".to_string(), Value::Number(0.0));
        assert!(!Options::from(&map).nerd_icons);

        // Alias "icons" works too
        let mut map_alias = HashMap::new();
        map_alias.insert("icons".to_string(), Value::Bool(true));
        assert!(Options::from(&map_alias).nerd_icons);
    }
}
