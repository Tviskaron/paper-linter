use serde::Serialize;
use serde_json::json;
use std::path::Path;

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
            diagnostic.file.display(),
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::checker::CheckResult;
    use crate::diagnostic::{Diagnostic, Severity};

    use super::render_sarif;

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
}
