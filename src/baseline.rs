use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use crate::diagnostic::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BaselineEntry {
    pub fingerprint: String,
}

pub fn load_baseline(path: &Path) -> io::Result<BTreeSet<String>> {
    let content = fs::read_to_string(path)?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

pub fn save_baseline(path: &Path, fingerprints: &BTreeSet<String>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut lines: Vec<_> = fingerprints.iter().cloned().collect();
    lines.sort();
    let mut content = String::from("# paper-linter baseline\n");
    for line in lines {
        content.push_str(&line);
        content.push('\n');
    }
    fs::write(path, content)
}

pub fn fingerprint(diagnostic: &Diagnostic) -> String {
    let file = diagnostic
        .file
        .canonicalize()
        .unwrap_or_else(|_| diagnostic.file.clone());
    format!(
        "{}|{}|{}|{}|{}",
        diagnostic.code,
        file.display(),
        diagnostic.line,
        diagnostic.column,
        diagnostic.message
    )
}

pub fn filter_baseline(diagnostics: Vec<Diagnostic>, baseline: &BTreeSet<String>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| !baseline.contains(&fingerprint(diagnostic)))
        .collect()
}

pub fn fingerprints_from_diagnostics(diagnostics: &[Diagnostic]) -> BTreeSet<String> {
    diagnostics.iter().map(fingerprint).collect()
}

pub fn update_baseline(path: &Path, diagnostics: &[Diagnostic]) -> io::Result<()> {
    save_baseline(path, &fingerprints_from_diagnostics(diagnostics))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::diagnostic::{Diagnostic, Severity};

    use super::{filter_baseline, fingerprint};

    #[test]
    fn baseline_filters_known_diagnostics() {
        let diagnostic = Diagnostic::new(
            "WS001",
            Severity::Warning,
            "trailing whitespace",
            Path::new("paper.tex"),
            2,
            5,
        );
        let baseline = [fingerprint(&diagnostic)].into_iter().collect();
        let filtered = filter_baseline(vec![diagnostic.clone()], &baseline);
        assert!(filtered.is_empty());
    }
}
