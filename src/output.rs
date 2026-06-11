use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use crate::baseline::diagnostic_fingerprint;
use crate::checker::CheckResult;
use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::rule_infos;

pub fn render_text(result: &CheckResult) -> String {
    let mut output = String::new();

    for diagnostic in &result.diagnostics {
        let hint = diagnostic
            .hint
            .as_ref()
            .map(|hint| format!("; hint: {hint}"))
            .unwrap_or_default();
        output.push_str(&format!(
            "{}:{}:{}: {}[{}] {}{}\n",
            display_path(&diagnostic.file),
            diagnostic.line,
            diagnostic.column,
            diagnostic.severity.as_str(),
            diagnostic.code,
            diagnostic.message,
            hint
        ));
    }

    output.push_str(&format!(
        "checked {} file(s), {} error(s), {} warning(s)\n",
        result.files_checked,
        result.error_count(),
        result.warning_count()
    ));

    output
}

pub fn render_json(result: &CheckResult) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&JsonOutput {
        version: env!("CARGO_PKG_VERSION"),
        diagnostics: &result.diagnostics,
        summary: JsonSummary {
            files_checked: result.files_checked,
            errors: result.error_count(),
            warnings: result.warning_count(),
        },
    })
}

pub fn render_sarif(result: &CheckResult, root: &Path) -> Result<String, serde_json::Error> {
    let rules: Vec<_> = rule_infos()
        .iter()
        .map(|rule| {
            json!({
                "id": rule.code,
                "name": rule.name,
                "shortDescription": {
                    "text": rule.summary,
                },
                "fullDescription": {
                    "text": rule.why,
                },
                "help": {
                    "text": rule.fix,
                },
                "defaultConfiguration": {
                    "level": sarif_level(rule.default_severity),
                },
            })
        })
        .collect();

    let results: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| sarif_result(diagnostic, root))
        .collect();

    serde_json::to_string_pretty(&json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "paper-linter",
                        "informationUri": "https://github.com/Tviskaron/paper-linter",
                        "semanticVersion": env!("CARGO_PKG_VERSION"),
                        "rules": rules,
                    },
                },
                "results": results,
            },
        ],
    }))
}

pub fn render_lsp(result: &CheckResult) -> Result<String, serde_json::Error> {
    let mut by_file: BTreeMap<&PathBuf, Vec<serde_json::Value>> = BTreeMap::new();

    for diagnostic in &result.diagnostics {
        by_file
            .entry(&diagnostic.file)
            .or_default()
            .push(lsp_diagnostic(diagnostic));
    }

    let diagnostics: Vec<_> = by_file
        .into_iter()
        .map(|(file, diagnostics)| {
            json!({
                "uri": file_uri(file),
                "diagnostics": diagnostics,
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "version": env!("CARGO_PKG_VERSION"),
        "diagnostics": diagnostics,
        "summary": JsonSummary {
            files_checked: result.files_checked,
            errors: result.error_count(),
            warnings: result.warning_count(),
        },
    }))
}

pub fn render_ready_text(result: &CheckResult) -> String {
    let groups = ReadyGroups::from_result(result);
    let mut output = String::new();

    output.push_str(&format!("submission readiness: {}\n", groups.status));
    output.push_str(&format!(
        "checked {} file(s), {} blocker(s), {} warning(s)\n",
        result.files_checked,
        groups.blockers.len(),
        result.warning_count()
    ));

    push_ready_group(&mut output, "Blockers", &groups.blockers);
    push_ready_group(&mut output, "Project risks", &groups.project_risks);
    push_ready_group(&mut output, "Polish", &groups.polish);

    output
}

pub fn render_ready_json(result: &CheckResult) -> Result<String, serde_json::Error> {
    let groups = ReadyGroups::from_result(result);
    serde_json::to_string_pretty(&ReadyOutput {
        version: env!("CARGO_PKG_VERSION"),
        status: groups.status,
        summary: JsonSummary {
            files_checked: result.files_checked,
            errors: result.error_count(),
            warnings: result.warning_count(),
        },
        blockers: groups.blockers,
        project_risks: groups.project_risks,
        polish: groups.polish,
    })
}

#[derive(Serialize)]
struct JsonOutput<'a> {
    version: &'static str,
    diagnostics: &'a [crate::diagnostic::Diagnostic],
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonSummary {
    files_checked: usize,
    errors: usize,
    warnings: usize,
}

#[derive(Serialize)]
struct ReadyOutput<'a> {
    version: &'static str,
    status: &'static str,
    summary: JsonSummary,
    blockers: Vec<&'a Diagnostic>,
    project_risks: Vec<&'a Diagnostic>,
    polish: Vec<&'a Diagnostic>,
}

struct ReadyGroups<'a> {
    status: &'static str,
    blockers: Vec<&'a Diagnostic>,
    project_risks: Vec<&'a Diagnostic>,
    polish: Vec<&'a Diagnostic>,
}

impl<'a> ReadyGroups<'a> {
    fn from_result(result: &'a CheckResult) -> Self {
        let mut blockers = Vec::new();
        let mut project_risks = Vec::new();
        let mut polish = Vec::new();

        for diagnostic in &result.diagnostics {
            match diagnostic.severity {
                Severity::Error => blockers.push(diagnostic),
                Severity::Warning if is_project_risk(diagnostic.code) => {
                    project_risks.push(diagnostic);
                }
                Severity::Warning => polish.push(diagnostic),
            }
        }

        let status = if !blockers.is_empty() {
            "not ready"
        } else if !project_risks.is_empty() || !polish.is_empty() {
            "ready with warnings"
        } else {
            "ready"
        };

        Self {
            status,
            blockers,
            project_risks,
            polish,
        }
    }
}

fn is_project_risk(code: &str) -> bool {
    matches!(
        &code[..3.min(code.len())],
        "ALG" | "AUX" | "BIB" | "BLG" | "CIT" | "FIG" | "LBL" | "LOG" | "PKG" | "PRJ" | "RDY"
            | "REF" | "SYN" | "TAB"
    )
}

fn push_ready_group(output: &mut String, title: &str, diagnostics: &[&Diagnostic]) {
    output.push('\n');
    output.push_str(title);
    output.push_str(":\n");

    if diagnostics.is_empty() {
        output.push_str("- none\n");
        return;
    }

    for diagnostic in diagnostics {
        let hint = diagnostic
            .hint
            .as_ref()
            .map(|hint| format!("; hint: {hint}"))
            .unwrap_or_default();
        output.push_str(&format!(
            "- {}[{}] {}:{}:{} {}{}\n",
            diagnostic.severity.as_str(),
            diagnostic.code,
            display_path(&diagnostic.file),
            diagnostic.line,
            diagnostic.column,
            diagnostic.message,
            hint
        ));
    }
}

fn sarif_result(diagnostic: &Diagnostic, root: &Path) -> serde_json::Value {
    json!({
        "ruleId": diagnostic.code,
        "level": sarif_level(diagnostic.severity),
        "message": {
            "text": diagnostic.message,
        },
        "locations": [
            {
                "physicalLocation": {
                    "artifactLocation": {
                        "uri": sarif_uri(&diagnostic.file, root),
                    },
                    "region": {
                        "startLine": diagnostic.line,
                        "startColumn": diagnostic.column,
                    },
                },
            },
        ],
        "fingerprints": {
            "paperLinter/v1": diagnostic_fingerprint(diagnostic, root),
        },
        "properties": sarif_properties(diagnostic),
    })
}

fn sarif_properties(diagnostic: &Diagnostic) -> serde_json::Value {
    match &diagnostic.hint {
        Some(hint) => json!({ "hint": hint }),
        None => json!({}),
    }
}

fn lsp_diagnostic(diagnostic: &Diagnostic) -> serde_json::Value {
    let line = diagnostic.line.saturating_sub(1);
    let character = diagnostic.column.saturating_sub(1);
    let mut value = json!({
        "range": {
            "start": {
                "line": line,
                "character": character,
            },
            "end": {
                "line": line,
                "character": character + 1,
            },
        },
        "severity": lsp_severity(diagnostic.severity),
        "code": diagnostic.code,
        "source": "paper-linter",
        "message": diagnostic.message,
    });

    if let Some(hint) = &diagnostic.hint {
        value["data"] = json!({ "hint": hint });
    }

    value
}

fn lsp_severity(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
    }
}

fn file_uri(path: &Path) -> String {
    let raw = path
        .to_string_lossy()
        .replace('\\', "/")
        .chars()
        .flat_map(|character| match character {
            '%' => "%25".chars().collect::<Vec<_>>(),
            ' ' => "%20".chars().collect(),
            '#' => "%23".chars().collect(),
            '?' => "%3F".chars().collect(),
            character => vec![character],
        })
        .collect::<String>();

    if raw.starts_with('/') {
        format!("file://{raw}")
    } else {
        format!("file:///{raw}")
    }
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn sarif_uri(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn display_path(path: &Path) -> String {
    if path.is_absolute() {
        if let Ok(current_dir) = env::current_dir() {
            if let Ok(relative) = path.strip_prefix(current_dir) {
                if !relative.as_os_str().is_empty() {
                    return relative.display().to_string();
                }
            }
        }
    }

    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::checker::CheckResult;
    use crate::diagnostic::{Diagnostic, Severity};

    use super::{display_path, render_ready_json, render_ready_text, render_sarif, render_text};

    #[test]
    fn text_output_uses_paths_relative_to_current_directory() {
        let path = std::env::current_dir()
            .unwrap()
            .join("tmp/sample-paper/root.tex");
        let result = CheckResult {
            diagnostics: vec![Diagnostic::new(
                "LBL001",
                Severity::Warning,
                "label 'sec:problem_statement' is never referenced",
                path,
                180,
                29,
            )],
            files_checked: 1,
        };

        let text = render_text(&result);

        assert!(text.starts_with("tmp/sample-paper/root.tex:180:29: warning[LBL001]"));
        assert!(!text.contains("/tmp/sample-paper/root.tex:180:29"));
    }

    #[test]
    fn display_path_keeps_external_absolute_paths() {
        let path = PathBuf::from("/external/paper/root.tex");

        assert_eq!(display_path(&path), "/external/paper/root.tex");
    }

    #[test]
    fn sarif_output_contains_rules_results_and_fingerprints() {
        let root = PathBuf::from("/tmp/paper");
        let result = CheckResult {
            diagnostics: vec![Diagnostic::new(
                "WS001",
                Severity::Warning,
                "trailing whitespace",
                "/tmp/paper/main.tex",
                2,
                5,
            )
            .with_hint("remove trailing whitespace")],
            files_checked: 1,
        };

        let value: serde_json::Value =
            serde_json::from_str(&render_sarif(&result, &root).unwrap()).unwrap();

        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "paper-linter");
        assert_eq!(value["runs"][0]["results"][0]["ruleId"], "WS001");
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            "main.tex"
        );
        assert_eq!(
            value["runs"][0]["results"][0]["fingerprints"]["paperLinter/v1"],
            "WS001:main.tex:2:5:trailing whitespace"
        );
        assert_eq!(
            value["runs"][0]["results"][0]["properties"]["hint"],
            "remove trailing whitespace"
        );
    }

    #[test]
    fn ready_output_groups_diagnostics() {
        let result = CheckResult {
            diagnostics: vec![
                Diagnostic::new(
                    "FIG001",
                    Severity::Error,
                    "asset missing",
                    "paper.tex",
                    1,
                    1,
                ),
                Diagnostic::new(
                    "CIT002",
                    Severity::Warning,
                    "unused citation",
                    "paper.tex",
                    2,
                    1,
                ),
                Diagnostic::new(
                    "WS001",
                    Severity::Warning,
                    "trailing whitespace",
                    "paper.tex",
                    3,
                    5,
                ),
            ],
            files_checked: 1,
        };

        let text = render_ready_text(&result);
        assert!(text.contains("submission readiness: not ready"));
        assert!(text.contains("Blockers:"));
        assert!(text.contains("Project risks:"));
        assert!(text.contains("Polish:"));

        let json: serde_json::Value =
            serde_json::from_str(&render_ready_json(&result).unwrap()).unwrap();
        assert_eq!(json["status"], "not ready");
        assert_eq!(json["blockers"][0]["code"], "FIG001");
        assert_eq!(json["project_risks"][0]["code"], "CIT002");
        assert_eq!(json["polish"][0]["code"], "WS001");
    }
}
