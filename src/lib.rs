pub mod artifacts;
pub mod baseline;
pub mod checker;
#[cfg(feature = "native")]
pub mod cli;
pub mod config;
#[cfg(feature = "native")]
pub mod demo;
pub mod diagnostic;
pub mod discovery;
pub mod doctor;
pub mod formatter;
pub mod latex;
pub mod output;
pub mod project;
pub mod project_graph;
pub mod rule_policy;
pub mod rules;
pub mod suggest;
pub mod suggest_ml;
pub mod suppressions;
pub mod web_api;

pub use checker::{run_check, CheckOptions, CheckResult, ToolError};
pub use diagnostic::{Diagnostic, Severity};
