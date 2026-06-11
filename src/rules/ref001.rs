use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::Ref;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct MissingReferenceTarget;

impl ProjectRule for MissingReferenceTarget {
    fn code(&self) -> &'static str {
        "REF001"
    }

    fn name(&self) -> &'static str {
        "ref-missing"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .refs
            .iter()
            .filter(|reference| is_static_ref_key(&reference.key))
            .filter(|reference| !is_package_generated_reference(project, reference))
            .filter(|reference| !project.has_label(&reference.key))
            .map(|reference| {
                Diagnostic::new(
                    self.code(),
                    Severity::Error,
                    format!(
                        "{} target '{}' does not exist",
                        reference.command, reference.key
                    ),
                    &reference.location.file,
                    reference.location.line,
                    reference.location.column,
                )
                .with_hint(format!(
                    "add \\label{{{}}} or fix the reference key",
                    reference.key
                ))
            })
            .collect()
    }
}

fn is_static_ref_key(key: &str) -> bool {
    !key.contains('#')
}

fn is_package_generated_reference(project: &ProjectIndex, reference: &Ref) -> bool {
    reference.command == "pageref"
        && reference.key == "LastPage"
        && project.uses_package("lastpage")
}

#[cfg(test)]
mod tests {
    use super::is_static_ref_key;

    #[test]
    fn macro_parameter_keys_are_not_static_refs() {
        assert!(!is_static_ref_key("#1"));
        assert!(!is_static_ref_key("fig:#1"));
        assert!(is_static_ref_key("fig:model"));
    }
}
