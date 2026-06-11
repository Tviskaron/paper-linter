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
        let mut diagnostics = Vec::new();
        let mut definition_brace_depth = 0i32;

        for (index, line) in content.lines().enumerate() {
            let active_line = uncommented_line(line);

            if definition_brace_depth > 0 {
                definition_brace_depth += brace_delta(active_line);
                definition_brace_depth = definition_brace_depth.max(0);
                continue;
            }

            if is_macro_definition(active_line) {
                definition_brace_depth = brace_delta(active_line).max(0);
                continue;
            }

            if let Some(column) = placeholder_column(active_line) {
                diagnostics.push(Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    self.name(),
                    path,
                    index + 1,
                    column,
                ));
            }
        }

        diagnostics
    }
}

fn placeholder_column(line: &str) -> Option<usize> {
    let line = uncommented_line(line);
    if is_macro_definition(line) {
        return None;
    }

    let lower = line.to_ascii_lowercase();
    let word_patterns = ["todo", "tbd", "fixme", "lorem"];
    let phrase_patterns = ["???", "citation needed", "add reference"];

    word_patterns
        .iter()
        .filter_map(|pattern| find_word_placeholder(&lower, pattern))
        .chain(
            phrase_patterns
                .iter()
                .filter_map(|pattern| lower.find(pattern)),
        )
        .map(|index| byte_to_column(line, index))
        .min()
}

fn uncommented_line(line: &str) -> &str {
    let mut escaped = false;

    for (index, ch) in line.char_indices() {
        if ch == '%' && !escaped {
            return &line[..index];
        }

        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }
    }

    line
}

fn is_macro_definition(line: &str) -> bool {
    let trimmed = line.trim_start();
    [
        "\\def",
        "\\gdef",
        "\\edef",
        "\\xdef",
        "\\newcommand",
        "\\renewcommand",
        "\\providecommand",
        "\\DeclareRobustCommand",
    ]
    .iter()
    .any(|command| trimmed.starts_with(command))
}

fn find_word_placeholder(line: &str, pattern: &str) -> Option<usize> {
    let mut search_start = 0;

    while let Some(relative_index) = line[search_start..].find(pattern) {
        let index = search_start + relative_index;
        let end = index + pattern.len();

        if is_word_placeholder_match(line, index, end) {
            return Some(index);
        }

        search_start = end;
    }

    None
}

fn is_word_placeholder_match(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();

    before != Some('\\') && !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0;
    let mut escaped = false;

    for ch in line.chars() {
        if !escaped {
            match ch {
                '{' => delta += 1,
                '}' => delta -= 1,
                _ => {}
            }
        }

        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }
    }

    delta
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

    #[test]
    fn ignores_commented_placeholders() {
        let diagnostics =
            PlaceholderText.check_file(Path::new("paper.tex"), "% TODO: revise this\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_todo_command_names() {
        let diagnostics = PlaceholderText.check_file(
            Path::new("paper.tex"),
            "\\usepackage{todonotes}\n\\todo{draft note}\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_macro_definition_placeholders() {
        let diagnostics = PlaceholderText.check_file(
            Path::new("paper.tex"),
            "\\newcommand{\\todo}[1]{\\textbf{TODO: #1}}\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_multiline_macro_definition_placeholders() {
        let diagnostics = PlaceholderText.check_file(
            Path::new("paper.tex"),
            "\\newcommand{\\todopa}[1]{\nTODO: describe the paper.\n}\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_normal_rewrite_this_phrase() {
        let diagnostics = PlaceholderText.check_file(
            Path::new("paper.tex"),
            "Let us now rewrite this equation using the identity.\n",
        );

        assert!(diagnostics.is_empty());
    }
}
