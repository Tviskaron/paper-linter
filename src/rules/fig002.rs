use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::FloatKind;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct OrphanFigure;

impl ProjectRule for OrphanFigure {
    fn code(&self) -> &'static str {
        "FIG002"
    }

    fn name(&self) -> &'static str {
        "orphan-figure"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .floats
            .iter()
            .filter(|float| float.kind == FloatKind::Figure)
            .flat_map(|float| float.labels.iter())
            .filter(|label| !project.is_referenced(&label.key))
            .map(|label| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!("figure '{}' is never referenced", label.key),
                    &label.location.file,
                    label.location.line,
                    label.location.column,
                )
                .with_hint(format!(
                    "reference it with \\ref{{{}}} or suppress this diagnostic",
                    label.key
                ))
            })
            .collect()
    }
}
