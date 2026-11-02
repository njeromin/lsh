use std::{
    collections::HashMap,
    sync::Arc,
};

use crate::eval::value::FunctionValue;

#[derive(Clone, Default, Debug)]
pub struct HookRegistry {
    hooks: HashMap<String, Vec<Arc<FunctionValue>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    pub fn normalize_event(event: &str) -> String {
        let trimmed = event.trim();
        if let Some(stripped) = trimmed.strip_prefix("on_") {
            stripped.to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn add(&mut self, event: &str, func: Arc<FunctionValue>) {
        let name = Self::normalize_event(event);
        self.hooks.entry(name).or_default().push(func);
    }

    pub fn remove(&mut self, event: &str, target: &Arc<FunctionValue>) -> bool {
        let name = Self::normalize_event(event);
        if let Some(list) = self.hooks.get_mut(&name) {
            let before = list.len();
            list.retain(|f| !Arc::ptr_eq(f, target));
            list.len() < before
        } else {
            false
        }
    }

    pub fn clear(&mut self, event: Option<&str>) {
        if let Some(e) = event {
            let name = Self::normalize_event(e);
            self.hooks.remove(&name);
        } else {
            self.hooks.clear();
        }
    }

    pub fn get_hooks(&self, event: &str) -> Vec<Arc<FunctionValue>> {
        let name = Self::normalize_event(event);
        self.hooks.get(&name).cloned().unwrap_or_default()
    }

    pub fn list(&self) -> HashMap<String, usize> {
        self.hooks
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect()
    }
}
