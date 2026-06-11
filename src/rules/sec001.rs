use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::sections::section_heading;
use crate::rules::Rule;

pub struct SkippedSectionLevel;

impl Rule for SkippedSectionLevel {
    fn code(&self) -> &'static str {
        "SEC001"
    }

    fn name(&self) -> &'static str {
        "skipped section hierarchy level"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut previous_level = None;

        for (index, line) in content.lines().enumerate() {
            let Some(heading) = section_heading(line, index + 1) else {
                continue;
            };

            if let Some(level) = previous_level {
                if heading.level > level + 1 {
                    diagnostics.push(Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        self.name(),
                        path,
                        heading.line,
                        heading.column,
                    ));
                }
            }

            previous_level = Some(heading.level);
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::SkippedSectionLevel;

    #[test]
    fn detects_skipped_section_level() {
        let content = "\\section{Intro}\n\\subsubsection{Details}\n";
        let diagnostics = SkippedSectionLevel.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SEC001");
        assert_eq!(diagnostics[0].line, 2);
    }

    #[test]
    fn allows_adjacent_section_levels() {
        let content = "\\section{Intro}\n\\subsection{Setup}\n\\subsubsection{Details}\n";
        let diagnostics = SkippedSectionLevel.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_warn_on_file_starting_with_subsection() {
        let diagnostics =
            SkippedSectionLevel.check_file(Path::new("section.tex"), "\\subsection{Setup}\n");

        assert!(diagnostics.is_empty());
    }
}
