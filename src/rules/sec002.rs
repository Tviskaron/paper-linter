use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::sections::{line_is_meaningful_section_content, section_heading, SectionHeading};
use crate::rules::Rule;

pub struct EmptySection;

impl Rule for EmptySection {
    fn code(&self) -> &'static str {
        "SEC002"
    }

    fn name(&self) -> &'static str {
        "empty section"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut open_section: Option<SectionHeading> = None;
        let mut has_content = false;

        for (index, line) in content.lines().enumerate() {
            if let Some(heading) = section_heading(line, index + 1) {
                if let Some(previous) = open_section {
                    if !previous.starred && !has_content && heading.level <= previous.level {
                        diagnostics.push(Diagnostic::new(
                            self.code(),
                            Severity::Warning,
                            self.name(),
                            path,
                            previous.line,
                            previous.column,
                        ));
                    }
                }

                open_section = Some(heading);
                has_content = heading.content_after_heading;
                continue;
            }

            if open_section.is_some() && line_is_meaningful_section_content(line) {
                has_content = true;
            }
        }

        if let Some(previous) = open_section {
            if !previous.starred && !has_content {
                diagnostics.push(Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    self.name(),
                    path,
                    previous.line,
                    previous.column,
                ));
            }
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::EmptySection;

    #[test]
    fn detects_empty_section() {
        let content = "\\section{Intro}\n\\label{sec:intro}\n\\section{Method}\nText.\n";
        let diagnostics = EmptySection.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SEC002");
        assert_eq!(diagnostics[0].line, 1);
    }

    #[test]
    fn detects_empty_final_section() {
        let diagnostics =
            EmptySection.check_file(Path::new("paper.tex"), "\\section{Conclusion}\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
    }

    #[test]
    fn ignores_section_with_text() {
        let diagnostics =
            EmptySection.check_file(Path::new("paper.tex"), "\\section{Intro}\nText.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_section_with_inline_text() {
        let diagnostics =
            EmptySection.check_file(Path::new("paper.tex"), "\\section{Intro} Text.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_empty_starred_wrapper_section() {
        let content = "\\section*{Appendix}\n\\label{sec:appendix}\n\\section{Details}\nText.\n";
        let diagnostics = EmptySection.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn allows_parent_section_with_subsection_content() {
        let content = "\\section{Method}\n\\subsection{Setup}\nDetails.\n";
        let diagnostics = EmptySection.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }
}
