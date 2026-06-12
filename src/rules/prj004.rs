use crate::diagnostic::{Diagnostic, Severity};
use crate::project_graph::{is_likely_alternate_or_service_tex, ProjectGraph};
use crate::rules::GraphProjectRule;

pub struct OrphanTex;

impl GraphProjectRule for OrphanTex {
    fn code(&self) -> &'static str {
        "PRJ004"
    }

    fn name(&self) -> &'static str {
        "orphan tex"
    }

    fn check_graph(&self, graph: &ProjectGraph) -> Vec<Diagnostic> {
        if graph.root.is_none() {
            return Vec::new();
        }

        graph
            .all_tex
            .iter()
            .filter(|path| !graph.reachable.contains(*path))
            .filter(|path| !is_likely_alternate_or_service_tex(path))
            .map(|path| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    "this .tex file is not reachable from the project root".to_string(),
                    path,
                    1,
                    1,
                )
                .with_hint("include it from the root document or remove the stray file")
            })
            .collect()
    }
}
