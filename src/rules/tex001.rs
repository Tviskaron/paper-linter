use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::commands::commands_in_line;
use crate::rules::Rule;

pub struct MissingNonBreakingSpace;

impl Rule for MissingNonBreakingSpace {
    fn code(&self) -> &'static str {
        "TEX001"
    }

    fn name(&self) -> &'static str {
        "missing non-breaking space before reference or citation"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (index, line) in content.lines().enumerate() {
            if !line
                .trim_start()
                .starts_with(|character: char| character.is_ascii_alphabetic())
            {
                continue;
            }

            for command in commands_in_line(line, index + 1) {
                if !is_reference_or_citation_command(&command.name) {
                    continue;
                }

                if has_breaking_space_before_command(line, command.column) {
                    diagnostics.push(Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        self.name(),
                        path,
                        command.line,
                        command.column,
                    ));
                }
            }
        }

        diagnostics
    }
}

fn is_reference_or_citation_command(command: &str) -> bool {
    matches!(
        command,
        "cite"
            | "citep"
            | "citet"
            | "citealp"
            | "autocite"
            | "parencite"
            | "ref"
            | "eqref"
            | "autoref"
            | "cref"
            | "Cref"
    )
}

fn has_breaking_space_before_command(line: &str, command_column: usize) -> bool {
    let command_byte_index = column_to_byte_index(line, command_column);
    let prefix = &line[..command_byte_index];
    let mut chars = prefix.chars().rev();

    let Some(previous) = chars.next() else {
        return false;
    };

    if !previous.is_whitespace() {
        return false;
    }

    chars_before_space(prefix).is_some_and(is_prose_reference_prefix)
}

fn chars_before_space(prefix: &str) -> Option<char> {
    prefix.trim_end().chars().last()
}

fn is_prose_reference_prefix(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, ')' | ']' | '}')
}

fn column_to_byte_index(line: &str, column: usize) -> usize {
    line.char_indices()
        .nth(column.saturating_sub(1))
        .map_or(line.len(), |(index, _)| index)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::MissingNonBreakingSpace;

    #[test]
    fn detects_space_before_citation() {
        let diagnostics =
            MissingNonBreakingSpace.check_file(Path::new("paper.tex"), "Prior work \\cite{a}.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TEX001");
        assert_eq!(diagnostics[0].column, 12);
    }

    #[test]
    fn detects_space_before_reference() {
        let diagnostics = MissingNonBreakingSpace
            .check_file(Path::new("paper.tex"), "See Figure \\ref{fig:a}.\n");

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn accepts_non_breaking_space_before_reference() {
        let diagnostics = MissingNonBreakingSpace
            .check_file(Path::new("paper.tex"), "See Figure~\\ref{fig:a}.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_command_lines() {
        let diagnostics = MissingNonBreakingSpace
            .check_file(Path::new("paper.tex"), "\\caption{See \\ref{fig:a}}\n");

        assert!(diagnostics.is_empty());
    }
}
