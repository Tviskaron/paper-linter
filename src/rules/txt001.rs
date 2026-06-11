use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct PlaceholderText;

impl Rule for PlaceholderText {
    fn code(&self) -> &'static str {
        "TXT001"
    }

    fn name(&self) -> &'static str {
        "placeholder text"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                placeholder_column(line).map(|column| {
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

fn placeholder_column(line: &str) -> Option<usize> {
    let lower = line.to_ascii_lowercase();
    let patterns = [
        "todo",
        "tbd",
        "fixme",
        "lorem",
        "???",
        "citation needed",
        "add reference",
        "rewrite this",
    ];

    patterns
        .iter()
        .filter_map(|pattern| lower.find(pattern).map(|index| byte_to_column(line, index)))
        .min()
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::PlaceholderText;

    #[test]
    fn detects_todo_markers() {
        let diagnostics = PlaceholderText.check_file(Path::new("paper.tex"), "Text TODO later\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TXT001");
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 6);
    }

    #[test]
    fn detects_case_insensitive_lorem() {
        let diagnostics = PlaceholderText.check_file(Path::new("paper.tex"), "Lorem ipsum\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].column, 1);
    }

    #[test]
    fn ignores_clean_text() {
        let diagnostics = PlaceholderText.check_file(Path::new("paper.tex"), "Finished text.\n");

        assert!(diagnostics.is_empty());
    }
}
