use crate::diagnostic::{Diagnostic, Severity};
use crate::project_graph::ProjectGraph;
use crate::rules::GraphProjectRule;

pub struct MissingInclude;

impl GraphProjectRule for MissingInclude {
    fn code(&self) -> &'static str {
        "PRJ001"
    }

    fn name(&self) -> &'static str {
        "missing include"
    }

    fn check_graph(&self, graph: &ProjectGraph) -> Vec<Diagnostic> {
        graph
            .missing_includes
            .iter()
            .map(|missing| {
                Diagnostic::new(
                    self.code(),
                    Severity::Error,
                    format!("include target '{}' not found", missing.raw_path),
                    &missing.file,
                    missing.line,
                    missing.column,
                )
                .with_hint("check the \\input, \\include, or \\subfile path")
            })
            .collect()
    }
}
