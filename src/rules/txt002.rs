use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct RepeatedWords;

impl Rule for RepeatedWords {
    fn code(&self) -> &'static str {
        "TXT002"
    }

    fn name(&self) -> &'static str {
        "repeated word"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                repeated_word_column(line).map(|column| {
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

fn repeated_word_column(line: &str) -> Option<usize> {
    if line.contains('&') {
        return None;
    }

    if !line
        .trim_start()
        .starts_with(|character: char| character.is_ascii_alphabetic())
    {
        return None;
    }

    let mut previous: Option<(String, usize)> = None;

    for (word, column) in words_outside_math_and_comments(line) {
        if word.len() == 1 {
            previous = None;
            continue;
        }

        let normalized = word.to_ascii_lowercase();
        if previous
            .as_ref()
            .is_some_and(|(previous_word, _)| previous_word == &normalized)
        {
            return Some(column);
        }

        previous = Some((normalized, column));
    }

    None
}

fn words_outside_math_and_comments(line: &str) -> Vec<(&str, usize)> {
    let mut words = Vec::new();
    let mut start = None;
    let mut in_math = false;
    let mut previous = None;

    for (index, character) in line.char_indices() {
        if character == '%' && previous != Some('\\') {
            break;
        }

        if character == '$' && previous != Some('\\') {
            if let Some(start_index) = start.take() {
                words.push((&line[start_index..index], byte_to_column(line, start_index)));
            }
            in_math = !in_math;
            previous = Some(character);
            continue;
        }

        if in_math || character == '\\' {
            if let Some(start_index) = start.take() {
                words.push((&line[start_index..index], byte_to_column(line, start_index)));
            }
            previous = Some(character);
            continue;
        }

        if character.is_ascii_alphabetic() {
            start.get_or_insert(index);
        } else if let Some(start_index) = start.take() {
            words.push((&line[start_index..index], byte_to_column(line, start_index)));
        }

        previous = Some(character);
    }

    if let Some(start_index) = start {
        words.push((&line[start_index..], byte_to_column(line, start_index)));
    }

    words
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::RepeatedWords;

    #[test]
    fn detects_repeated_words() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "This result result is useful.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TXT002");
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 13);
    }

    #[test]
    fn ignores_case_difference_when_repeated() {
        let diagnostics = RepeatedWords.check_file(Path::new("paper.tex"), "The the result\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].column, 5);
    }

    #[test]
    fn ignores_clean_text() {
        let diagnostics = RepeatedWords.check_file(Path::new("paper.tex"), "This is useful.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_words_inside_math_spans() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "$x_i = x_i + 1$ is useful.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_table_rows() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "Agents & Success & Success & Collisions \\\\\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_comments() {
        let diagnostics = RepeatedWords.check_file(Path::new("paper.tex"), "Text % TODO the the\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_command_lines() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "\\begin{tabular}{c cc cc}\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_single_letter_repeats() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "Models include 2M, 6M, and 85M.\n");

        assert!(diagnostics.is_empty());
    }
}
