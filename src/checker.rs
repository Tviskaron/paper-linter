use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::baseline::{Baseline, BaselineError};
use crate::diagnostic::{Diagnostic, Severity};
use crate::discovery::discover_tex_files;
use crate::project::ProjectIndex;
use crate::rules::citations::{check_project, explicit_bib_files, SourceFile};
use crate::rules::{all_project_rules, all_rules};
use crate::suppressions::Suppressions;

#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub paths: Vec<PathBuf>,
    pub select: Vec<String>,
    pub ignore: Vec<String>,
    pub strict: bool,
    pub all_tex: bool,
    pub baseline: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub files_checked: usize,
}

impl CheckResult {
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .count()
    }
}

#[derive(Debug)]
pub enum ToolError {
    Io {
        path: Option<PathBuf>,
        source: io::Error,
    },
    Baseline {
        path: PathBuf,
        source: BaselineError,
    },
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::Io {
                path: Some(path),
                source,
            } => write!(formatter, "{}: {}", path.display(), source),
            ToolError::Io { path: None, source } => write!(formatter, "{source}"),
            ToolError::Baseline { path, source } => {
                write!(
                    formatter,
                    "{}: failed to read baseline: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ToolError {}

pub fn run_check(options: &CheckOptions) -> Result<CheckResult, ToolError> {
    let files = discover_tex_files(&options.paths, options.all_tex)
        .map_err(|source| ToolError::Io { path: None, source })?;
    let mut diagnostics = Vec::new();

    if files.is_empty() {
        return Ok(CheckResult {
            diagnostics,
            files_checked: 0,
        });
    }

    let project = ProjectIndex::build(&options.paths, &files)
        .map_err(|source| ToolError::Io { path: None, source })?;
    let mut sources = Vec::new();

    for file in &project.files {
        for rule in all_rules() {
            if !rule_is_enabled(
                rule.code(),
                rule.strict_only(),
                options.strict,
                &options.select,
                &options.ignore,
            ) {
                continue;
            }

            diagnostics.extend(rule.check_file(&file.path, &file.content));
        }

        sources.push(SourceFile {
            path: file.path.clone(),
            content: file.content.clone(),
        });
    }

    for rule in all_project_rules() {
        if !rule_is_enabled(
            rule.code(),
            rule.strict_only(),
            options.strict,
            &options.select,
            &options.ignore,
        ) {
            continue;
        }

        diagnostics.extend(rule.check_project(&project));
    }

    if family_may_be_enabled("CIT", &options.select, &options.ignore)
        || family_may_be_enabled("BIB", &options.select, &options.ignore)
    {
        let explicit_bibs = explicit_bib_files(&options.paths);
        let citation_diagnostics = check_project(&sources, &explicit_bibs)
            .map_err(|source| ToolError::Io { path: None, source })?;
        diagnostics.extend(citation_diagnostics);
    }

    diagnostics
        .retain(|diagnostic| code_is_enabled(diagnostic.code, &options.select, &options.ignore));

    let suppressions = Suppressions::from_sources(&project.files);
    diagnostics.retain(|diagnostic| !suppressions.suppresses(diagnostic));

    if let Some(path) = &options.baseline {
        let baseline = Baseline::read(path).map_err(|source| ToolError::Baseline {
            path: path.clone(),
            source,
        })?;
        diagnostics.retain(|diagnostic| !baseline.contains(diagnostic, &project.root));
    }

    if options.strict {
        for diagnostic in &mut diagnostics {
            if diagnostic.severity == Severity::Warning {
                diagnostic.severity = Severity::Error;
            }
        }
    }

    diagnostics.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
            .then(left.code.cmp(right.code))
    });

    Ok(CheckResult {
        diagnostics,
        files_checked: project.files.len(),
    })
}

fn code_is_enabled(code: &str, select: &[String], ignore: &[String]) -> bool {
    let selected = select.is_empty() || select.iter().any(|pattern| code.starts_with(pattern));
    let ignored = ignore.iter().any(|pattern| code.starts_with(pattern));
    selected && !ignored
}

fn rule_is_enabled(
    code: &str,
    strict_only: bool,
    strict: bool,
    select: &[String],
    ignore: &[String],
) -> bool {
    if !code_is_enabled(code, select, ignore) {
        return false;
    }

    !strict_only || strict || !select.is_empty()
}

fn family_may_be_enabled(family: &str, select: &[String], ignore: &[String]) -> bool {
    let selected = select.is_empty()
        || select
            .iter()
            .any(|pattern| family.starts_with(pattern) || pattern.starts_with(family));
    let ignored = ignore.iter().any(|pattern| *pattern == family);
    selected && !ignored
}

#[cfg(test)]
mod tests {
    use super::{code_is_enabled, rule_is_enabled};

    #[test]
    fn select_defaults_to_all_rules() {
        assert!(code_is_enabled("WS001", &[], &[]));
        assert!(code_is_enabled("FIG001", &[], &[]));
    }

    #[test]
    fn select_accepts_exact_codes_and_prefixes() {
        assert!(code_is_enabled("WS001", &[String::from("WS001")], &[]));
        assert!(code_is_enabled("WS001", &[String::from("WS")], &[]));
        assert!(!code_is_enabled("WS001", &[String::from("CIT")], &[]));
    }

    #[test]
    fn ignore_is_applied_after_select() {
        assert!(!code_is_enabled(
            "WS001",
            &[String::from("WS")],
            &[String::from("WS001")]
        ));
    }

    #[test]
    fn strict_only_rules_require_strict_or_explicit_select() {
        assert!(!rule_is_enabled("CAP002", true, false, &[], &[]));
        assert!(rule_is_enabled(
            "CAP002",
            true,
            false,
            &[String::from("CAP002")],
            &[]
        ));
        assert!(rule_is_enabled("CAP002", true, true, &[], &[]));
    }
}
