use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::latex::significant::{mask_discarded_macro_arguments, mask_inactive_regions};

const MAIN_LIKE_NAMES: [&str; 5] = [
    "main.tex",
    "paper.tex",
    "article.tex",
    "manuscript.tex",
    "ms.tex",
];

type IncludeEdges = Vec<(PathBuf, PathBuf)>;
type IncludeMap = BTreeMap<PathBuf, BTreeSet<PathBuf>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RootMethod {
    Readme,
    MagicComment,
    PrimaryRoot,
    Fallback,
    Unresolved,
}

impl RootMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            RootMethod::Readme => "00README.json",
            RootMethod::MagicComment => "magic comment",
            RootMethod::PrimaryRoot => "primary root",
            RootMethod::Fallback => "fallback",
            RootMethod::Unresolved => "unresolved",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MissingInclude {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub raw_path: String,
}

#[derive(Debug, Clone)]
pub struct ProjectGraph {
    pub paper_dir: PathBuf,
    pub root: Option<PathBuf>,
    pub root_method: RootMethod,
    pub root_candidates: Vec<PathBuf>,
    pub reachable: BTreeSet<PathBuf>,
    pub all_tex: BTreeSet<PathBuf>,
    pub missing_includes: Vec<MissingInclude>,
    pub include_edges: Vec<(PathBuf, PathBuf)>,
}

impl ProjectGraph {
    pub fn analyze(dir: &Path) -> io::Result<Self> {
        let paper_dir = dir.canonicalize()?;
        let all_tex = collect_tex_files(&paper_dir)?;
        let missing_includes = collect_missing_includes(&paper_dir, &all_tex);
        let (include_edges, included_by) = build_include_graph(&paper_dir, &all_tex);

        let (root, root_method, root_candidates) =
            resolve_root(&paper_dir, &all_tex, &included_by, &include_edges);

        let reachable = if let Some(root_path) = &root {
            collect_reachable(root_path, &include_edges)
        } else {
            BTreeSet::new()
        };

        Ok(Self {
            paper_dir,
            root,
            root_method,
            root_candidates,
            reachable,
            all_tex,
            missing_includes,
            include_edges,
        })
    }

    pub fn should_analyze(paths: &[PathBuf]) -> bool {
        paths.iter().any(|path| path.is_dir())
    }

    pub fn analyze_paths(paths: &[PathBuf]) -> io::Result<Vec<Self>> {
        paths
            .iter()
            .filter(|path| path.is_dir())
            .map(|path| Self::analyze(path))
            .collect()
    }

    pub fn tex_files_for_lint(&self, all_tex: bool) -> Vec<PathBuf> {
        let files = if all_tex || self.root.is_none() {
            &self.all_tex
        } else {
            &self.reachable
        };

        files.iter().cloned().collect()
    }
}

fn collect_tex_files(dir: &Path) -> io::Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    collect_tex_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn collect_tex_files_recursive(dir: &Path, files: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_tex_files_recursive(&path, files)?;
        } else if is_tex_file(&path) {
            if let Ok(canonical) = path.canonicalize() {
                files.insert(canonical);
            }
        }
    }
    Ok(())
}

fn is_tex_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("tex"))
}

fn declares_document(content: &str) -> bool {
    content.contains("\\documentclass") || content.contains("\\begin{document}")
}

fn active_tex_content(path: &Path) -> String {
    let content = fs::read_to_string(path).unwrap_or_default();
    let content = mask_discarded_macro_arguments(&content);
    mask_inactive_regions(&content)
}

fn is_main_like_name(name: &str) -> bool {
    MAIN_LIKE_NAMES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

pub fn is_likely_alternate_or_service_tex(path: &Path) -> bool {
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

fn resolve_include_path(paper_dir: &Path, current_file: &Path, target: &str) -> Option<PathBuf> {
    let raw = target.trim();
    if raw.is_empty() {
        return None;
    }

    let raw_path = Path::new(raw);
    if is_explicit_non_tex_input(raw_path) {
        return Some(current_file.parent()?.join(raw_path));
    }

    let base = current_file.parent()?;
    let mut candidates = tex_include_candidates(base, raw_path);
    if !raw_path.is_absolute() {
        candidates.extend(tex_include_candidates(paper_dir, raw_path));
    }

    candidates
        .into_iter()
        .find_map(|candidate| candidate.canonicalize().ok().filter(|path| path.is_file()))
}

fn is_explicit_non_tex_input(raw_path: &Path) -> bool {
    raw_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| !extension.eq_ignore_ascii_case("tex"))
}

fn tex_include_candidates(base: &Path, raw: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let raw_path = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        base.join(raw)
    };

    if raw
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| extension.eq_ignore_ascii_case("tex"))
    {
        candidates.push(raw_path.clone());
    }

    if !raw_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("tex"))
    {
        candidates.push(PathBuf::from(format!("{}.tex", raw_path.display())));
    }

    candidates
}

fn root_from_readme(paper_dir: &Path) -> Option<PathBuf> {
    let readme = paper_dir.join("00README.json");
    let content = fs::read_to_string(&readme).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    for source in data.get("sources")?.as_array()? {
        if source.get("usage")?.as_str()? != "toplevel" {
            continue;
        }
        let filename = source.get("filename")?.as_str()?;
        let candidate = paper_dir.join(filename).canonicalize().ok()?;
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn root_from_magic_comment(paper_dir: &Path, all_tex: &BTreeSet<PathBuf>) -> Option<PathBuf> {
    for path in all_tex {
        let content = active_tex_content(path);
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
            if value.is_empty() {
                continue;
            }
            if let Some(candidate) = resolve_include_path(paper_dir, path, value) {
                return Some(candidate);
            }
        }
    }
    None
}

fn choose_root_candidate(candidates: &[PathBuf], paper_dir: &Path) -> Option<PathBuf> {
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    let paper_root = paper_dir.canonicalize().ok()?;
    let top_level: Vec<_> = candidates
        .iter()
        .filter(|path| {
            path.parent()
                .map(|parent| parent == paper_root)
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    let pool = if top_level.is_empty() {
        candidates.to_vec()
    } else {
        top_level
    };

    for name in MAIN_LIKE_NAMES {
        for path in &pool {
            if path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .is_some_and(|file_name| file_name.eq_ignore_ascii_case(name))
            {
                return Some(path.clone());
            }
        }
    }

    let (included_by, outgoing) = build_include_graph_for_candidates(paper_dir, &pool);
    let independent: Vec<_> = pool
        .iter()
        .filter(|path| {
            included_by
                .get(*path)
                .is_none_or(|parents| parents.is_empty())
        })
        .cloned()
        .collect();
    let pool = if !independent.is_empty() {
        independent
    } else {
        pool
    };

    pool.into_iter().max_by_key(|path| {
        (
            outgoing.get(path).map(|set| set.len()).unwrap_or(0),
            path.clone(),
        )
    })
}

fn build_include_graph_for_candidates(
    paper_dir: &Path,
    candidates: &[PathBuf],
) -> (IncludeMap, IncludeMap) {
    let all_tex = candidates.iter().cloned().collect::<BTreeSet<_>>();
    let (edges, _) = build_include_graph(paper_dir, &all_tex);
    let mut included_by = BTreeMap::new();
    let mut outgoing = BTreeMap::new();
    for candidate in candidates {
        included_by.insert(candidate.clone(), BTreeSet::new());
        outgoing.insert(candidate.clone(), BTreeSet::new());
    }
    for (from, to) in edges {
        if outgoing.contains_key(&from) && all_tex.contains(&to) {
            outgoing.entry(from.clone()).or_default().insert(to.clone());
            included_by.entry(to).or_default().insert(from);
        }
    }
    (included_by, outgoing)
}

fn root_from_primary_roots(
    paper_dir: &Path,
    all_tex: &BTreeSet<PathBuf>,
) -> (Option<PathBuf>, Vec<PathBuf>) {
    let paper_root = paper_dir.canonicalize().ok();
    let mut document_roots = Vec::new();

    for path in all_tex {
        let content = active_tex_content(path);
        if declares_document(&content) {
            document_roots.push(path.clone());
        }
    }

    if document_roots.is_empty() {
        return (None, Vec::new());
    }

    let roots_with_bbl: Vec<_> = document_roots
        .iter()
        .filter(|path| {
            let mut bbl = (*path).clone();
            bbl.set_extension("bbl");
            bbl.is_file()
        })
        .cloned()
        .collect();
    if !roots_with_bbl.is_empty() {
        return (
            choose_root_candidate(&roots_with_bbl, paper_dir),
            roots_with_bbl,
        );
    }

    if let Some(paper_root) = &paper_root {
        let main_like: Vec<_> = document_roots
            .iter()
            .filter(|path| {
                path.parent() == Some(paper_root.as_path())
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(is_main_like_name)
            })
            .cloned()
            .collect();
        if !main_like.is_empty() {
            return (choose_root_candidate(&main_like, paper_dir), main_like);
        }

        let top_level: Vec<_> = document_roots
            .iter()
            .filter(|path| path.parent() == Some(paper_root.as_path()))
            .cloned()
            .collect();
        if !top_level.is_empty() {
            return (choose_root_candidate(&top_level, paper_dir), top_level);
        }
    }

    (
        choose_root_candidate(&document_roots, paper_dir),
        document_roots,
    )
}

fn resolve_root(
    paper_dir: &Path,
    all_tex: &BTreeSet<PathBuf>,
    included_by: &BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    include_edges: &[(PathBuf, PathBuf)],
) -> (Option<PathBuf>, RootMethod, Vec<PathBuf>) {
    if let Some(root) = root_from_readme(paper_dir) {
        return (Some(root), RootMethod::Readme, Vec::new());
    }

    if let Some(root) = root_from_magic_comment(paper_dir, all_tex) {
        return (Some(root), RootMethod::MagicComment, Vec::new());
    }

    let (root, candidates) = root_from_primary_roots(paper_dir, all_tex);
    if root.is_some() {
        let ambiguous = candidates.len() > 1;
        return (
            root,
            RootMethod::PrimaryRoot,
            if ambiguous { candidates } else { Vec::new() },
        );
    }

    let paper_root = paper_dir.canonicalize().ok();
    if let Some(paper_root) = paper_root {
        for name in ["main.tex", "paper.tex", "ms.tex"] {
            let candidate = paper_root.join(name);
            if candidate.is_file() {
                if let Ok(canonical) = candidate.canonicalize() {
                    return (Some(canonical), RootMethod::Fallback, Vec::new());
                }
            }
        }

        let top_level: Vec<_> = all_tex
            .iter()
            .filter(|path| path.parent() == Some(paper_root.as_path()))
            .cloned()
            .collect();
        if top_level.len() == 1 {
            return (Some(top_level[0].clone()), RootMethod::Fallback, Vec::new());
        }

        if all_tex.len() == 1 {
            return (
                Some(all_tex.iter().next().unwrap().clone()),
                RootMethod::Fallback,
                Vec::new(),
            );
        }
    }

    let _ = (included_by, include_edges);
    (None, RootMethod::Unresolved, Vec::new())
}

fn build_include_graph(
    paper_dir: &Path,
    all_tex: &BTreeSet<PathBuf>,
) -> (IncludeEdges, IncludeMap) {
    let mut edges = Vec::new();
    let mut included_by: IncludeMap = BTreeMap::new();

    for path in all_tex {
        included_by.entry(path.clone()).or_default();
        let content = active_tex_content(path);
        for include in find_include_paths(&content) {
            if let Some(child) = resolve_include_path(paper_dir, path, &include.raw_path) {
                if all_tex.contains(&child) {
                    edges.push((path.clone(), child.clone()));
                    included_by.entry(child).or_default().insert(path.clone());
                }
            }
        }
    }

    (edges, included_by)
}

fn collect_reachable(root: &Path, edges: &[(PathBuf, PathBuf)]) -> BTreeSet<PathBuf> {
    let mut adjacency: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for (from, to) in edges {
        adjacency.entry(from.clone()).or_default().push(to.clone());
    }

    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();
    if let Ok(root) = root.canonicalize() {
        queue.push_back(root.clone());
        reachable.insert(root);
    }

    while let Some(current) = queue.pop_front() {
        if let Some(children) = adjacency.get(&current) {
            for child in children {
                if reachable.insert(child.clone()) {
                    queue.push_back(child.clone());
                }
            }
        }
    }

    reachable
}

#[derive(Debug, Clone)]
struct IncludeMatch {
    raw_path: String,
    line: usize,
    column: usize,
}

fn collect_missing_includes(paper_dir: &Path, all_tex: &BTreeSet<PathBuf>) -> Vec<MissingInclude> {
    let paper_root = paper_dir.canonicalize().ok();
    let mut missing = Vec::new();

    for path in all_tex {
        let content = active_tex_content(path);
        for include in find_include_paths(&content) {
            let resolved = resolve_include_path(paper_dir, path, &include.raw_path);
            let missing_target = match resolved {
                None => true,
                Some(resolved) => paper_root
                    .as_ref()
                    .is_some_and(|root| resolved.is_file() && !resolved.starts_with(root)),
            };
            if missing_target {
                missing.push(MissingInclude {
                    file: path.clone(),
                    line: include.line,
                    column: include.column,
                    raw_path: include.raw_path,
                });
            }
        }
    }

    missing
}

fn find_include_paths(content: &str) -> Vec<IncludeMatch> {
    let mut matches = Vec::new();
    let mut offset = 0;

    while let Some(relative) = content[offset..].find('\\') {
        let start = offset + relative;
        if is_commented_position(content, start) {
            offset = start + 1;
            continue;
        }

        let Some((name, after_name)) = read_command_name(content, start) else {
            offset = start + 1;
            continue;
        };

        if !matches!(name, "input" | "include" | "subfile") {
            offset = after_name;
            continue;
        }

        let Some((body_start, body_end)) = read_include_argument(content, after_name) else {
            offset = after_name;
            continue;
        };

        let raw_path = content[body_start..body_end].trim().to_string();
        if !raw_path.is_empty() {
            matches.push(IncludeMatch {
                raw_path,
                line: line_number(content, start),
                column: column_number(content, start),
            });
        }

        offset = body_end + 1;
    }

    matches
}

fn read_command_name(content: &str, slash_index: usize) -> Option<(&str, usize)> {
    let command_start = slash_index + 1;
    let mut command_end = command_start;

    for (index, character) in content[command_start..].char_indices() {
        if character.is_ascii_alphabetic() {
            command_end = command_start + index + character.len_utf8();
        } else {
            break;
        }
    }

    if command_end == command_start {
        None
    } else {
        Some((&content[command_start..command_end], command_end))
    }
}

fn read_required_argument(content: &str, mut offset: usize) -> Option<(usize, usize)> {
    offset = skip_ascii_whitespace(content, offset);
    if !content.get(offset..)?.starts_with('{') {
        return None;
    }

    let end = balanced_group_end(content, offset, '{', '}')?;
    Some((offset + 1, end))
}

fn read_include_argument(content: &str, offset: usize) -> Option<(usize, usize)> {
    if let Some(argument) = read_required_argument(content, offset) {
        return Some(argument);
    }

    let start = skip_ascii_whitespace(content, offset);
    let mut end = start;
    while content.as_bytes().get(end).is_some_and(|byte| {
        !byte.is_ascii_whitespace() && !matches!(byte, b'%' | b'{' | b'}' | b'\\')
    }) {
        end += 1;
    }

    (end > start).then_some((start, end))
}

fn skip_ascii_whitespace(content: &str, mut offset: usize) -> usize {
    while content
        .as_bytes()
        .get(offset)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        offset += 1;
    }
    offset
}

fn balanced_group_end(content: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut escaped = false;

    for (relative, character) in content[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + relative);
            }
        }
    }

    None
}

fn is_commented_position(content: &str, byte_index: usize) -> bool {
    let line_start = content[..byte_index]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let mut escaped = false;

    for character in content[line_start..byte_index].chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '%' {
            return true;
        }
    }

    false
}

fn line_number(content: &str, byte_index: usize) -> usize {
    content[..byte_index].matches('\n').count() + 1
}

fn column_number(content: &str, byte_index: usize) -> usize {
    let line_start = content[..byte_index]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    content[line_start..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::ProjectGraph;

    fn temp_project(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paper-linter-graph-{name}-{nonce}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn detects_missing_include() {
        let dir = temp_project("missing-include");
        let main = dir.join("paper.tex");
        write(
            &main,
            "\\documentclass{article}\n\\input{sections/missing}\n",
        );

        let graph = ProjectGraph::analyze(&dir).expect("analyze");
        assert_eq!(graph.missing_includes.len(), 1);
        assert_eq!(graph.missing_includes[0].raw_path, "sections/missing");
    }

    #[test]
    fn finds_orphan_tex_files() {
        let dir = temp_project("orphan");
        let main = dir.join("paper.tex");
        let orphan = dir.join("old-draft.tex");
        write(
            &main,
            "\\documentclass{article}\n\\begin{document}\nHi\n\\end{document}\n",
        );
        write(&orphan, "\\section{Draft}\n");

        let graph = ProjectGraph::analyze(&dir).expect("analyze");
        assert!(graph.reachable.contains(&main.canonicalize().unwrap()));
        assert!(!graph.reachable.contains(&orphan.canonicalize().unwrap()));
    }

    #[test]
    fn follows_bare_and_root_relative_inputs() {
        let dir = temp_project("bare-root-relative");
        let main = dir.join("main.tex");
        let chapter = dir.join("chapters/one.tex");
        let method = dir.join("sections/method.tex");
        write(&main, "\\documentclass{article}\n\\input chapters/one\n");
        write(&chapter, "\\input sections/method\n");
        write(&method, "Body\n");

        let graph = ProjectGraph::analyze(&dir).expect("analyze");

        assert!(graph.missing_includes.is_empty());
        assert!(graph.reachable.contains(&main.canonicalize().unwrap()));
        assert!(graph.reachable.contains(&chapter.canonicalize().unwrap()));
        assert!(graph.reachable.contains(&method.canonicalize().unwrap()));
    }

    #[test]
    fn ignores_explicit_non_tex_inputs_for_missing_include() {
        let dir = temp_project("non-tex-input");
        let main = dir.join("paper.tex");
        write(&main, "\\documentclass{article}\n\\input{plot.pgf}\n");

        let graph = ProjectGraph::analyze(&dir).expect("analyze");

        assert!(graph.missing_includes.is_empty());
    }

    #[test]
    fn resolves_main_like_root() {
        let dir = temp_project("main-like");
        let main = dir.join("main.tex");
        write(
            &main,
            "\\documentclass{article}\n\\begin{document}\nHi\n\\end{document}\n",
        );

        let graph = ProjectGraph::analyze(&dir).expect("analyze");
        assert_eq!(graph.root, Some(main.canonicalize().unwrap()));
    }

    #[test]
    fn resolves_tex_root_magic_comment() {
        let dir = temp_project("magic-root");
        let main = dir.join("main.tex");
        let section = dir.join("sections/method.tex");
        write(
            &main,
            "\\documentclass{article}\n\\input{sections/method}\n",
        );
        write(&section, "%! TeX root = ../main.tex\n\\section{Method}\n");

        let graph = ProjectGraph::analyze(&dir).expect("analyze");

        assert_eq!(graph.root, Some(main.canonicalize().unwrap()));
        assert_eq!(graph.root_method, super::RootMethod::MagicComment);
        assert!(graph.reachable.contains(&section.canonicalize().unwrap()));
    }

    #[test]
    fn ignores_inactive_include_edges() {
        let dir = temp_project("inactive-include");
        let main = dir.join("main.tex");
        let live = dir.join("live.tex");
        let dead = dir.join("dead.tex");
        write(
            &main,
            "\\documentclass{article}\n\\begin{document}\n\\input{live}\n\\begin{comment}\n\\input{dead}\n\\end{comment}\n\\iffalse\n\\input{dead}\n\\fi\n\\end{document}\n",
        );
        write(&live, "Live\n");
        write(&dead, "Dead\n");

        let graph = ProjectGraph::analyze(&dir).expect("analyze");

        assert!(graph.reachable.contains(&main.canonicalize().unwrap()));
        assert!(graph.reachable.contains(&live.canonicalize().unwrap()));
        assert!(!graph.reachable.contains(&dead.canonicalize().unwrap()));
    }
}
