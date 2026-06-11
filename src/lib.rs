pub mod checker;
pub mod cli;
pub mod diagnostic;
pub mod discovery;
pub mod latex;
pub mod output;
pub mod project;
pub mod rules;

pub use checker::{run_check, CheckOptions, CheckResult, ToolError};
pub use diagnostic::{Diagnostic, Severity};
