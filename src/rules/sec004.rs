use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::sections::{line_is_meaningful_section_content, section_heading, SectionHeading};
use crate::rules::Rule;

pub struct StackedHeadings;

impl Rule for StackedHeadings {
    fn code(&self) -> &'static str {
        "SEC004"
    }

    fn name(&self) -> &'static str {
        "stacked headings"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut pending_heading: Option<SectionHeading> = None;

        for (index, line) in content.lines().enumerate() {
            if let Some(heading) = section_heading(line, index + 1) {
                if let Some(previous) = pending_heading {
                    if !previous.starred && !heading.starred {
                        diagnostics.push(
                            Diagnostic::new(
                                self.code(),
                                Severity::Warning,
                                "heading follows another heading without intervening text",
                                path,
                                heading.line,
                                heading.column,
                            )
                            .with_hint("add introductory text or remove the extra heading"),
                        );
                    }
                }

                pending_heading = Some(heading);
                continue;
            }

            if pending_heading.is_some() && line_is_meaningful_section_content(line) {
                pending_heading = None;
            }
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::StackedHeadings;

    #[test]
    fn detects_stacked_headings() {
        let content = "\\section{Method}\n\\subsection{Setup}\nText.\n";
        let diagnostics = StackedHeadings.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SEC004");
        assert_eq!(diagnostics[0].line, 2);
    }

    #[test]
    fn labels_and_comments_do_not_break_stacked_headings() {
        let content =
            "\\section{Method}\n\\label{sec:method}\n% note\n\\subsection{Setup}\nText.\n";
        let diagnostics = StackedHeadings.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn text_between_headings_is_accepted() {
        let content = "\\section{Method}\nIntro text.\n\\subsection{Setup}\nText.\n";
        let diagnostics = StackedHeadings.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_starred_heading_pairs() {
        let content = "\\section*{Appendix}\n\\section{Details}\nText.\n";
        let diagnostics = StackedHeadings.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }
}
