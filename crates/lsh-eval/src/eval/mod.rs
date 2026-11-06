pub mod cmds;
mod command;
pub mod exec;
mod expr;
mod globals;
pub mod hooks;
mod lifecycle;
mod methods;
mod prompt;
pub mod scope;
pub mod statement;
pub mod value;

#[cfg(test)]
mod tests;

pub(crate) use expr::NamedFunctionWithReceiver;
pub use scope::Scope;
pub use statement::{EvalFlow, EvalOutcome};

use std::{
    collections::{HashMap, HashSet},
    env,
    path::PathBuf,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

use hooks::HookRegistry;
use lsh_completions::CompletionRegistry;
use lsh_sys::signals::get_interrupt_flag;

use crate::{eval::value::Value, jobs::JobManager, plugins::PluginManager};

#[derive(Debug)]
pub enum EvalError {
    ScopeNotFound,
}

pub struct Evaluator {
    pub(crate) scopes: Vec<Scope>,
    pub(crate) aliases: HashMap<String, String>,
    pub last_status: i32,
    pub interrupted: Arc<AtomicBool>,
    pub completion_registry: Arc<RwLock<CompletionRegistry>>,
    pub plugin_manager: PluginManager,
    pub hook_registry: Arc<RwLock<HookRegistry>>,
    pub module_cache: Arc<RwLock<HashMap<PathBuf, Value>>>,
    pub exported_vars: HashSet<String>,
    pub job_manager: JobManager,
    pub warned_stopped_jobs: bool,
}

impl Evaluator {
    pub fn new() -> Evaluator {
        let mut exported_vars: HashSet<String> = env::vars().map(|(k, _)| k).collect();
        exported_vars.insert("PATH".to_string());
        exported_vars.insert("PWD".to_string());

        Evaluator {
            scopes: vec![Scope::default()],
            aliases: HashMap::new(),
            last_status: 0,
            interrupted: get_interrupt_flag(),
            completion_registry: Arc::new(RwLock::new(CompletionRegistry::with_defaults())),
            plugin_manager: PluginManager::new(),
            hook_registry: Arc::new(RwLock::new(HookRegistry::new())),
            module_cache: Arc::new(RwLock::new(HashMap::new())),
            exported_vars,
            job_manager: JobManager::new(),
            warned_stopped_jobs: false,
        }
    }

    pub fn with_interrupt_flag(mut self, flag: Arc<AtomicBool>) -> Evaluator {
        self.interrupted = flag;
        self
    }

    pub fn with_completion_registry(
        mut self,
        registry: Arc<RwLock<CompletionRegistry>>,
    ) -> Evaluator {
        self.completion_registry = registry;
        self
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}
