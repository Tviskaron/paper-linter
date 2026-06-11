use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct HeadingStyle;

impl Rule for HeadingStyle {
    fn code(&self) -> &'static str {
        "SEC006"
    }

    fn name(&self) -> &'static str {
        "heading style"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (index, line) in content.lines().enumerate() {
            let Some(heading) = heading_title(line, index + 1) else {
                continue;
            };
            if heading.starred {
                continue;
            }

            let title = heading.title.trim();
            if has_trailing_heading_punctuation(title) {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        "heading ends with punctuation",
                        path,
                        heading.line,
                        heading.column,
                    )
                    .with_hint("remove trailing punctuation from the heading"),
                );
            }

            if is_all_caps_heading(title) {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        "heading is all caps",
                        path,
                        heading.line,
                        heading.column,
                    )
                    .with_hint(
                        "use normal heading capitalization unless the venue requires all caps",
                    ),
                );
            }
        }

        diagnostics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeadingTitle<'a> {
    title: &'a str,
    line: usize,
    column: usize,
    starred: bool,
}

fn heading_title(line: &str, line_number: usize) -> Option<HeadingTitle<'_>> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('%') {
        return None;
    }

    let column = line.chars().count() - trimmed.chars().count() + 1;
    let commands = ["\\subsubsection", "\\subsection", "\\section"];

    for command in commands {
        if let Some(rest) = trimmed.strip_prefix(command) {
            let rest = rest.trim_start();
            let (starred, rest) = if let Some(rest) = rest.strip_prefix('*') {
                (true, rest)
            } else {
                (false, rest)
            };
            let rest = rest.trim_start().strip_prefix('{')?;
            let end = matching_brace_end(rest)?;
            return Some(HeadingTitle {
                title: &rest[..end],
                line: line_number,
                column,
                starred,
            });
        }
    }

    None
}

fn matching_brace_end(rest: &str) -> Option<usize> {
    let mut depth = 1;
    let mut escaped = false;

    for (index, character) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match character {
            '\\' => escaped = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
}

fn has_trailing_heading_punctuation(title: &str) -> bool {
    title.ends_with('.') || title.ends_with(':') || title.ends_with(';') || title.ends_with(',')
}

fn is_all_caps_heading(title: &str) -> bool {
    let letters: Vec<_> = title
        .chars()
        .filter(|character| character.is_alphabetic())
        .collect();
    letters.len() >= 4 && letters.iter().all(|character| !character.is_lowercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::HeadingStyle;

    #[test]
    fn detects_trailing_heading_punctuation() {
        let diagnostics = HeadingStyle.check_file(Path::new("paper.tex"), "\\section{Method:}\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SEC006");
        assert_eq!(diagnostics[0].message, "heading ends with punctuation");
    }

    #[test]
    fn detects_all_caps_heading() {
        let diagnostics =
            HeadingStyle.check_file(Path::new("paper.tex"), "\\subsection{ABLATION STUDY}\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "heading is all caps");
    }

    #[test]
    fn accepts_normal_headings_and_questions() {
        let content = "\\section{Methodology and Results}\n\\subsection{Why does it work?}\n";
        let diagnostics = HeadingStyle.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_starred_and_commented_headings() {
        let content = "% \\section{BAD:}\n\\section*{ACKNOWLEDGMENTS}\n";
        let diagnostics = HeadingStyle.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }
}
