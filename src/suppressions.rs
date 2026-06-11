use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::diagnostic::Diagnostic;
use crate::project::SourceFile;

#[derive(Debug, Default)]
pub struct Suppressions {
    files: BTreeMap<PathBuf, FileSuppressions>,
}

#[derive(Debug, Default)]
struct FileSuppressions {
    file_rules: Vec<String>,
    line_rules: BTreeMap<usize, Vec<String>>,
    next_line_rules: BTreeMap<usize, Vec<String>>,
}

impl Suppressions {
    pub fn from_sources(sources: &[SourceFile]) -> Self {
        let mut suppressions = Self::default();

        for source in sources {
            let file_suppressions = parse_file_suppressions(&source.content);
            if !file_suppressions.is_empty() {
                suppressions
                    .files
                    .insert(source.path.clone(), file_suppressions);
            }
        }

        suppressions
    }

    pub fn suppresses(&self, diagnostic: &Diagnostic) -> bool {
        self.files
            .get(&diagnostic.file)
            .is_some_and(|file| file.suppresses(diagnostic))
    }
}

impl FileSuppressions {
    fn is_empty(&self) -> bool {
        self.file_rules.is_empty() && self.line_rules.is_empty() && self.next_line_rules.is_empty()
    }

    fn suppresses(&self, diagnostic: &Diagnostic) -> bool {
        matches_rule(&self.file_rules, diagnostic.code)
            || self
                .line_rules
                .get(&diagnostic.line)
                .is_some_and(|rules| matches_rule(rules, diagnostic.code))
            || self
                .next_line_rules
                .get(&diagnostic.line)
                .is_some_and(|rules| matches_rule(rules, diagnostic.code))
    }
}

fn parse_file_suppressions(content: &str) -> FileSuppressions {
    let mut suppressions = FileSuppressions::default();

    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        let Some(comment) = comment_text(line) else {
            continue;
        };
        let Some((directive, rules)) = parse_directive(comment) else {
            continue;
        };

        match directive {
            "paper-linter-ignore-file" => {
                append_unique(&mut suppressions.file_rules, rules);
            }
            "paper-linter-ignore-line" => {
                append_unique(
                    suppressions.line_rules.entry(line_number).or_default(),
                    rules,
                );
            }
            "paper-linter-ignore-next-line" => {
                append_unique(
                    suppressions
                        .next_line_rules
                        .entry(line_number + 1)
                        .or_default(),
                    rules,
                );
            }
            _ => {}
        }
    }

    suppressions
}

fn comment_text(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'%' && !is_escaped(bytes, index) {
            return Some(line[index + 1..].trim());
        }
    }
    None
}

fn parse_directive(comment: &str) -> Option<(&str, Vec<String>)> {
    let mut parts = comment.split_whitespace();
    let directive = parts.next()?;
    if !matches!(
        directive,
        "paper-linter-ignore-file" | "paper-linter-ignore-line" | "paper-linter-ignore-next-line"
    ) {
        return None;
    }

    let rules = parts
        .flat_map(|part| part.split(','))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    Some((directive, rules))
}

fn append_unique(target: &mut Vec<String>, rules: Vec<String>) {
    let mut seen: BTreeSet<String> = target.iter().cloned().collect();
    for rule in rules {
        if seen.insert(rule.clone()) {
            target.push(rule);
        }
    }
}

fn matches_rule(patterns: &[String], code: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| pattern == "*" || code.starts_with(pattern))
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut slash_count = 0;
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        slash_count += 1;
        cursor -= 1;
    }
    slash_count % 2 == 1
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::diagnostic::{Diagnostic, Severity};
    use crate::project::SourceFile;

    use super::Suppressions;

    fn diagnostic(code: &'static str, line: usize) -> Diagnostic {
        Diagnostic::new(
            code,
            Severity::Warning,
            "message",
            PathBuf::from("paper.tex"),
            line,
            1,
        )
    }

    #[test]
    fn suppresses_next_line_by_exact_code() {
        let source = SourceFile {
            path: PathBuf::from("paper.tex"),
            content: "% paper-linter-ignore-next-line WS001\ntrailing \n".to_string(),
        };
        let suppressions = Suppressions::from_sources(&[source]);

        assert!(suppressions.suppresses(&diagnostic("WS001", 2)));
        assert!(!suppressions.suppresses(&diagnostic("WS001", 1)));
        assert!(!suppressions.suppresses(&diagnostic("TXT001", 2)));
    }

    #[test]
    fn suppresses_file_by_rule_prefix() {
        let source = SourceFile {
            path: PathBuf::from("paper.tex"),
            content: "% paper-linter-ignore-file CIT\n".to_string(),
        };
        let suppressions = Suppressions::from_sources(&[source]);

        assert!(suppressions.suppresses(&diagnostic("CIT001", 10)));
        assert!(!suppressions.suppresses(&diagnostic("FIG001", 10)));
    }

    #[test]
    fn suppresses_same_line_by_rule_prefix() {
        let source = SourceFile {
            path: PathBuf::from("paper.tex"),
            content: "trailing  % paper-linter-ignore-line WS\n".to_string(),
        };
        let suppressions = Suppressions::from_sources(&[source]);

        assert!(suppressions.suppresses(&diagnostic("WS001", 1)));
        assert!(!suppressions.suppresses(&diagnostic("TXT001", 1)));
        assert!(!suppressions.suppresses(&diagnostic("WS001", 2)));
    }

    #[test]
    fn ignores_escaped_percent() {
        let source = SourceFile {
            path: PathBuf::from("paper.tex"),
            content: "\\% paper-linter-ignore-next-line WS001\ntrailing \n".to_string(),
        };
        let suppressions = Suppressions::from_sources(&[source]);

        assert!(!suppressions.suppresses(&diagnostic("WS001", 2)));
    }
}
