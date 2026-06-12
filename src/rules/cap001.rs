use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct MissingCaption;

impl ProjectRule for MissingCaption {
    fn code(&self) -> &'static str {
        "CAP001"
    }

    fn name(&self) -> &'static str {
        "caption-missing"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .floats
            .iter()
            .filter(|float| !is_subfloat(&float.env_name))
            .filter(|float| float.captions.is_empty() && !float.has_nested_caption)
            .map(|float| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!("{} has no caption", float.kind.as_str()),
                    &float.location.file,
                    float.location.line,
                    float.location.column,
                )
                .with_hint("add \\caption{...}")
            })
            .collect()
    }
}

fn is_subfloat(env_name: &str) -> bool {
    matches!(env_name, "subfigure" | "subtable")
}
