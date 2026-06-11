use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::diagnostic::Diagnostic;

const BASELINE_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BaselineFile {
    pub version: u32,
    pub diagnostics: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BaselineEntry {
    pub fingerprint: String,
    pub code: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Baseline {
    fingerprints: BTreeSet<String>,
}

impl Baseline {
    pub fn read(path: &Path) -> Result<Self, BaselineError> {
        let content = fs::read_to_string(path).map_err(BaselineError::Io)?;
        let baseline: BaselineFile = serde_json::from_str(&content).map_err(BaselineError::Json)?;

        Ok(Self {
            fingerprints: baseline
                .diagnostics
                .into_iter()
                .map(|entry| entry.fingerprint)
                .collect(),
        })
    }

    pub fn contains(&self, diagnostic: &Diagnostic, root: &Path) -> bool {
        self.fingerprints
            .contains(&diagnostic_fingerprint(diagnostic, root))
    }
}

#[derive(Debug)]
pub enum BaselineError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for BaselineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for BaselineError {}

pub fn write_baseline(
    path: &Path,
    diagnostics: &[Diagnostic],
    root: &Path,
) -> Result<(), BaselineError> {
    let mut entries: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| BaselineEntry {
            fingerprint: diagnostic_fingerprint(diagnostic, root),
            code: diagnostic.code.to_string(),
            file: display_path(&diagnostic.file, root),
            line: diagnostic.line,
            column: diagnostic.column,
            message: diagnostic.message.clone(),
        })
        .collect();
    entries.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));

    let content = serde_json::to_string_pretty(&BaselineFile {
        version: BASELINE_VERSION,
        diagnostics: entries,
    })
    .map_err(BaselineError::Json)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(BaselineError::Io)?;
    }
    fs::write(path, format!("{content}\n")).map_err(BaselineError::Io)
}

pub fn diagnostic_fingerprint(diagnostic: &Diagnostic, root: &Path) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        diagnostic.code,
        display_path(&diagnostic.file, root),
        diagnostic.line,
        diagnostic.column,
        diagnostic.message
    )
}

fn display_path(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    normalize_path(relative)
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn infer_baseline_root(paths: &[PathBuf]) -> PathBuf {
    paths
        .iter()
        .filter_map(|path| {
            if path.is_dir() {
                path.canonicalize().ok()
            } else {
                path.parent().and_then(|parent| parent.canonicalize().ok())
            }
        })
        .reduce(common_ancestor)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn common_ancestor(left: PathBuf, right: PathBuf) -> PathBuf {
    let left_components: Vec<_> = left.components().collect();
    let right_components: Vec<_> = right.components().collect();
    let mut common = PathBuf::new();

    for (left, right) in left_components.iter().zip(right_components.iter()) {
        if left != right {
            break;
        }
        common.push(left.as_os_str());
    }

    common
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::diagnostic::{Diagnostic, Severity};

    use super::diagnostic_fingerprint;

    #[test]
    fn fingerprint_uses_relative_normalized_path() {
        let root = PathBuf::from("/tmp/project");
        let diagnostic = Diagnostic::new(
            "WS001",
            Severity::Warning,
            "trailing whitespace",
            PathBuf::from("/tmp/project/sections/intro.tex"),
            2,
            5,
        );

        assert_eq!(
            diagnostic_fingerprint(&diagnostic, &root),
            "WS001:sections/intro.tex:2:5:trailing whitespace"
        );
    }
}
