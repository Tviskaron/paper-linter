use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::FloatKind;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct OrphanAlgorithm;

impl ProjectRule for OrphanAlgorithm {
    fn code(&self) -> &'static str {
        "ALG001"
    }

    fn name(&self) -> &'static str {
        "orphan-algorithm"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .floats
            .iter()
            .filter(|float| float.kind == FloatKind::Algorithm)
            .flat_map(|float| float.labels.iter())
            .filter(|label| !project.is_referenced(&label.key))
            .map(|label| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!("algorithm '{}' is never referenced", label.key),
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
