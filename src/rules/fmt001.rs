use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct MissingFinalNewline;

impl Rule for MissingFinalNewline {
    fn code(&self) -> &'static str {
        "FMT001"
    }

    fn name(&self) -> &'static str {
        "missing final newline"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        if content.is_empty() || content.ends_with('\n') {
            return Vec::new();
        }

        let line = content.lines().count();
        let column = content
            .lines()
            .last()
            .map_or(1, |line| line.chars().count() + 1);

        vec![Diagnostic::new(
            self.code(),
            Severity::Warning,
            self.name(),
            path,
            line,
            column,
        )]
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::MissingFinalNewline;

    #[test]
    fn detects_missing_final_newline() {
        let diagnostics = MissingFinalNewline.check_file(Path::new("paper.tex"), "hello");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "FMT001");
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 6);
    }

    #[test]
    fn ignores_file_with_final_newline() {
        let diagnostics = MissingFinalNewline.check_file(Path::new("paper.tex"), "hello\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_empty_file() {
        let diagnostics = MissingFinalNewline.check_file(Path::new("paper.tex"), "");

        assert!(diagnostics.is_empty());
    }
}
