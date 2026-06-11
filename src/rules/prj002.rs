use crate::diagnostic::{Diagnostic, Severity};
use crate::project_graph::ProjectGraph;
use crate::rules::GraphProjectRule;

pub struct AmbiguousRoot;

impl GraphProjectRule for AmbiguousRoot {
    fn code(&self) -> &'static str {
        "PRJ002"
    }

    fn name(&self) -> &'static str {
        "ambiguous root"
    }

    fn check_graph(&self, graph: &ProjectGraph) -> Vec<Diagnostic> {
        if graph.root_candidates.len() <= 1 {
            return Vec::new();
        }

        let candidates: Vec<_> = graph
            .root_candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect();

        vec![Diagnostic::new(
            self.code(),
            Severity::Warning,
            "multiple candidate root .tex files found".to_string(),
            &graph.paper_dir,
            1,
            1,
        )
        .with_hint(format!("candidates: {}", candidates.join(", ")))]
    }
}
