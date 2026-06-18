use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::checker::CheckResult;
use crate::config::LinterConfig;
use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::{scan_latex_with_aliases, ScanAliases};
use crate::latex::significant::{mask_discarded_macro_arguments, mask_inactive_regions};
use crate::project::{normalize_virtual_path, ProjectIndex};
use crate::rule_policy;
use crate::rules::{all_project_rules, all_rules, rule_infos};

const VIRTUAL_ROOT: &str = "/project";
const INPUT_LIMIT_BYTES: usize = 250 * 1024 * 1024;
const MAIN_LIKE_NAMES: [&str; 5] = [
    "main.tex",
    "paper.tex",
    "article.tex",
    "manuscript.tex",
    "ms.tex",
];
const MAX_ROOT_VIEW_COUNT: usize = 4;

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
            return Err(
                "input exceeds the 250 MB limit after excluding generated files".to_string(),
            );
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

        let graph = VirtualGraph::build(&self.files, &config.aliases);
        let active_view = graph.active_view(&options)?;
        let views = graph.views_for_active(&active_view);
        let root_files = active_view.root_files(&graph);
        let project = ProjectIndex::build_virtual_with_aliases(
            PathBuf::from(VIRTUAL_ROOT),
            &root_files,
            &self.files,
            &config.aliases,
            active_view.all_tex,
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
            active_view_id: active_view.id,
            views,
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
    root: Option<String>,
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
            root: None,
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
    active_view_id: String,
    views: Vec<WebProjectView>,
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

#[derive(Debug, Clone, Serialize)]
struct WebProjectView {
    id: String,
    kind: &'static str,
    label: String,
    root: Option<String>,
    file_count: usize,
    reason: String,
    preferred: bool,
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

#[derive(Debug, Clone)]
struct ActiveView {
    id: String,
    root: Option<PathBuf>,
    all_tex: bool,
}

impl ActiveView {
    fn root_files(&self, graph: &VirtualGraph) -> Vec<PathBuf> {
        if self.all_tex {
            return graph.tex_files.iter().cloned().collect();
        }
        self.root.iter().cloned().collect()
    }
}

#[derive(Debug, Clone)]
struct RootCandidate {
    root: PathBuf,
    reachable: BTreeSet<PathBuf>,
    reason: String,
    score: RootScore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RootScore {
    source: i32,
    main_like: i32,
    top_level: i32,
    ordinary_name: i32,
    fanout: i32,
}

#[derive(Debug)]
struct VirtualGraph {
    tex_files: BTreeSet<PathBuf>,
    candidates: Vec<RootCandidate>,
}

impl VirtualGraph {
    fn build(files: &BTreeMap<PathBuf, Vec<u8>>, aliases: &ScanAliases) -> Self {
        let tex_files = files
            .keys()
            .filter(|path| is_ext(path, "tex"))
            .cloned()
            .collect::<BTreeSet<_>>();
        let edges = build_virtual_edges(files, &tex_files, aliases);
        let candidates = rank_virtual_roots(files, &tex_files, &edges);
        Self {
            tex_files,
            candidates,
        }
    }

    fn views(&self) -> Vec<WebProjectView> {
        if self.tex_files.is_empty() {
            return Vec::new();
        }

        let root_views = self.root_views();
        if root_views.len() <= 1 {
            return root_views;
        }

        let mut views = Vec::with_capacity(root_views.len() + 1);
        views.push(root_views[0].clone());
        views.push(self.all_view());
        views.extend(root_views.into_iter().skip(1));
        views
    }

    fn views_for_active(&self, active_view: &ActiveView) -> Vec<WebProjectView> {
        let mut views = self.views();
        if active_view.all_tex
            && !self.tex_files.is_empty()
            && !views.iter().any(|view| view.id == "all")
        {
            views.push(self.all_view());
        }
        views
    }

    fn all_view(&self) -> WebProjectView {
        WebProjectView {
            id: "all".to_string(),
            kind: "all",
            label: "All .tex".to_string(),
            root: None,
            file_count: self.tex_files.len(),
            reason: "all source .tex files".to_string(),
            preferred: false,
        }
    }

    fn active_view(&self, options: &WebCheckOptions) -> Result<ActiveView, String> {
        let views = self.views();
        if self.tex_files.is_empty() {
            return Ok(ActiveView {
                id: "all".to_string(),
                root: None,
                all_tex: true,
            });
        }

        if options.all_tex {
            return Ok(ActiveView {
                id: "all".to_string(),
                root: None,
                all_tex: true,
            });
        }

        if let Some(root) = options
            .root
            .as_deref()
            .filter(|root| !root.trim().is_empty())
        {
            let normalized = normalize_upload_path(root)?;
            if !self.tex_files.contains(&normalized) {
                return Err(format!("selected root '{}' was not found", root));
            }
            let id = root_view_id(&normalized);
            return Ok(ActiveView {
                id,
                root: Some(normalized),
                all_tex: false,
            });
        }

        if let Some(view) = views.iter().find(|view| view.kind == "root") {
            let root = view
                .root
                .as_deref()
                .ok_or_else(|| "root view is missing root path".to_string())?;
            return Ok(ActiveView {
                id: view.id.clone(),
                root: Some(normalize_upload_path(root)?),
                all_tex: false,
            });
        }

        Ok(ActiveView {
            id: "all".to_string(),
            root: None,
            all_tex: true,
        })
    }

    fn root_views(&self) -> Vec<WebProjectView> {
        let mut seen_reachable = BTreeSet::new();
        let mut views = Vec::new();
        for candidate in &self.candidates {
            if views.len() >= MAX_ROOT_VIEW_COUNT {
                break;
            }
            let key = candidate
                .reachable
                .iter()
                .map(|path| display_relative(path))
                .collect::<Vec<_>>()
                .join("\n");
            if !seen_reachable.insert(key) {
                continue;
            }
            views.push(WebProjectView {
                id: root_view_id(&candidate.root),
                kind: "root",
                label: root_label(&candidate.root),
                root: Some(display_relative(&candidate.root)),
                file_count: candidate.reachable.len(),
                reason: candidate.reason.clone(),
                preferred: views.is_empty(),
            });
        }
        views
    }
}

fn build_virtual_edges(
    files: &BTreeMap<PathBuf, Vec<u8>>,
    tex_files: &BTreeSet<PathBuf>,
    aliases: &ScanAliases,
) -> BTreeMap<PathBuf, BTreeSet<PathBuf>> {
    let mut edges = BTreeMap::new();
    for path in tex_files {
        edges.entry(path.clone()).or_insert_with(BTreeSet::new);
        let Some(content) = active_uploaded_tex(files, path) else {
            continue;
        };
        let scan = scan_latex_with_aliases(path.clone(), &content, aliases);
        for include in scan.includes {
            if let Some(child) =
                resolve_uploaded_tex(Path::new(VIRTUAL_ROOT), path, &include.raw_path, files)
            {
                if tex_files.contains(&child) {
                    edges.entry(path.clone()).or_default().insert(child);
                }
            }
        }
    }
    edges
}

fn rank_virtual_roots(
    files: &BTreeMap<PathBuf, Vec<u8>>,
    tex_files: &BTreeSet<PathBuf>,
    edges: &BTreeMap<PathBuf, BTreeSet<PathBuf>>,
) -> Vec<RootCandidate> {
    let mut candidates = BTreeMap::<PathBuf, (String, i32)>::new();

    if let Some(root) = root_from_uploaded_readme(files, tex_files) {
        candidates.insert(root, ("00README.json".to_string(), 1000));
    }
    for root in roots_from_uploaded_magic_comments(files, tex_files) {
        candidates
            .entry(root)
            .or_insert_with(|| ("magic comment".to_string(), 950));
    }

    let document_roots = tex_files
        .iter()
        .filter(|path| {
            active_uploaded_tex(files, path).is_some_and(|content| declares_document(&content))
        })
        .cloned()
        .collect::<Vec<_>>();
    for root in document_roots {
        candidates
            .entry(root)
            .or_insert_with(|| ("document root".to_string(), 800));
    }

    let mut ranked = candidates
        .into_iter()
        .map(|(root, (reason, source_score))| {
            let reachable = collect_virtual_reachable(&root, edges);
            let score = score_root(&root, &reachable, source_score);
            RootCandidate {
                root,
                reachable,
                reason,
                score,
            }
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.root.cmp(&right.root))
    });
    ranked
}

fn score_root(root: &Path, reachable: &BTreeSet<PathBuf>, source: i32) -> RootScore {
    RootScore {
        source,
        main_like: i32::from(
            root.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_main_like_name),
        ),
        top_level: i32::from(root.parent() == Some(Path::new(VIRTUAL_ROOT))),
        ordinary_name: i32::from(!is_likely_alternate_or_service_tex(root)),
        fanout: reachable.len() as i32,
    }
}

fn collect_virtual_reachable(
    root: &Path,
    edges: &BTreeMap<PathBuf, BTreeSet<PathBuf>>,
) -> BTreeSet<PathBuf> {
    let root = normalize_virtual_path(root);
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();
    reachable.insert(root.clone());
    queue.push_back(root);

    while let Some(current) = queue.pop_front() {
        if let Some(children) = edges.get(&current) {
            for child in children {
                if reachable.insert(child.clone()) {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    reachable
}

fn root_from_uploaded_readme(
    files: &BTreeMap<PathBuf, Vec<u8>>,
    tex_files: &BTreeSet<PathBuf>,
) -> Option<PathBuf> {
    let content = read_uploaded_string(files, Path::new("00README.json"))?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    for source in data.get("sources")?.as_array()? {
        if source.get("usage")?.as_str()? != "toplevel" {
            continue;
        }
        let filename = source.get("filename")?.as_str()?;
        let candidate = normalize_virtual_path(Path::new(filename));
        if tex_files.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn roots_from_uploaded_magic_comments(
    files: &BTreeMap<PathBuf, Vec<u8>>,
    tex_files: &BTreeSet<PathBuf>,
) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for path in tex_files {
        let Some(content) = active_uploaded_tex(files, path) else {
            continue;
        };
        for line in content.lines() {
            let trimmed = line.trim();
            let Some(comment) = trimmed.strip_prefix('%') else {
                continue;
            };
            let comment = comment.trim_start();
            let comment = comment.strip_prefix('!').unwrap_or(comment).trim_start();
            let lower = comment.to_ascii_lowercase();
            if !lower.starts_with("tex root") && !lower.starts_with("root") {
                continue;
            }
            let Some((_, value)) = comment.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if let Some(root) = resolve_uploaded_tex(Path::new(VIRTUAL_ROOT), path, value, files) {
                if tex_files.contains(&root) {
                    roots.push(root);
                }
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn active_uploaded_tex(files: &BTreeMap<PathBuf, Vec<u8>>, path: &Path) -> Option<String> {
    let content = read_uploaded_string(files, path)?;
    let content = mask_discarded_macro_arguments(&content);
    Some(mask_inactive_regions(&content))
}

fn declares_document(content: &str) -> bool {
    content.contains("\\documentclass") || content.contains("\\begin{document}")
}

fn is_main_like_name(name: &str) -> bool {
    MAIN_LIKE_NAMES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

fn is_likely_alternate_or_service_tex(path: &Path) -> bool {
    let keywords = [
        "appendix",
        "appendices",
        "backup",
        "camera-ready",
        "cameraready",
        "copy",
        "draft",
        "example",
        "rebuttal",
        "response",
        "revision",
        "sample",
        "supp",
        "supplement",
        "supplementary",
        "template",
        "translation",
        "translated",
    ];

    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        keywords.iter().any(|keyword| text.contains(keyword))
    })
}

fn root_view_id(root: &Path) -> String {
    format!("root:{}", display_relative(root))
}

fn root_label(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| display_relative(root))
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

    fn check_error(files: &[(&str, &[u8])], options: &str) -> String {
        let mut linter = PaperLinter::new();
        for (path, bytes) in files {
            linter.add_file(path, bytes).expect("file should add");
        }
        let output: Value = serde_json::from_str(&linter.check(options)).expect("valid output");
        output["error"].as_str().unwrap().to_string()
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
    fn exposes_ranked_report_views_for_multiple_roots() {
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
                (
                    "paper-v2.tex",
                    br"\documentclass{article}
\begin{document}
Second version.
\end{document}
",
                ),
            ],
            r#"{"preset":null}"#,
        );

        assert_eq!(output["active_view_id"], "root:main.tex");
        let views = output["views"].as_array().unwrap();
        assert_eq!(views.len(), 3);
        assert_eq!(views[0]["label"], "main.tex");
        assert_eq!(views[0]["file_count"], 2);
        assert_eq!(views[1]["id"], "all");
        assert_eq!(views[1]["file_count"], 3);
        assert_eq!(views[2]["label"], "paper-v2.tex");
    }

    #[test]
    fn selected_root_checks_only_that_reachable_graph() {
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
                (
                    "paper-v2.tex",
                    br"\documentclass{article}
\begin{document}
Second version.
\end{document}
",
                ),
            ],
            r#"{"preset":null,"root":"paper-v2.tex"}"#,
        );

        assert_eq!(output["active_view_id"], "root:paper-v2.tex");
        assert_eq!(output["summary"]["files_checked"], 1);
        assert_eq!(output["checked_files"], serde_json::json!(["paper-v2.tex"]));
    }

    #[test]
    fn all_tex_view_checks_every_tex_file() {
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
                (
                    "paper-v2.tex",
                    br"\documentclass{article}
\begin{document}
Second version.
\end{document}
",
                ),
            ],
            r#"{"preset":null,"all_tex":true}"#,
        );

        assert_eq!(output["active_view_id"], "all");
        assert_eq!(output["summary"]["files_checked"], 3);
    }

    #[test]
    fn loose_tex_files_do_not_become_root_tabs() {
        let output = check(
            &[
                ("sections/intro.tex", br"\section{Intro}"),
                ("sections/method.tex", br"\section{Method}"),
                ("notes/old.tex", br"notes"),
            ],
            r#"{"preset":null}"#,
        );

        assert_eq!(output["active_view_id"], "all");
        let views = output["views"].as_array().unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0]["id"], "all");
        assert_eq!(output["summary"]["files_checked"], 3);
    }

    #[test]
    fn readme_root_does_not_duplicate_document_root_view() {
        let output = check(
            &[
                (
                    "00README.json",
                    br#"{"sources":[{"usage":"toplevel","filename":"paper.tex"}]}"#,
                ),
                (
                    "paper.tex",
                    br"\documentclass{article}
\begin{document}
Body.
\end{document}
",
                ),
            ],
            r#"{"preset":null}"#,
        );

        let views = output["views"].as_array().unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0]["label"], "paper.tex");
        assert_eq!(views[0]["reason"], "00README.json");
    }

    #[test]
    fn invalid_selected_root_returns_error() {
        let message = check_error(
            &[(
                "main.tex",
                br"\documentclass{article}
\begin{document}
Body.
\end{document}
",
            )],
            r#"{"preset":null,"root":"missing.tex"}"#,
        );

        assert_eq!(message, "selected root 'missing.tex' was not found");
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
