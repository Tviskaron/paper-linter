#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionHeading {
    pub level: usize,
    pub line: usize,
    pub column: usize,
    pub starred: bool,
    pub content_after_heading: bool,
}

pub fn section_heading(line: &str, line_number: usize) -> Option<SectionHeading> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('%') {
        return None;
    }

    let column = line.chars().count() - trimmed.chars().count() + 1;
    let commands = [
        ("\\subsubsection", 3),
        ("\\subsection", 2),
        ("\\section", 1),
    ];

    for (command, level) in commands {
        if let Some(rest) = trimmed.strip_prefix(command) {
            if let Some((starred, after_heading)) = content_after_section_heading(rest) {
                return Some(SectionHeading {
                    level,
                    line: line_number,
                    column,
                    starred,
                    content_after_heading: line_is_meaningful_section_content(after_heading),
                });
            }
        }
    }

    None
}

pub fn line_is_meaningful_section_content(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('%') {
        return false;
    }

    !trimmed.starts_with("\\label{")
}

fn content_after_section_heading(rest: &str) -> Option<(bool, &str)> {
    let rest = rest.trim_start();
    let (starred, rest) = if let Some(rest) = rest.strip_prefix('*') {
        (true, rest)
    } else {
        (false, rest)
    };
    let rest = rest.strip_prefix('{')?;
    let end = matching_brace_end(rest)?;

    Some((starred, &rest[end + 1..]))
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

#[cfg(test)]
mod tests {
    use super::{line_is_meaningful_section_content, section_heading};

    #[test]
    fn recognizes_section_levels() {
        assert_eq!(section_heading("\\section{Intro}", 1).unwrap().level, 1);
        assert_eq!(section_heading("\\subsection{Setup}", 1).unwrap().level, 2);
        assert_eq!(
            section_heading("\\subsubsection{Details}", 1)
                .unwrap()
                .level,
            3
        );
    }

    #[test]
    fn recognizes_starred_sections() {
        let heading = section_heading("\\section*{Intro}", 1).unwrap();

        assert_eq!(heading.level, 1);
        assert!(heading.starred);
    }

    #[test]
    fn detects_inline_content_after_heading() {
        let heading = section_heading("\\section{Intro} Inline text.", 1).unwrap();

        assert!(heading.content_after_heading);
    }

    #[test]
    fn handles_nested_braces_in_heading() {
        let heading = section_heading("\\section{A {Nested} Title} Inline text.", 1).unwrap();

        assert!(heading.content_after_heading);
    }

    #[test]
    fn ignores_commented_sections() {
        assert!(section_heading("% \\section{Old}", 1).is_none());
    }

    #[test]
    fn treats_labels_as_non_content() {
        assert!(!line_is_meaningful_section_content("\\label{sec:intro}"));
        assert!(line_is_meaningful_section_content("Actual text."));
    }
}
