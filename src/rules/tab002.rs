use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::FloatKind;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct MissingTableLabel;

impl ProjectRule for MissingTableLabel {
    fn code(&self) -> &'static str {
        "TAB002"
    }

    fn name(&self) -> &'static str {
        "table-label-missing"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .floats
            .iter()
            .filter(|env| env.kind == FloatKind::Table)
            .filter(|env| env.labels.is_empty())
            .filter(|env| !env.captions.is_empty())
            .map(|env| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    "table has no label",
                    &env.location.file,
                    env.location.line,
                    env.location.column,
                )
                .with_hint("add \\label{tab:...} after \\caption{...}")
            })
            .collect()
    }
}
