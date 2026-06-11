#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionHeading {
    pub level: usize,
    pub line: usize,
    pub column: usize,
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
            if section_argument_starts(rest) {
                return Some(SectionHeading {
                    level,
                    line: line_number,
                    column,
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

fn section_argument_starts(rest: &str) -> bool {
    let rest = rest.trim_start();
    rest.starts_with('{') || rest.starts_with("*{")
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
        assert_eq!(section_heading("\\section*{Intro}", 1).unwrap().level, 1);
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
