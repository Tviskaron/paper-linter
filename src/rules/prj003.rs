use crate::diagnostic::{Diagnostic, Severity};
use crate::project_graph::ProjectGraph;
use crate::rules::GraphProjectRule;

pub struct RootNotFound;

impl GraphProjectRule for RootNotFound {
    fn code(&self) -> &'static str {
        "PRJ003"
    }

    fn name(&self) -> &'static str {
        "root not found"
    }

    fn check_graph(&self, graph: &ProjectGraph) -> Vec<Diagnostic> {
        if graph.root.is_some() || graph.all_tex.is_empty() {
            return Vec::new();
        }

        vec![Diagnostic::new(
            self.code(),
            Severity::Error,
            "no root .tex file could be resolved in this directory".to_string(),
            &graph.paper_dir,
            1,
            1,
        )
        .with_hint(
            "add a \\documentclass file, 00README.json, or %! root = main.tex magic comment",
        )]
    }
}
