use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct DoubleDollarDisplayMath;

impl Rule for DoubleDollarDisplayMath {
    fn code(&self) -> &'static str {
        "MTH001"
    }

    fn name(&self) -> &'static str {
        "double-dollar display math"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut verbatim_depth = 0usize;
        let mut in_double_dollar_math = false;

        for (index, raw_line) in content.lines().enumerate() {
            let line_number = index + 1;
            let line = uncommented_line(raw_line);
            let scan_line = verbatim_depth == 0;
            let next_verbatim_depth = update_verbatim_depth(line, verbatim_depth);

            if scan_line && next_verbatim_depth == 0 {
                for column in double_dollar_columns(line) {
                    if !in_double_dollar_math {
                        diagnostics.push(
                            Diagnostic::new(
                                self.code(),
                                Severity::Warning,
                                "avoid $$ display math",
                                path,
                                line_number,
                                column,
                            )
                            .with_hint("use \\[...\\] or a display math environment"),
                        );
                    }
                    in_double_dollar_math = !in_double_dollar_math;
                }
            }

            verbatim_depth = next_verbatim_depth;
        }

        diagnostics
    }
}

fn double_dollar_columns(line: &str) -> impl Iterator<Item = usize> + '_ {
    let mut columns = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;

    while index + 1 < bytes.len() {
        if bytes[index] == b'$' && bytes[index + 1] == b'$' && !is_escaped(line, index) {
            columns.push(byte_to_column(line, index));
            index += 2;
        } else {
            index += 1;
        }
    }

    columns.into_iter()
}

fn uncommented_line(line: &str) -> &str {
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if character == '%' && !escaped {
            return &line[..index];
        }

        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }

    line
}

fn update_verbatim_depth(line: &str, mut depth: usize) -> usize {
    let mut search_start = 0;

    while let Some(relative_index) = line[search_start..].find('\\') {
        let index = search_start + relative_index;
        if let Some((begin, name, end)) = environment_event(line, index) {
            if is_verbatim_environment(name) {
                if begin {
                    depth += 1;
                } else {
                    depth = depth.saturating_sub(1);
                }
            }
            search_start = end;
        } else {
            search_start = index + 1;
        }
    }

    depth
}

fn environment_event(line: &str, index: usize) -> Option<(bool, &str, usize)> {
    let (begin, rest) = if let Some(rest) = line[index..].strip_prefix("\\begin{") {
        (true, rest)
    } else if let Some(rest) = line[index..].strip_prefix("\\end{") {
        (false, rest)
    } else {
        return None;
    };
    let closing = rest.find('}')?;
    Some((
        begin,
        &rest[..closing],
        index + line[index..].len() - rest.len() + closing + 1,
    ))
}

fn is_verbatim_environment(name: &str) -> bool {
    matches!(
        name,
        "verbatim" | "Verbatim" | "lstlisting" | "minted" | "alltt"
    )
}

fn is_escaped(line: &str, byte_index: usize) -> bool {
    line.as_bytes()[..byte_index]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        == 1
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::DoubleDollarDisplayMath;

    #[test]
    fn detects_double_dollar_display_math() {
        let diagnostics = DoubleDollarDisplayMath.check_file(Path::new("paper.tex"), "$$x$$\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "MTH001");
        assert_eq!(diagnostics[0].column, 1);
    }

    #[test]
    fn ignores_comments_and_escaped_dollars() {
        let content = "\\$\\$ escaped\n% $$ commented\nText.\n";
        let diagnostics = DoubleDollarDisplayMath.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_verbatim_environments() {
        let content = "\\begin{verbatim}\n$$x$$\n\\end{verbatim}\n";
        let diagnostics = DoubleDollarDisplayMath.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }
}
