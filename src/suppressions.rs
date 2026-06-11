use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::diagnostic::Diagnostic;

const IGNORE_NEXT_LINE: &str = "paper-linter-ignore-next-line";
const IGNORE_FILE: &str = "paper-linter-ignore-file";

#[derive(Debug, Default)]
pub struct SuppressionIndex {
    file_rules: BTreeMap<PathBuf, BTreeSet<String>>,
    next_line: BTreeMap<(PathBuf, usize), BTreeSet<String>>,
}

impl SuppressionIndex {
    pub fn from_files(files: &[(PathBuf, String)]) -> Self {
        let mut index = Self::default();

        for (path, content) in files {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            for (line_number, line) in content.lines().enumerate() {
                let line_index = line_number + 1;
                if let Some(comment) = comment_text(line) {
                    if let Some(code) = parse_ignore_next_line(comment) {
                        index
                            .next_line
                            .entry((canonical.clone(), line_index + 1))
                            .or_default()
                            .insert(code);
                    }
                    if let Some(code) = parse_ignore_file(comment) {
                        index
                            .file_rules
                            .entry(canonical.clone())
                            .or_default()
                            .insert(code);
                    }
                }
            }
        }

        index
    }
}

pub fn apply_suppressions(diagnostics: Vec<Diagnostic>, index: &SuppressionIndex) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| !is_suppressed(diagnostic, index))
        .collect()
}

fn is_suppressed(diagnostic: &Diagnostic, index: &SuppressionIndex) -> bool {
    let canonical = diagnostic.file.canonicalize().unwrap_or_else(|_| diagnostic.file.clone());

    if index
        .file_rules
        .get(&canonical)
        .is_some_and(|rules| rules.iter().any(|rule| diagnostic.code.starts_with(rule)))
    {
        return true;
    }

    index
        .next_line
        .get(&(canonical, diagnostic.line))
        .is_some_and(|rules| rules.iter().any(|rule| diagnostic.code.starts_with(rule)))
}

fn comment_text(line: &str) -> Option<&str> {
    let index = line.find('%')?;
    if index > 0 && line.as_bytes()[index - 1] == b'\\' {
        return None;
    }
    Some(line[index + 1..].trim())
}

fn parse_ignore_next_line(comment: &str) -> Option<String> {
    let rest = comment.strip_prefix(IGNORE_NEXT_LINE)?.trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest.split_whitespace().next()?.to_ascii_uppercase())
}

fn parse_ignore_file(comment: &str) -> Option<String> {
    let rest = comment.strip_prefix(IGNORE_FILE)?.trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest.split_whitespace().next()?.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::diagnostic::{Diagnostic, Severity};

    use super::{apply_suppressions, SuppressionIndex};

    #[test]
    fn suppresses_next_line_rule() {
        let path = Path::new("paper.tex").to_path_buf();
        let content = "line\n% paper-linter-ignore-next-line WS001\nbad \n";
        let index = SuppressionIndex::from_files(&[(path.clone(), content.to_string())]);
        let diagnostics = vec![Diagnostic::new(
            "WS001",
            Severity::Warning,
            "trailing whitespace",
            &path,
            3,
            1,
        )];

        assert!(apply_suppressions(diagnostics, &index).is_empty());
    }
}
