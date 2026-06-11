use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct MissingAsset;

impl ProjectRule for MissingAsset {
    fn code(&self) -> &'static str {
        "FIG001"
    }

    fn name(&self) -> &'static str {
        "asset-missing"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .graphics
            .iter()
            .filter(|graphic| project.resolve_graphic(graphic).is_none())
            .map(|graphic| {
                Diagnostic::new(
                    self.code(),
                    Severity::Error,
                    format!("asset '{}' not found", graphic.raw_path),
                    &graphic.location.file,
                    graphic.location.line,
                    graphic.location.column,
                )
                .with_hint("check the path or add the file")
            })
            .collect()
    }
}
