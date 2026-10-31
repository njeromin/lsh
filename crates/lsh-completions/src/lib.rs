pub mod defaults;
pub mod registry;
pub mod types;

pub use defaults::populate_default_rules;
pub use registry::CompletionRegistry;
pub use types::{
    CandidateSpec, CompletionItem, CompletionKind, CompletionResolution, CompletionRule,
};
