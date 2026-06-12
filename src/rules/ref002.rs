use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::Label;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct DuplicateLabel;

impl ProjectRule for DuplicateLabel {
    fn code(&self) -> &'static str {
        "REF002"
    }

    fn name(&self) -> &'static str {
        "label-duplicate"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        duplicate_labels(&project.labels)
            .into_iter()
            .map(|label| {
                Diagnostic::new(
                    self.code(),
                    Severity::Error,
                    format!("label '{}' is defined more than once", label.key),
                    &label.location.file,
                    label.location.line,
                    label.location.column,
                )
                .with_hint("keep one definition for this label or rename one of the labels and matching references")
            })
            .collect()
    }
}

fn duplicate_labels(labels: &[Label]) -> Vec<&Label> {
    let mut first_by_key = BTreeMap::new();
    let mut duplicates = Vec::new();

    for label in labels {
        if !is_static_label_key(&label.key) {
            continue;
        }

        if first_by_key.contains_key(label.key.as_str()) {
            duplicates.push(label);
        } else {
            first_by_key.insert(label.key.as_str(), label);
        }
    }

    duplicates
}

fn is_static_label_key(key: &str) -> bool {
    !key.contains('#')
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::latex::scan::{Label, LabelKind, SourceLocation};

    use super::duplicate_labels;

    fn label(key: &str, line: usize) -> Label {
        Label {
            key: key.to_string(),
            kind: LabelKind::Other,
            location: SourceLocation {
                file: PathBuf::from("paper.tex"),
                line,
                column: 1,
            },
        }
    }

    #[test]
    fn reports_duplicate_static_labels_after_first_definition() {
        let labels = vec![
            label("sec:intro", 1),
            label("fig:main", 2),
            label("sec:intro", 3),
        ];

        let duplicates = duplicate_labels(&labels);

        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].key, "sec:intro");
        assert_eq!(duplicates[0].location.line, 3);
    }

    #[test]
    fn ignores_macro_parameter_labels() {
        let labels = vec![label("fig:#1", 1), label("fig:#1", 2)];

        assert!(duplicate_labels(&labels).is_empty());
    }
}
