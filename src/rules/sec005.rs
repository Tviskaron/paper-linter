use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::prose::word_count;
use crate::latex::sections::{line_is_meaningful_section_content, section_heading, SectionHeading};
use crate::rules::Rule;

const MIN_SECTION_WORDS: usize = 30;

pub struct ShortSection;

impl Rule for ShortSection {
    fn code(&self) -> &'static str {
        "SEC005"
    }

    fn name(&self) -> &'static str {
        "short section"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let lines: Vec<_> = content.lines().collect();
        let headings = headings(content);

        headings
            .iter()
            .enumerate()
            .filter(|(_, section)| !section.heading.starred)
            .filter(|(_, section)| !has_child_heading(section, &headings))
            .filter_map(|(index, section)| {
                let end_line = section_end_line(index, &headings, lines.len());
                let words = section_word_count(section.heading.line, end_line, &lines);

                if (1..MIN_SECTION_WORDS).contains(&words) {
                    Some(
                        Diagnostic::new(
                            self.code(),
                            Severity::Warning,
                            format!("section is very short ({words} words)"),
                            path,
                            section.heading.line,
                            section.heading.column,
                        )
                        .with_hint("expand the section or merge it with a neighboring section"),
                    )
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
struct Section {
    heading: SectionHeading,
}

fn headings(content: &str) -> Vec<Section> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| section_heading(line, index + 1))
        .map(|heading| Section { heading })
        .collect()
}

fn has_child_heading(section: &Section, sections: &[Section]) -> bool {
    sections
        .iter()
        .skip_while(|candidate| candidate.heading.line != section.heading.line)
        .skip(1)
        .take_while(|candidate| candidate.heading.level > section.heading.level)
        .any(|candidate| !candidate.heading.starred)
}

fn section_end_line(index: usize, sections: &[Section], line_count: usize) -> usize {
    let level = sections[index].heading.level;
    sections[index + 1..]
        .iter()
        .find(|section| section.heading.level <= level)
        .map(|section| section.heading.line.saturating_sub(1))
        .unwrap_or(line_count)
}

fn section_word_count(start_line: usize, end_line: usize, lines: &[&str]) -> usize {
    lines
        .iter()
        .enumerate()
        .skip(start_line)
        .take_while(|(index, _)| *index < end_line)
        .filter(|(_, line)| line_is_meaningful_section_content(line))
        .map(|(_, line)| word_count(line))
        .sum()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::ShortSection;

    #[test]
    fn detects_short_leaf_section() {
        let content = "\\section{Results}\nA brief note.\n";
        let diagnostics = ShortSection.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SEC005");
        assert_eq!(diagnostics[0].line, 1);
    }

    #[test]
    fn accepts_long_leaf_section() {
        let content = "\\section{Results}\nThis section contains enough explanatory prose to pass the strict short section threshold because it gives the reader context, motivation, evidence, and a compact but meaningful discussion of the reported result.\n";
        let diagnostics = ShortSection.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_empty_and_parent_sections() {
        let content = "\\section{Method}\n\\subsection{Setup}\nShort text.\n\\section{Empty}\n";
        let diagnostics = ShortSection.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
    }

    #[test]
    fn ignores_starred_sections() {
        let content = "\\section*{Acknowledgments}\nThanks.\n";
        let diagnostics = ShortSection.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }
}
