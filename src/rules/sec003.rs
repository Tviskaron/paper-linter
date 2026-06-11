use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::sections::{section_heading, SectionHeading};
use crate::rules::Rule;

pub struct SingletonSubdivision;

impl Rule for SingletonSubdivision {
    fn code(&self) -> &'static str {
        "SEC003"
    }

    fn name(&self) -> &'static str {
        "singleton subdivision"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let headings: Vec<_> = content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| section_heading(line, index + 1))
            .collect();

        headings
            .iter()
            .filter(|heading| !heading.starred && heading.level < 3)
            .filter(|heading| direct_child_count(heading, &headings) == 1)
            .map(|heading| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!(
                        "{} has only one {}",
                        heading_name(heading.level),
                        child_name(heading.level)
                    ),
                    path,
                    heading.line,
                    heading.column,
                )
                .with_hint("merge the subdivision or add a peer subdivision")
            })
            .collect()
    }
}

fn direct_child_count(parent: &SectionHeading, headings: &[SectionHeading]) -> usize {
    headings
        .iter()
        .skip_while(|heading| heading.line != parent.line)
        .skip(1)
        .take_while(|heading| heading.level > parent.level)
        .filter(|heading| !heading.starred && heading.level == parent.level + 1)
        .count()
}

fn heading_name(level: usize) -> &'static str {
    match level {
        1 => "section",
        2 => "subsection",
        _ => "heading",
    }
}

fn child_name(level: usize) -> &'static str {
    match level {
        1 => "subsection",
        2 => "subsubsection",
        _ => "subdivision",
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::SingletonSubdivision;

    #[test]
    fn detects_single_subsection() {
        let content = "\\section{Method}\n\\subsection{Setup}\nText.\n";
        let diagnostics = SingletonSubdivision.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SEC003");
        assert_eq!(diagnostics[0].line, 1);
    }

    #[test]
    fn detects_single_subsubsection() {
        let content = "\\section{Method}\n\\subsection{Setup}\n\\subsubsection{Data}\nText.\n";
        let diagnostics = SingletonSubdivision.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics[1].message,
            "subsection has only one subsubsection"
        );
    }

    #[test]
    fn allows_two_peer_subsections() {
        let content = "\\section{Method}\n\\subsection{Setup}\nText.\n\\subsection{Data}\nText.\n";
        let diagnostics = SingletonSubdivision.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_starred_headings() {
        let content = "\\section*{Appendix}\n\\subsection{Extra}\nText.\n";
        let diagnostics = SingletonSubdivision.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }
}
