use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::checker::CheckResult;
use crate::config::LinterConfig;
use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::scan_latex_with_aliases;
use crate::project::{normalize_virtual_path, ProjectIndex};
use crate::rule_policy;
use crate::rules::{all_project_rules, all_rules, rule_infos};

const VIRTUAL_ROOT: &str = "/project";
const INPUT_LIMIT_BYTES: usize = 250 * 1024 * 1024;

#[cfg(feature = "web")]
use wasm_bindgen::prelude::*;

#[cfg_attr(feature = "web", wasm_bindgen)]
pub struct PaperLinter {
    files: BTreeMap<PathBuf, Vec<u8>>,
    total_bytes: usize,
}

#[cfg_attr(feature = "web", wasm_bindgen)]
impl PaperLinter {
    #[cfg_attr(feature = "web", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            total_bytes: 0,
        }
    }

    pub fn add_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let normalized = normalize_upload_path(path)?;
        let previous = self.files.get(&normalized).map_or(0, Vec::len);
        let total = self.total_bytes - previous + bytes.len();
        if total > INPUT_LIMIT_BYTES {
            return Err("input exceeds the 250 MB limit after excluding generated files".to_string());
        }
        self.files.insert(normalized, bytes.to_vec());
        self.total_bytes = total;
        Ok(())
    }

    pub fn check(&self, options_json: &str) -> String {
        match self.check_inner(options_json) {
            Ok(output) => serde_json::to_string(&output).unwrap_or_else(error_json),
            Err(message) => error_json(message),
        }
    }

    pub fn rules_json(&self) -> String {
        let rules = rule_infos()
            .iter()
            .map(|rule| WebRuleView {
                enabled_by_default: rule_policy::enabled_by_default(rule.code)
                    && !rule_policy::strict_only(rule.code),
                code: rule.code,
                severity: rule.default_severity.as_str(),
                name: rule.name,
                summary: rule.summary,
                strict_only: rule_policy::strict_only(rule.code),
                family: rule_family(rule.code),
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&WebRulesOutput { rules }).unwrap_or_else(error_json)
    }
}

impl Default for PaperLinter {
    fn default() -> Self {
        Self::new()
    }
}

impl PaperLinter {
    fn check_inner(&self, options_json: &str) -> Result<WebCheckOutput, String> {
        let mut options: WebCheckOptions =
            serde_json::from_str(options_json).map_err(|error| error.to_string())?;
        if options.preset.is_none() {
            options.preset = Some("standard".to_string());
        }

        let mut select = options.select.clone();
        let mut ignore = options.ignore.clone();
        let mut config = LinterConfig::default();
        if let Some(preset) = options.preset.as_deref() {
            config = LinterConfig::load_preset(preset).map_err(|error| error.to_string())?;
            config.merge_into_options(&mut select, &mut ignore);
        }
        let strict = options.strict || config.strict;

        let root_files = choose_root_files(&self.files, options.all_tex);
        let project = ProjectIndex::build_virtual_with_aliases(
            PathBuf::from(VIRTUAL_ROOT),
            &root_files,
            &self.files,
            &config.aliases,
            options.all_tex,
        )
        .map_err(|error| error.to_string())?;

        let mut diagnostics = Vec::new();
        for file in &project.files {
            for rule in all_rules() {
                if rule_enabled(
                    rule.code(),
                    rule.strict_only(),
                    &select,
                    &ignore,
                    strict,
                    options.all_rules,
                    &config,
                ) {
                    diagnostics.extend(rule.check_file(&file.path, &file.content));
                }
            }
        }

        for rule in all_project_rules() {
            if rule_enabled(
                rule.code(),
                rule.strict_only(),
                &select,
                &ignore,
                strict,
                options.all_rules,
                &config,
            ) {
                diagnostics.extend(rule.check_project(&project));
            }
        }

        diagnostics.extend(check_virtual_citations(
            &project,
            &self.files,
            &select,
            &ignore,
            strict,
            options.all_rules,
            &config,
        ));
        diagnostics.extend(check_virtual_missing_includes(
            &project,
            &self.files,
            &select,
            &ignore,
            strict,
            options.all_rules,
            &config,
        ));

        diagnostics.retain(|diagnostic| {
            code_enabled(
                diagnostic.code,
                &select,
                &ignore,
                strict,
                options.all_rules,
                &config,
            )
        });
        if strict {
            for diagnostic in &mut diagnostics {
                if diagnostic.severity == Severity::Warning
                    && !rule_policy::never_promote_to_error(diagnostic.code)
                {
                    diagnostic.severity = Severity::Error;
                }
            }
        }
        for diagnostic in &mut diagnostics {
            diagnostic.file = relative_virtual_path(&diagnostic.file);
        }
        diagnostics.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then(left.line.cmp(&right.line))
                .then(left.column.cmp(&right.column))
                .then(left.code.cmp(right.code))
        });

        let checked_files = project
            .files
            .iter()
            .map(|file| relative_virtual_path(&file.path))
            .collect::<Vec<_>>();
        let result = CheckResult {
            files_checked: checked_files.len(),
            checked_files: checked_files.clone(),
            diagnostics,
        };
        let errors = result.error_count();
        let warnings = result.warning_count();
        let by_rule = by_rule(&result);

        Ok(WebCheckOutput {
            version: env!("CARGO_PKG_VERSION"),
            diagnostics: result.diagnostics,
            summary: WebSummary {
                files_checked: result.files_checked,
                errors,
                warnings,
            },
            checked_files: checked_files
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            report: WebReport { by_rule },
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct WebCheckOptions {
    preset: Option<String>,
    select: Vec<String>,
    ignore: Vec<String>,
    strict: bool,
    all_rules: bool,
    all_tex: bool,
}

impl Default for WebCheckOptions {
    fn default() -> Self {
        Self {
            preset: Some("standard".to_string()),
            select: Vec::new(),
            ignore: Vec::new(),
            strict: false,
            all_rules: false,
            all_tex: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct WebCheckOutput {
    version: &'static str,
    diagnostics: Vec<Diagnostic>,
    summary: WebSummary,
    checked_files: Vec<String>,
    report: WebReport,
}

#[derive(Debug, Serialize)]
struct WebSummary {
    files_checked: usize,
    errors: usize,
    warnings: usize,
}

#[derive(Debug, Serialize)]
struct WebReport {
    by_rule: Vec<WebRuleCount>,
}

#[derive(Debug, Serialize)]
struct WebRuleCount {
    code: &'static str,
    name: &'static str,
    count: usize,
}

#[derive(Debug, Serialize)]
struct WebRulesOutput {
    rules: Vec<WebRuleView>,
}

#[derive(Debug, Serialize)]
struct WebRuleView {
    code: &'static str,
    severity: &'static str,
    name: &'static str,
    summary: &'static str,
    enabled_by_default: bool,
    strict_only: bool,
    family: &'static str,
}

fn rule_family(code: &'static str) -> &'static str {
    let end = code
        .char_indices()
        .find_map(|(index, character)| character.is_ascii_digit().then_some(index))
        .unwrap_or(code.len());
    &code[..end]
}

fn normalize_upload_path(path: &str) -> Result<PathBuf, String> {
    let path = path.replace('\\', "/");
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return Err("empty path".to_string());
    }
    let normalized = normalize_virtual_path(Path::new(trimmed));
    if normalized == Path::new(VIRTUAL_ROOT) {
        return Err("empty path".to_string());
    }
    Ok(normalized)
}

fn choose_root_files(files: &BTreeMap<PathBuf, Vec<u8>>, all_tex: bool) -> Vec<PathBuf> {
    let tex_files: Vec<_> = files
        .keys()
        .filter(|path| is_ext(path, "tex"))
        .cloned()
        .collect();
    if all_tex {
        return tex_files;
    }
    let mut document_roots: Vec<_> = tex_files
        .iter()
        .filter(|path| {
            read_uploaded_string(files, path).is_some_and(|content| {
                content.contains("\\documentclass") || content.contains("\\begin{document}")
            })
        })
        .cloned()
        .collect();
    if document_roots.is_empty() {
        document_roots = tex_files;
    }
    document_roots.sort_by_key(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        (
            !matches!(
                name.to_ascii_lowercase().as_str(),
                "main.tex" | "paper.tex" | "article.tex" | "manuscript.tex" | "ms.tex"
            ),
            path.clone(),
        )
    });
    document_roots.into_iter().take(1).collect()
}

fn rule_enabled(
    code: &str,
    strict_only: bool,
    select: &[String],
    ignore: &[String],
    strict: bool,
    all_rules: bool,
    config: &LinterConfig,
) -> bool {
    code_enabled(code, select, ignore, strict, all_rules, config)
        && (all_rules || !strict_only || strict || !select.is_empty())
}

fn code_enabled(
    code: &str,
    select: &[String],
    ignore: &[String],
    strict: bool,
    all_rules: bool,
    config: &LinterConfig,
) -> bool {
    if config
        .disable
        .iter()
        .any(|pattern| code.starts_with(pattern))
    {
        return false;
    }
    if config
        .enable
        .iter()
        .any(|pattern| code.starts_with(pattern))
    {
        return !ignore.iter().any(|pattern| code.starts_with(pattern));
    }
    if all_rules {
        return !ignore.iter().any(|pattern| code.starts_with(pattern));
    }
    rule_policy::code_is_enabled(code, select, ignore, strict)
}

fn check_virtual_missing_includes(
    project: &ProjectIndex,
    files: &BTreeMap<PathBuf, Vec<u8>>,
    select: &[String],
    ignore: &[String],
    strict: bool,
    all_rules: bool,
    config: &LinterConfig,
) -> Vec<Diagnostic> {
    if !code_enabled("PRJ001", select, ignore, strict, all_rules, config) {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    for file in &project.files {
        let scan = scan_latex_with_aliases(file.path.clone(), &file.content, &config.aliases);
        for include in scan.includes {
            if resolve_uploaded_tex(&project.root, &file.path, &include.raw_path, files).is_none() {
                diagnostics.push(
                    Diagnostic::new(
                        "PRJ001",
                        Severity::Error,
                        format!("included file '{}' was not found", include.raw_path),
                        include.location.file,
                        include.location.line,
                        include.location.column,
                    )
                    .with_hint("check the path or add the missing source file"),
                );
            }
        }
    }
    diagnostics
}

fn check_virtual_citations(
    project: &ProjectIndex,
    files: &BTreeMap<PathBuf, Vec<u8>>,
    select: &[String],
    ignore: &[String],
    strict: bool,
    all_rules: bool,
    config: &LinterConfig,
) -> Vec<Diagnostic> {
    let cit_enabled = code_enabled("CIT001", select, ignore, strict, all_rules, config)
        || code_enabled("CIT003", select, ignore, strict, all_rules, config)
        || code_enabled("CIT012", select, ignore, strict, all_rules, config);
    if !cit_enabled {
        return Vec::new();
    }

    let mut cited = Vec::new();
    let mut bib_paths = BTreeSet::new();
    for file in &project.files {
        cited.extend(find_citation_uses(
            &file.path,
            &file.content,
            &config.aliases.cites,
        ));
        for bibliography in &project.bibliographies {
            if bibliography.location.file == file.path {
                bib_paths.insert(resolve_uploaded_bib(&file.path, &bibliography.raw_path));
            }
        }
    }

    let mut known = HashSet::new();
    let mut diagnostics = Vec::new();
    for bib_path in bib_paths {
        if let Some(content) = read_uploaded_string(files, &bib_path) {
            known.extend(parse_bib_keys(&content));
            continue;
        }
        let mut bbl_path = bib_path.clone();
        bbl_path.set_extension("bbl");
        if let Some(content) = read_uploaded_string(files, &bbl_path) {
            known.extend(parse_bbl_keys_minimal(&content));
            diagnostics.push(Diagnostic::new(
                "CIT012",
                Severity::Warning,
                format!("bibliography source '{}' is missing; citations are resolved from prebuilt .bbl only", display_relative(&bib_path)),
                project.files.first().map(|file| file.path.clone()).unwrap_or_else(|| PathBuf::from(VIRTUAL_ROOT)),
                1,
                1,
            ));
        } else {
            diagnostics.push(Diagnostic::new(
                "CIT003",
                Severity::Error,
                format!(
                    "bibliography file '{}' was not found",
                    display_relative(&bib_path)
                ),
                project
                    .files
                    .first()
                    .map(|file| file.path.clone())
                    .unwrap_or_else(|| PathBuf::from(VIRTUAL_ROOT)),
                1,
                1,
            ));
        }
    }

    for citation in cited {
        if citation.key == "*" || known.contains(&citation.key) {
            continue;
        }
        diagnostics.push(
            Diagnostic::new(
                "CIT001",
                Severity::Error,
                format!(
                    "citation key '{}' was not found in bibliography",
                    citation.key
                ),
                citation.file,
                citation.line,
                citation.column,
            )
            .with_hint("add the missing bibliography entry or fix the citation key"),
        );
    }
    diagnostics
}

#[derive(Debug)]
struct CitationUse {
    key: String,
    file: PathBuf,
    line: usize,
    column: usize,
}

fn find_citation_uses(file: &Path, content: &str, aliases: &[String]) -> Vec<CitationUse> {
    let mut uses = Vec::new();
    let mut offset = 0;
    while let Some(relative) = content[offset..].find("\\cite") {
        let start = offset + relative;
        let command_end = content[start + 1..]
            .find(|ch: char| !ch.is_ascii_alphabetic())
            .map(|end| start + 1 + end)
            .unwrap_or(content.len());
        let command = &content[start + 1..command_end];
        if command != "cite" && !aliases.iter().any(|alias| alias == command) {
            offset = command_end;
            continue;
        }
        let Some(open) = content[command_end..]
            .find('{')
            .map(|index| command_end + index)
        else {
            offset = command_end;
            continue;
        };
        let Some(close) = content[open + 1..].find('}').map(|index| open + 1 + index) else {
            offset = open + 1;
            continue;
        };
        let (line, column) = line_column(content, open + 1);
        for key in content[open + 1..close]
            .split(',')
            .map(str::trim)
            .filter(|key| !key.is_empty())
        {
            uses.push(CitationUse {
                key: key.to_string(),
                file: file.to_path_buf(),
                line,
                column,
            });
        }
        offset = close + 1;
    }
    uses
}

fn parse_bib_keys(content: &str) -> HashSet<String> {
    let mut keys = HashSet::new();
    let mut offset = 0;
    while let Some(relative) = content[offset..].find('@') {
        let start = offset + relative;
        let Some(open) = content[start..]
            .find('{')
            .or_else(|| content[start..].find('('))
            .map(|index| start + index)
        else {
            break;
        };
        let Some(comma) = content[open + 1..].find(',').map(|index| open + 1 + index) else {
            break;
        };
        let key = content[open + 1..comma].trim();
        if !key.is_empty() {
            keys.insert(key.to_string());
        }
        offset = comma + 1;
    }
    keys
}

fn parse_bbl_keys_minimal(content: &str) -> HashSet<String> {
    let mut keys = HashSet::new();
    let mut offset = 0;
    while let Some(relative) = content[offset..].find("\\bibitem") {
        let start = offset + relative;
        let Some(open) = content[start..].find('{').map(|index| start + index) else {
            break;
        };
        let Some(close) = content[open + 1..].find('}').map(|index| open + 1 + index) else {
            break;
        };
        let key = content[open + 1..close].trim();
        if !key.is_empty() {
            keys.insert(key.to_string());
        }
        offset = close + 1;
    }
    keys
}

fn resolve_uploaded_tex(
    root: &Path,
    current_file: &Path,
    raw_path: &str,
    files: &BTreeMap<PathBuf, Vec<u8>>,
) -> Option<PathBuf> {
    let base = current_file.parent()?;
    let raw = Path::new(raw_path.trim());
    for candidate in [
        base.join(raw),
        PathBuf::from(format!("{}.tex", base.join(raw).display())),
        root.join(raw),
        PathBuf::from(format!("{}.tex", root.join(raw).display())),
    ] {
        let normalized = normalize_virtual_path(&candidate);
        if files.contains_key(&normalized) {
            return Some(normalized);
        }
    }
    None
}

fn resolve_uploaded_bib(current_file: &Path, raw_path: &str) -> PathBuf {
    let mut path = PathBuf::from(raw_path.trim());
    if path.extension().is_none() {
        path.set_extension("bib");
    }
    normalize_virtual_path(
        &current_file
            .parent()
            .unwrap_or_else(|| Path::new(VIRTUAL_ROOT))
            .join(path),
    )
}

fn read_uploaded_string(files: &BTreeMap<PathBuf, Vec<u8>>, path: &Path) -> Option<String> {
    String::from_utf8(files.get(&normalize_virtual_path(path))?.clone()).ok()
}

fn is_ext(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn relative_virtual_path(path: &Path) -> PathBuf {
    path.strip_prefix(VIRTUAL_ROOT)
        .unwrap_or(path)
        .to_path_buf()
}

fn display_relative(path: &Path) -> String {
    relative_virtual_path(path).to_string_lossy().to_string()
}

fn line_column(content: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for character in content[..offset.min(content.len())].chars() {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn by_rule(result: &CheckResult) -> Vec<WebRuleCount> {
    let mut counts = BTreeMap::new();
    for diagnostic in &result.diagnostics {
        *counts.entry(diagnostic.code).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .map(|(code, count)| WebRuleCount {
            code,
            name: rule_infos()
                .iter()
                .find(|rule| rule.code == code)
                .map(|rule| rule.name)
                .unwrap_or("unknown rule"),
            count,
        })
        .collect()
}

fn error_json(message: impl ToString) -> String {
    serde_json::json!({
        "error": message.to_string(),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::PaperLinter;
    use serde_json::Value;

    fn check(files: &[(&str, &[u8])], options: &str) -> Value {
        let mut linter = PaperLinter::new();
        for (path, bytes) in files {
            linter.add_file(path, bytes).expect("file should add");
        }
        let output: Value = serde_json::from_str(&linter.check(options)).expect("valid output");
        if output.get("error").is_some() {
            panic!("{output}");
        }
        output
    }

    fn codes(output: &Value) -> Vec<String> {
        output["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|diagnostic| diagnostic["code"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn web_output_uses_relative_paths() {
        let output = check(
            &[(
                "paper/main.tex",
                br"\documentclass{article}
\begin{document}
\cite{missing}
\bibliography{refs}
\end{document}
",
            )],
            r#"{"select":["CIT"],"preset":null}"#,
        );

        assert_eq!(output["diagnostics"][0]["file"], "paper/main.tex");
    }

    #[test]
    fn follows_includes_in_virtual_project() {
        let output = check(
            &[
                (
                    "main.tex",
                    br"\documentclass{article}
\begin{document}
\input{sections/method}
\end{document}
",
                ),
                ("sections/method.tex", br"\section{Method}"),
            ],
            r#"{"preset":"standard"}"#,
        );

        assert_eq!(output["summary"]["files_checked"], 2);
    }

    #[test]
    fn reports_missing_citation_key() {
        let output = check(
            &[
                (
                    "main.tex",
                    br"\documentclass{article}
\begin{document}
\cite{missing}
\bibliography{refs}
\end{document}
",
                ),
                (
                    "refs.bib",
                    br"@article{present, title={A}, author={B}, year={2024}}",
                ),
            ],
            r#"{"select":["CIT"],"preset":null}"#,
        );

        assert!(codes(&output).contains(&"CIT001".to_string()));
    }

    #[test]
    fn accepts_bbl_fallback_for_missing_bib() {
        let output = check(
            &[
                (
                    "main.tex",
                    br"\documentclass{article}
\begin{document}
\cite{known}
\bibliography{refs}
\end{document}
",
                ),
                ("refs.bbl", br"\bibitem{known} Known paper."),
            ],
            r#"{"select":["CIT"],"preset":null}"#,
        );

        assert_eq!(codes(&output), vec!["CIT012"]);
    }

    #[test]
    fn resolves_uploaded_figure_assets() {
        let output = check(
            &[
                (
                    "main.tex",
                    br"\documentclass{article}
\begin{document}
\includegraphics{figures/model.png}
\end{document}
",
                ),
                (
                    "figures/model.png",
                    b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x02\x80\0\0\x01\xe0",
                ),
            ],
            r#"{"select":["FIG001"],"preset":null}"#,
        );

        assert!(!codes(&output).contains(&"FIG001".to_string()));
    }
}
