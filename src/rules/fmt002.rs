use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

const MAX_CONSECUTIVE_BLANK_LINES: usize = 2;

pub struct RepeatedBlankLines;

impl Rule for RepeatedBlankLines {
    fn code(&self) -> &'static str {
        "FMT002"
    }

    fn name(&self) -> &'static str {
        "repeated blank lines"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut blank_count = 0;

        for (index, line) in content.lines().enumerate() {
            if line.trim_matches([' ', '\t']).is_empty() {
                blank_count += 1;
            } else {
                blank_count = 0;
            }

            if blank_count == MAX_CONSECUTIVE_BLANK_LINES + 1 {
                diagnostics.push(Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    self.name(),
                    path,
                    index + 1,
                    1,
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

    use super::RepeatedBlankLines;

    #[test]
    fn detects_three_blank_lines() {
        let diagnostics = RepeatedBlankLines.check_file(Path::new("paper.tex"), "a\n\n\n\nb\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "FMT002");
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn ignores_two_blank_lines() {
        let diagnostics = RepeatedBlankLines.check_file(Path::new("paper.tex"), "a\n\n\nb\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn treats_whitespace_only_lines_as_blank() {
        let diagnostics = RepeatedBlankLines.check_file(Path::new("paper.tex"), "a\n\n \n\t\nb\n");

        assert_eq!(diagnostics.len(), 1);
    }
}
