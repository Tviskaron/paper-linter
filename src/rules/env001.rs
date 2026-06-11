use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::environments::{environment_events, EnvironmentEvent, EnvironmentEventKind};
use crate::rules::Rule;

pub struct EnvironmentMismatch;

impl Rule for EnvironmentMismatch {
    fn code(&self) -> &'static str {
        "ENV001"
    }

    fn name(&self) -> &'static str {
        "environment begin/end mismatch"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut stack: Vec<EnvironmentEvent> = Vec::new();

        for event in environment_events(content) {
            if is_ignored_environment(&event.name) {
                continue;
            }

            match event.kind {
                EnvironmentEventKind::Begin => stack.push(event),
                EnvironmentEventKind::End => {
                    let Some(open) = stack.pop() else {
                        diagnostics.push(diagnostic(self, path, &event));
                        continue;
                    };

                    if open.name != event.name {
                        diagnostics.push(diagnostic(self, path, &event));
                    }
                }
            }
        }

        diagnostics.extend(
            stack
                .iter()
                .rev()
                .map(|event| diagnostic(self, path, event)),
        );
        diagnostics
    }
}

fn is_ignored_environment(name: &str) -> bool {
    matches!(name, "document" | "list")
}

fn diagnostic(rule: &EnvironmentMismatch, path: &Path, event: &EnvironmentEvent) -> Diagnostic {
    Diagnostic::new(
        rule.code(),
        Severity::Error,
        rule.name(),
        path,
        event.line,
        event.column,
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::diagnostic::Severity;
    use crate::rules::Rule;

    use super::EnvironmentMismatch;

    #[test]
    fn detects_mismatched_environment() {
        let diagnostics = EnvironmentMismatch
            .check_file(Path::new("paper.tex"), "\\begin{figure}\n\\end{table}\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "ENV001");
        assert_eq!(diagnostics[0].severity, Severity::Error);
        assert_eq!(diagnostics[0].line, 2);
    }

    #[test]
    fn detects_unclosed_environment() {
        let diagnostics =
            EnvironmentMismatch.check_file(Path::new("paper.tex"), "\\begin{figure}\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
    }

    #[test]
    fn ignores_balanced_environment() {
        let diagnostics = EnvironmentMismatch
            .check_file(Path::new("paper.tex"), "\\begin{figure}\n\\end{figure}\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_document_environment_boundaries() {
        let diagnostics =
            EnvironmentMismatch.check_file(Path::new("fragment.tex"), "\\end{document}\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_internal_list_environment() {
        let diagnostics = EnvironmentMismatch.check_file(
            Path::new("paper.tex"),
            "\\begin{abstract}\n\\end{list}\n\\end{abstract}\n",
        );

        assert!(diagnostics.is_empty());
    }
}
