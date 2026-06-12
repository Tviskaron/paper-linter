use std::fs;
use std::io;
use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};

pub fn check_artifacts(dir: &Path) -> io::Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        match extension.as_str() {
            "log" => diagnostics.extend(parse_log(&path)?),
            "blg" => diagnostics.extend(parse_blg(&path)?),
            "aux" => diagnostics.extend(parse_aux(&path)?),
            "bcf" => diagnostics.extend(parse_bcf(&path)?),
            _ => {}
        }
    }

    diagnostics.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.code.cmp(right.code))
    });

    Ok(diagnostics)
}

fn parse_log(path: &Path) -> io::Result<Vec<Diagnostic>> {
    let content = fs::read_to_string(path)?;
    let mut diagnostics = Vec::new();

    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        if line.starts_with('!') {
            diagnostics.push(diagnostic_for_artifact(
                "LOG001",
                Severity::Error,
                line.trim_start_matches('!').trim(),
                path,
                line_number,
            ));
            continue;
        }

        if let Some(message) = line.strip_prefix("LaTeX Warning: ") {
            if message.contains("undefined") {
                diagnostics.push(diagnostic_for_artifact(
                    "LOG001",
                    Severity::Warning,
                    message,
                    path,
                    line_number,
                ));
            }
        }
    }

    Ok(diagnostics)
}

fn parse_blg(path: &Path) -> io::Result<Vec<Diagnostic>> {
    let content = fs::read_to_string(path)?;
    let mut diagnostics = Vec::new();

    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("Repeated entry")
            || trimmed.starts_with("I was expecting a")
            || trimmed.contains("---line")
            || trimmed.starts_with("Warning--")
            || trimmed.starts_with("Error")
        {
            diagnostics.push(diagnostic_for_artifact(
                "BLG001",
                Severity::Error,
                trimmed,
                path,
                line_number,
            ));
        }
    }

    Ok(diagnostics)
}

fn parse_aux(path: &Path) -> io::Result<Vec<Diagnostic>> {
    let content = fs::read_to_string(path)?;
    let mut diagnostics = Vec::new();
    let mut cited = collect_aux_keys(&content, "citation");
    cited.sort();
    cited.dedup();

    for key in cited {
        if key == "*" {
            continue;
        }
        if !content.contains(&format!("\\bibcite{{{key}}}")) {
            diagnostics.push(diagnostic_for_artifact(
                "AUX001",
                Severity::Warning,
                format!("citation '{key}' has no resolved bibcite entry in .aux"),
                path,
                1,
            ));
        }
    }

    Ok(diagnostics)
}

fn parse_bcf(path: &Path) -> io::Result<Vec<Diagnostic>> {
    let content = fs::read_to_string(path)?;
    let mut diagnostics = Vec::new();

    for (index, line) in content.lines().enumerate() {
        if line.contains("<bcf:citekey order=\"0\"") && line.contains("missing") {
            diagnostics.push(diagnostic_for_artifact(
                "AUX001",
                Severity::Warning,
                line.trim(),
                path,
                index + 1,
            ));
        }
    }

    Ok(diagnostics)
}

fn collect_aux_keys(content: &str, command: &str) -> Vec<String> {
    let marker = format!("\\{command}{{");
    let mut keys = Vec::new();
    let mut offset = 0;

    while let Some(relative) = content[offset..].find(&marker) {
        let start = offset + relative + marker.len();
        let end = content[start..]
            .find('}')
            .map(|index| start + index)
            .unwrap_or(start);
        let key = content[start..end].trim();
        if !key.is_empty() {
            keys.push(key.to_string());
        }
        offset = end + 1;
    }

    keys
}

fn diagnostic_for_artifact(
    code: &'static str,
    severity: Severity,
    message: impl Into<String>,
    path: &Path,
    line: usize,
) -> Diagnostic {
    Diagnostic::new(code, severity, message, path, line, 1)
}

pub fn compile_regression_diagnostics(
    compile_result_path: &Path,
    paper_root: &Path,
) -> io::Result<Vec<Diagnostic>> {
    let content = fs::read_to_string(compile_result_path)?;
    let value: serde_json::Value = serde_json::from_str(&content).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, error.to_string())
    })?;

    let papers = value
        .get("papers")
        .and_then(|papers| papers.as_array())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing papers array"))?;

    let paper_id = paper_id_from_root(paper_root).unwrap_or_default();

    let mut diagnostics = Vec::new();

    for paper in papers {
        let Some(id) = paper.get("arxiv_id").and_then(|id| id.as_str()) else {
            continue;
        };
        if id != paper_id {
            continue;
        }

        if paper.get("status").and_then(|status| status.as_str()) == Some("compile_failed") {
            diagnostics.push(
                Diagnostic::new(
                    "RDY001",
                    Severity::Error,
                    format!("paper '{id}' failed to compile in baseline corpus"),
                    compile_result_path,
                    1,
                    1,
                )
                .with_hint("inspect compile logs and fix build blockers before submission"),
            );
        }

        if let Some(comparison) = paper.get("comparison") {
            if comparison.get("page_match").and_then(|value| value.as_bool()) == Some(false) {
                let compiled = comparison
                    .get("compiled_pages")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                let reference = comparison
                    .get("reference_pages")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                diagnostics.push(
                    Diagnostic::new(
                        "RDY002",
                        Severity::Error,
                        format!(
                            "compiled PDF has {compiled} pages but reference PDF has {reference} pages"
                        ),
                        compile_result_path,
                        1,
                        1,
                    )
                    .with_hint("missing sections, figures, or bibliography can change page count"),
                );
            }

            if let Some(ratio) = comparison.get("size_ratio").and_then(|value| value.as_f64()) {
                if ratio < 0.85 {
                    diagnostics.push(
                        Diagnostic::new(
                            "RDY003",
                            Severity::Warning,
                            format!("compiled PDF size ratio is {:.3}, which is unusually low", ratio),
                            compile_result_path,
                            1,
                            1,
                        )
                        .with_hint("check for missing figures, fonts, or bibliography content"),
                    );
                }
            }
        }

        break;
    }

    Ok(diagnostics)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{check_artifacts, compile_regression_diagnostics};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paper-linter-{name}-{stamp}"));
        fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    #[test]
    fn parses_log_and_blg_artifacts() {
        let dir = temp_dir("artifacts");
        fs::write(dir.join("main.log"), "! Undefined control sequence.\n").unwrap();
        fs::write(
            dir.join("main.blg"),
            "Repeated entry---line 4 of file refs.bib\n",
        )
        .unwrap();

        let diagnostics = check_artifacts(&dir).expect("artifacts");
        assert!(diagnostics.iter().any(|diag| diag.code == "LOG001"));
        assert!(diagnostics.iter().any(|diag| diag.code == "BLG001"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn flags_compile_regressions_from_baseline_json() {
        let dir = temp_dir("compile-regression");
        let paper_dir = dir.join("2203.10833");
        fs::create_dir_all(&paper_dir).unwrap();
        let baseline = dir.join("compile_result.json");
        fs::write(
            &baseline,
            r#"{"papers":[{"arxiv_id":"2203.10833","status":"compiled","comparison":{"page_match":false,"compiled_pages":11,"reference_pages":16,"size_ratio":0.8}}]}"#,
        )
        .unwrap();

        let diagnostics =
            compile_regression_diagnostics(&baseline, &paper_dir).expect("regressions");
        assert!(diagnostics.iter().any(|diag| diag.code == "RDY002"));
        assert!(diagnostics.iter().any(|diag| diag.code == "RDY003"));
        let _ = fs::remove_dir_all(dir);
    }
}

fn paper_id_from_root(paper_root: &Path) -> Option<String> {
    if paper_root.is_file() {
        paper_root
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_string)
    } else {
        paper_root
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
    }
}
