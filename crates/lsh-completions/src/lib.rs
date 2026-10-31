pub mod defaults;
pub use defaults as r#default;
pub mod registry;
pub mod types;

pub use defaults::populate_default_rules;
pub use registry::CompletionRegistry;
pub use types::{
    CandidateSpec, CompletionItem, CompletionKind, CompletionResolution, CompletionRule,
};
