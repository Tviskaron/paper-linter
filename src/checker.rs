use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::baseline::{Baseline, BaselineError};
use crate::config::LinterConfig;
use crate::diagnostic::{Diagnostic, Severity};
use crate::discovery::discover_tex_files;
use crate::project::ProjectIndex;
use crate::project_graph::ProjectGraph;
use crate::rule_policy;
use crate::rules::citations::{check_project, explicit_bib_files, SourceFile};
use crate::rules::{all_graph_project_rules, all_project_rules, all_rules};
use crate::suppressions::Suppressions;

#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    pub paths: Vec<PathBuf>,
    pub select: Vec<String>,
    pub ignore: Vec<String>,
    pub strict: bool,
    pub all_tex: bool,
    pub baseline: Option<PathBuf>,
    pub project_index: Option<PathBuf>,
    pub config: LinterConfig,
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
    let mut diagnostics = Vec::new();

    let project = load_project_index(options)?;

    if project.is_none() && !ProjectGraph::should_analyze(&options.paths) {
        return Ok(CheckResult {
            diagnostics,
            files_checked: 0,
        });
    }

    let mut sources = Vec::new();

    if let Some(project) = &project {
        for file in &project.files {
            for rule in all_rules() {
                if !rule_is_enabled(rule.code(), rule.strict_only(), options) {
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
            if !rule_is_enabled(rule.code(), rule.strict_only(), options) {
                continue;
            }

            diagnostics.extend(rule.check_project(project));
        }
    }

    if family_may_be_enabled("CIT", options) || family_may_be_enabled("BIB", options) {
        let explicit_bibs = explicit_bib_files(&options.paths);
        let citation_diagnostics = check_project(&sources, &explicit_bibs)
            .map_err(|source| ToolError::Io { path: None, source })?;
        diagnostics.extend(citation_diagnostics);
    }

    if ProjectGraph::should_analyze(&options.paths) {
        let graphs = ProjectGraph::analyze_paths(&options.paths)
            .map_err(|source| ToolError::Io { path: None, source })?;
        for graph in graphs {
            for rule in all_graph_project_rules() {
                if !rule_is_enabled(rule.code(), false, options) {
                    continue;
                }
                diagnostics.extend(rule.check_graph(&graph));
            }
        }
    }

    diagnostics.retain(|diagnostic| code_is_enabled(diagnostic.code, options));

    if let Some(project) = &project {
        let suppressions = Suppressions::from_sources(&project.files);
        diagnostics.retain(|diagnostic| !suppressions.suppresses(diagnostic));

        if let Some(path) = &options.baseline {
            let baseline = Baseline::read(path).map_err(|source| ToolError::Baseline {
                path: path.clone(),
                source,
            })?;
            diagnostics.retain(|diagnostic| !baseline.contains(diagnostic, &project.root));
        }
    }

    if options.strict {
        for diagnostic in &mut diagnostics {
            if diagnostic.severity == Severity::Warning
                && !rule_policy::never_promote_to_error(diagnostic.code)
            {
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
        files_checked: project
            .as_ref()
            .map(|project| project.files.len())
            .unwrap_or(0),
    })
}

pub fn build_project_index(
    paths: &[PathBuf],
    all_tex: bool,
) -> Result<Option<ProjectIndex>, ToolError> {
    let files = discover_tex_files(paths, all_tex)
        .map_err(|source| ToolError::Io { path: None, source })?;
    if files.is_empty() {
        return Ok(None);
    }

    ProjectIndex::build(paths, &files)
        .map(Some)
        .map_err(|source| ToolError::Io { path: None, source })
}

fn load_project_index(options: &CheckOptions) -> Result<Option<ProjectIndex>, ToolError> {
    if let Some(path) = &options.project_index {
        return ProjectIndex::read(path)
            .map(Some)
            .map_err(|source| ToolError::Io {
                path: Some(path.clone()),
                source,
            });
    }

    build_project_index(&options.paths, options.all_tex)
}

fn code_is_enabled(code: &str, options: &CheckOptions) -> bool {
    if options
        .config
        .disable
        .iter()
        .any(|pattern| code.starts_with(pattern))
    {
        return false;
    }

    if options
        .config
        .enable
        .iter()
        .any(|pattern| code.starts_with(pattern))
    {
        let ignored = options
            .ignore
            .iter()
            .any(|pattern| code.starts_with(pattern));
        return !ignored;
    }

    rule_policy::code_is_enabled(code, &options.select, &options.ignore, options.strict)
}

fn rule_is_enabled(code: &str, strict_only: bool, options: &CheckOptions) -> bool {
    if !code_is_enabled(code, options) {
        return false;
    }

    !strict_only || options.strict || !options.select.is_empty()
}

fn family_may_be_enabled(family: &str, options: &CheckOptions) -> bool {
    let selected = options.select.is_empty()
        || options
            .select
            .iter()
            .any(|pattern| family.starts_with(pattern) || pattern.starts_with(family))
        || options
            .config
            .enable
            .iter()
            .any(|pattern| pattern.starts_with(family));
    let ignored = options.ignore.iter().any(|pattern| *pattern == family);
    selected && !ignored
}

#[cfg(test)]
mod tests {
    use super::{code_is_enabled, rule_is_enabled, CheckOptions};

    #[test]
    fn select_defaults_to_all_default_rules() {
        let options = CheckOptions::default();
        assert!(!code_is_enabled("WS001", &options));
        assert!(code_is_enabled("PRJ001", &options));
        assert!(code_is_enabled("TEX001", &options));
        assert!(!code_is_enabled("TXT003", &options));
    }

    #[test]
    fn select_accepts_exact_codes_and_prefixes() {
        let options = CheckOptions {
            select: vec![String::from("WS001")],
            ..CheckOptions::default()
        };
        assert!(code_is_enabled("WS001", &options));
        assert!(!code_is_enabled("FIG001", &options));
    }

    #[test]
    fn ignore_is_applied_after_select() {
        let options = CheckOptions {
            select: vec![String::from("WS")],
            ignore: vec![String::from("WS001")],
            ..CheckOptions::default()
        };
        assert!(!code_is_enabled("WS001", &options));
    }

    #[test]
    fn strict_only_rules_require_strict_or_explicit_select() {
        let options = CheckOptions::default();
        assert!(!rule_is_enabled("CAP002", true, &options));
        let selected = CheckOptions {
            select: vec![String::from("CAP002")],
            ..CheckOptions::default()
        };
        assert!(rule_is_enabled("CAP002", true, &selected));
        let strict = CheckOptions {
            strict: true,
            ..CheckOptions::default()
        };
        assert!(rule_is_enabled("CAP002", true, &strict));
    }
}
