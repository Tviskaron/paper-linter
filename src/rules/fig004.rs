use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::FloatKind;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct MissingFigureLabel;

impl ProjectRule for MissingFigureLabel {
    fn code(&self) -> &'static str {
        "FIG004"
    }

    fn name(&self) -> &'static str {
        "figure-label-missing"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .floats
            .iter()
            .filter(|env| env.kind == FloatKind::Figure)
            .filter(|env| env.labels.is_empty())
            .filter(|env| !env.captions.is_empty() || !env.graphics.is_empty())
            .map(|env| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    "figure has no label",
                    &env.location.file,
                    env.location.line,
                    env.location.column,
                )
                .with_hint("add \\label{fig:...} after \\caption{...}")
            })
            .collect()
    }
}
