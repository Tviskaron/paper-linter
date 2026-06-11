use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct TrailingWhitespace;

impl Rule for TrailingWhitespace {
    fn code(&self) -> &'static str {
        "WS001"
    }

    fn name(&self) -> &'static str {
        "trailing whitespace"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        content
            .split_inclusive('\n')
            .enumerate()
            .filter_map(|(index, raw_line)| {
                let line = raw_line.trim_end_matches('\n').trim_end_matches('\r');
                trailing_whitespace_column(line).map(|column| {
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        self.name(),
                        path,
                        index + 1,
                        column,
                    )
                })
            })
            .collect()
    }
}

fn trailing_whitespace_column(line: &str) -> Option<usize> {
    let trimmed = line.trim_end_matches([' ', '\t']);
    if trimmed.len() == line.len() {
        return None;
    }

    Some(trimmed.chars().count() + 1)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::diagnostic::Severity;
    use crate::rules::Rule;

    use super::TrailingWhitespace;

    #[test]
    fn detects_spaces_before_newline() {
        let diagnostics = TrailingWhitespace.check_file(Path::new("paper.tex"), "hello  \n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "WS001");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 6);
    }

    #[test]
    fn detects_tabs_before_newline() {
        let diagnostics = TrailingWhitespace.check_file(Path::new("paper.tex"), "hello\t\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 6);
    }

    #[test]
    fn ignores_clean_lines() {
        let diagnostics = TrailingWhitespace.check_file(Path::new("paper.tex"), "hello\nworld\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_correct_line_and_column() {
        let diagnostics =
            TrailingWhitespace.check_file(Path::new("paper.tex"), "first\nsecond \nthird\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].column, 7);
    }
}
