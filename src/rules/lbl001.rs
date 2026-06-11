use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::LabelKind;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct UnusedLabel;

impl ProjectRule for UnusedLabel {
    fn code(&self) -> &'static str {
        "LBL001"
    }

    fn name(&self) -> &'static str {
        "label-unused"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .labels
            .iter()
            .filter(|label| label.kind == LabelKind::Other)
            .filter(|label| !project.is_referenced(&label.key))
            .map(|label| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!("label '{}' is never referenced", label.key),
                    &label.location.file,
                    label.location.line,
                    label.location.column,
                )
            })
            .collect()
    }
}
