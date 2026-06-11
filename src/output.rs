use serde::Serialize;

use crate::checker::CheckResult;

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
