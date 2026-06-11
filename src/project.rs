use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::latex::scan::{
    scan_latex, BibliographyDecl, DocumentClass, FloatEnv, Graphic, GraphicsPath, Include, Label,
    PackageImport, Ref,
};
use crate::latex::significant::{mask_discarded_macro_arguments, mask_inactive_regions};

const GRAPHICS_EXTENSIONS: [&str; 6] = ["pdf", "png", "jpg", "jpeg", "eps", "svg"];
const PROJECT_INDEX_VERSION: u32 = 2;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectIndex {
    pub root: PathBuf,
    pub files: Vec<SourceFile>,
    pub labels: Vec<Label>,
    pub refs: Vec<Ref>,
    pub graphics: Vec<Graphic>,
    pub graphics_paths: Vec<GraphicsPath>,
    pub bibliographies: Vec<BibliographyDecl>,
    pub document_classes: Vec<DocumentClass>,
    pub packages: Vec<PackageImport>,
    pub floats: Vec<FloatEnv>,
}

impl ProjectIndex {
    pub fn build(input_paths: &[PathBuf], discovered_files: &[PathBuf]) -> io::Result<Self> {
        let root = infer_project_root(input_paths, discovered_files)?;
        let mut builder = ProjectBuilder {
            root,
            seen: BTreeSet::new(),
            document_ended: BTreeMap::new(),
            files: Vec::new(),
            labels: Vec::new(),
            refs: Vec::new(),
            graphics: Vec::new(),
            graphics_paths: Vec::new(),
            bibliographies: Vec::new(),
            document_classes: Vec::new(),
            packages: Vec::new(),
            floats: Vec::new(),
        };

        for file in discovered_files {
            builder.add_file(file)?;
        }

        Ok(builder.finish())
    }

    pub fn read(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let file: ProjectIndexFile = serde_json::from_str(&content).map_err(json_io_error)?;
        if file.version != PROJECT_INDEX_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported project index version {}; expected {}",
                    file.version, PROJECT_INDEX_VERSION
                ),
            ));
        }
        Ok(file.project)
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        let content = serde_json::to_string_pretty(&ProjectIndexFile {
            version: PROJECT_INDEX_VERSION,
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            project: self.clone(),
        })
        .map_err(json_io_error)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("{content}\n"))
    }

    pub fn is_referenced(&self, key: &str) -> bool {
        self.refs.iter().any(|reference| reference.key == key)
    }

    pub fn has_label(&self, key: &str) -> bool {
        self.labels.iter().any(|label| label.key == key)
    }

    pub fn resolve_graphic(&self, graphic: &Graphic) -> Option<PathBuf> {
        resolve_graphic_path(
            &self.root,
            &graphic.location.file,
            &graphic.raw_path,
            &self.graphics_paths,
        )
    }

    pub fn find_graphic_case_mismatch(&self, graphic: &Graphic) -> Option<PathBuf> {
        find_graphic_case_mismatch(
            &self.root,
            &graphic.location.file,
            &graphic.raw_path,
            &self.graphics_paths,
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ProjectIndexFile {
    version: u32,
    package_version: String,
    project: ProjectIndex,
}

fn json_io_error(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

struct ProjectBuilder {
    root: PathBuf,
    seen: BTreeSet<PathBuf>,
    document_ended: BTreeMap<PathBuf, bool>,
    files: Vec<SourceFile>,
    labels: Vec<Label>,
    refs: Vec<Ref>,
    graphics: Vec<Graphic>,
    graphics_paths: Vec<GraphicsPath>,
    bibliographies: Vec<BibliographyDecl>,
    document_classes: Vec<DocumentClass>,
    packages: Vec<PackageImport>,
    floats: Vec<FloatEnv>,
}

impl ProjectBuilder {
    fn add_file(&mut self, path: &Path) -> io::Result<bool> {
        let canonical = canonicalize_existing(path)?;
        if !canonical.starts_with(&self.root) {
            return Ok(false);
        }
        if !self.seen.insert(canonical.clone()) {
            return Ok(self
                .document_ended
                .get(&canonical)
                .copied()
                .unwrap_or(false));
        }

        let content = fs::read_to_string(&canonical)?;
        let content = mask_discarded_macro_arguments(&content);
        let content = mask_inactive_regions(&content);
        let scan = scan_latex(canonical.clone(), &content);
        let includes = scan.includes.clone();
        let mut truncation_line = scan.document_end.as_ref().map(|location| location.line);

        for include in includes {
            if truncation_line.is_some_and(|line| include.location.line > line) {
                break;
            }

            let Some(path) = resolve_include_path(&self.root, &canonical, &include) else {
                continue;
            };

            if self.add_file(&path)? {
                truncation_line = Some(
                    truncation_line
                        .map(|line| line.min(include.location.line))
                        .unwrap_or(include.location.line),
                );
                break;
            }
        }

        let content = truncation_line
            .map(|line| truncate_after_line(&content, line))
            .unwrap_or(content);
        let scan = scan_latex(canonical.clone(), &content);

        self.labels.extend(scan.labels);
        self.refs.extend(scan.refs);
        self.graphics.extend(scan.graphics);
        self.graphics_paths.extend(scan.graphics_paths);
        self.bibliographies.extend(scan.bibliographies);
        self.document_classes.extend(scan.document_classes);
        self.packages.extend(scan.packages);
        self.floats.extend(scan.floats);
        self.files.push(SourceFile {
            path: canonical.clone(),
            content,
        });

        let ended = truncation_line.is_some();
        self.document_ended.insert(canonical, ended);

        Ok(ended)
    }

    fn finish(mut self) -> ProjectIndex {
        self.files.sort_by(|left, right| left.path.cmp(&right.path));
        self.labels
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));
        self.refs
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));
        self.graphics
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));
        self.graphics_paths
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));
        self.bibliographies
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));
        self.document_classes
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));
        self.packages
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));
        self.floats
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));

        ProjectIndex {
            root: self.root,
            files: self.files,
            labels: self.labels,
            refs: self.refs,
            graphics: self.graphics,
            graphics_paths: self.graphics_paths,
            bibliographies: self.bibliographies,
            document_classes: self.document_classes,
            packages: self.packages,
            floats: self.floats,
        }
    }
}

fn truncate_after_line(content: &str, line_number: usize) -> String {
    for (line_index, (index, _)) in content.match_indices('\n').enumerate() {
        if line_index + 1 == line_number {
            return content[..index + 1].to_string();
        }
    }

    content.to_string()
}

fn infer_project_root(
    input_paths: &[PathBuf],
    discovered_files: &[PathBuf],
) -> io::Result<PathBuf> {
    let mut roots = Vec::new();

    for path in input_paths {
        if path.is_dir() {
            roots.push(canonicalize_existing(path)?);
        } else if path.is_file() {
            if let Some(parent) = path.parent() {
                roots.push(canonicalize_existing(parent)?);
            }
        }
    }

    for file in discovered_files {
        if let Some(parent) = file.parent() {
            roots.push(canonicalize_existing(parent)?);
        }
    }

    roots
        .into_iter()
        .reduce(common_ancestor)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no TeX files found"))
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

fn resolve_include_path(root: &Path, current_file: &Path, include: &Include) -> Option<PathBuf> {
    if !is_tex_like_include(&include.raw_path) {
        return None;
    }

    let base = current_file.parent()?;
    let raw = Path::new(&include.raw_path);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        base.join(raw)
    };

    let mut candidates = if candidate.extension().is_some() {
        vec![candidate]
    } else {
        vec![candidate.clone(), candidate.with_extension("tex")]
    };
    if !raw.is_absolute() {
        let root_candidate = root.join(raw);
        if root_candidate.extension().is_some() {
            candidates.push(root_candidate);
        } else {
            candidates.push(root_candidate.clone());
            candidates.push(root_candidate.with_extension("tex"));
        }
    }

    candidates.into_iter().find_map(|candidate| {
        let canonical = candidate.canonicalize().ok()?;
        (canonical.is_file() && canonical.starts_with(root)).then_some(canonical)
    })
}

fn is_tex_like_include(raw_path: &str) -> bool {
    Path::new(raw_path.trim())
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| extension.eq_ignore_ascii_case("tex"))
}

fn resolve_graphic_path(
    root: &Path,
    current_file: &Path,
    raw_path: &str,
    graphics_paths: &[GraphicsPath],
) -> Option<PathBuf> {
    graphic_candidate_paths(root, current_file, raw_path, graphics_paths)?
        .into_iter()
        .find_map(|candidate| {
            if find_case_mismatch(root, &candidate).is_some() {
                return None;
            }
            let canonical = candidate.canonicalize().ok()?;
            canonical.starts_with(root).then_some(canonical)
        })
}

fn find_graphic_case_mismatch(
    root: &Path,
    current_file: &Path,
    raw_path: &str,
    graphics_paths: &[GraphicsPath],
) -> Option<PathBuf> {
    graphic_candidate_paths(root, current_file, raw_path, graphics_paths)?
        .into_iter()
        .find_map(|candidate| find_case_mismatch(root, &candidate))
}

fn graphic_candidate_paths(
    root: &Path,
    current_file: &Path,
    raw_path: &str,
    graphics_paths: &[GraphicsPath],
) -> Option<Vec<PathBuf>> {
    let base = current_file.parent()?;
    let raw = Path::new(raw_path);
    Some(if raw.is_absolute() {
        graphic_candidates(raw.to_path_buf())
    } else {
        let mut candidates = graphic_candidates(base.join(raw));
        candidates.extend(graphic_candidates(root.join(raw)));
        for graphics_path in graphics_paths {
            candidates.extend(graphic_candidates(
                resolve_graphics_path_base(
                    root,
                    &graphics_path.location.file,
                    &graphics_path.raw_path,
                )?
                .join(raw),
            ));
        }
        candidates
    })
}

fn find_case_mismatch(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let parent = candidate.parent()?;
    let target_name = candidate.file_name()?.to_str()?;

    for entry in fs::read_dir(parent).ok()? {
        let entry = entry.ok()?;
        let file_name = entry.file_name();
        let file_name = file_name.to_str()?;

        if file_name == target_name {
            return None;
        }

        if file_name.eq_ignore_ascii_case(target_name) {
            let canonical = entry.path().canonicalize().ok()?;
            if canonical.starts_with(root) {
                return Some(canonical);
            }
        }
    }

    None
}

fn resolve_graphics_path_base(
    root: &Path,
    declaring_file: &Path,
    raw_path: &str,
) -> Option<PathBuf> {
    let raw = Path::new(raw_path);
    if raw.is_absolute() {
        return Some(raw.to_path_buf());
    }

    let declaring_dir = declaring_file.parent()?;
    let local = declaring_dir.join(raw);
    if local.exists() {
        return Some(local);
    }

    Some(root.join(raw))
}

fn graphic_candidates(candidate: PathBuf) -> Vec<PathBuf> {
    let mut candidates = vec![candidate.clone()];
    if candidate.extension().is_none() {
        candidates.extend(
            GRAPHICS_EXTENSIONS
                .iter()
                .map(|extension| candidate.with_extension(extension)),
        );
    }
    candidates
}

fn canonicalize_existing(path: &Path) -> io::Result<PathBuf> {
    path.canonicalize()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::ProjectIndex;

    fn temp_project(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paper-linter-project-{name}-{nonce}"));
        fs::create_dir_all(&dir).expect("failed to create temp project");
        dir
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent");
        }
        fs::write(path, content).expect("failed to write fixture");
    }

    fn canonical(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    #[test]
    fn follows_input_and_include_within_root() {
        let dir = temp_project("includes");
        let main = dir.join("paper.tex");
        let method = dir.join("sections/method.tex");
        let results = dir.join("sections/results.tex");
        write(
            &main,
            "\\input{sections/method}\n\\include{sections/results}\n",
        );
        write(&method, "\\label{sec:method}\n");
        write(&results, "\\ref{sec:method}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.files.len(), 3);
        assert!(index.labels.iter().any(|label| label.key == "sec:method"));
        assert!(index.is_referenced("sec:method"));
        assert!(index.has_label("sec:method"));
        assert!(!index.has_label("sec:missing"));
    }

    #[test]
    fn ignores_explicit_non_tex_inputs() {
        let dir = temp_project("pgf-input");
        let main = dir.join("paper.tex");
        let plot = dir.join("plot.pgf");
        write(&main, "\\input{plot.pgf}\n");
        write(&plot, "\\label{fig:plot}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.files.len(), 1);
        assert!(index.labels.is_empty());
    }

    #[test]
    fn ignores_inputs_that_resolve_to_directories() {
        let dir = temp_project("directory-input");
        let main = dir.join("paper.tex");
        fs::create_dir_all(dir.join("sections")).expect("failed to create input directory");
        write(&main, "\\input sections\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.files.len(), 1);
    }

    #[test]
    fn follows_bare_input_paths() {
        let dir = temp_project("bare-input");
        let main = dir.join("paper.tex");
        let method = dir.join("sections/method.tex");
        write(&main, "\\input sections/method\n");
        write(&method, "\\label{sec:method}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.files.len(), 2);
        assert!(index.labels.iter().any(|label| label.key == "sec:method"));
    }

    #[test]
    fn follows_root_relative_inputs_from_nested_files() {
        let dir = temp_project("root-relative-input");
        let main = dir.join("paper.tex");
        let supp = dir.join("src/supp/supp.tex");
        let method = dir.join("src/supp/method.tex");
        write(&main, "\\input src/supp/supp\n");
        write(&supp, "\\input src/supp/method\n");
        write(&method, "\\label{sec:method}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.files.len(), 3);
        assert!(index.labels.iter().any(|label| label.key == "sec:method"));
    }

    #[test]
    fn truncates_parent_after_input_that_ends_document() {
        let dir = temp_project("input-ends-document");
        let main = dir.join("paper.tex");
        let supplement = dir.join("supplement.tex");
        write(
            &main,
            "\\label{active}\n\\input{supplement}\n\\label{dead}\nLorem ipsum\n",
        );
        write(&supplement, "\\label{supplement}\n\\end{document}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");
        let main_file = index
            .files
            .iter()
            .find(|file| file.path == canonical(&main))
            .expect("main file should be indexed");

        assert!(index.has_label("active"));
        assert!(index.has_label("supplement"));
        assert!(!index.has_label("dead"));
        assert!(!main_file.content.contains("Lorem ipsum"));
    }

    #[test]
    fn masks_discarded_macro_arguments_before_indexing() {
        let dir = temp_project("discarded-macro");
        let main = dir.join("paper.tex");
        write(
            &main,
            "\\documentclass{article}\n\\long\\def\\todel#1{\\relax}\n\\begin{document}\n\\label{active}\n\\todel{TODO\n\\label{dead}\n\\input{dead}}\n\\end{document}\n",
        );
        write(&dir.join("dead.tex"), "\\label{included-dead}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");
        let main_file = index
            .files
            .iter()
            .find(|file| file.path == canonical(&main))
            .expect("main file should be indexed");

        assert!(index.has_label("active"));
        assert!(!index.has_label("dead"));
        assert!(!index.has_label("included-dead"));
        assert!(!main_file.content.contains("TODO"));
    }

    #[test]
    fn ignores_inline_verb_document_end_when_truncating() {
        let dir = temp_project("verb-document-end");
        let main = dir.join("paper.tex");
        write(
            &main,
            "\\documentclass{article}\n\\begin{document}\n\\begin{comment}\nUse \\verb|\\end{document}| as text.\n\\end{comment}\n\\label{active}\n\\end{document}\n\\label{dead}\n",
        );

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");
        let main_file = index
            .files
            .iter()
            .find(|file| file.path == canonical(&main))
            .expect("main file should be indexed");

        assert!(index.has_label("active"));
        assert!(!index.has_label("dead"));
        assert!(!main_file.content.contains("\\begin{comment}"));
        assert!(!main_file.content.contains("\\verb|\\end{document}|"));
    }

    #[test]
    fn resolves_extensionless_graphic() {
        let dir = temp_project("graphics");
        let main = dir.join("paper.tex");
        let asset = dir.join("figures/model.pdf");
        write(
            &main,
            "\\begin{figure}\\includegraphics{figures/model}\\end{figure}\n",
        );
        write(&asset, "");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.graphics.len(), 1);
        assert_eq!(
            index
                .resolve_graphic(&index.graphics[0])
                .and_then(|path| path.canonicalize().ok()),
            asset.canonicalize().ok()
        );
    }

    #[test]
    fn resolves_graphic_relative_to_project_root_from_included_file() {
        let dir = temp_project("root-relative-graphics");
        let main = dir.join("paper.tex");
        let section = dir.join("sections/method.tex");
        let asset = dir.join("figures/model.pdf");
        write(&main, "\\input{sections/method}\n");
        write(
            &section,
            "\\begin{figure}\\includegraphics{figures/model}\\end{figure}\n",
        );
        write(&asset, "");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.graphics.len(), 1);
        assert_eq!(
            index.resolve_graphic(&index.graphics[0]),
            asset.canonicalize().ok()
        );
    }

    #[test]
    fn resolves_graphic_using_graphicspath() {
        let dir = temp_project("graphicspath");
        let main = dir.join("paper.tex");
        let asset = dir.join("images/model.png");
        write(
            &main,
            "\\graphicspath{{images/}}\n\\begin{figure}\\includegraphics{model}\\end{figure}\n",
        );
        write(&asset, "");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.graphics_paths.len(), 1);
        assert_eq!(index.graphics.len(), 1);
        assert_eq!(
            index.resolve_graphic(&index.graphics[0]),
            asset.canonicalize().ok()
        );
    }

    #[test]
    fn finds_graphic_case_mismatch() {
        let dir = temp_project("case-mismatch-graphics");
        let main = dir.join("paper.tex");
        let asset = dir.join("images/ROC_Validator.pdf");
        write(
            &main,
            "\\begin{figure}\\includegraphics{images/ROC_validator.pdf}\\end{figure}\n",
        );
        write(&asset, "");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.graphics.len(), 1);
        assert_eq!(index.resolve_graphic(&index.graphics[0]), None);
        assert_eq!(
            index.find_graphic_case_mismatch(&index.graphics[0]),
            asset.canonicalize().ok()
        );
    }

    #[test]
    fn indexes_document_classes_and_packages_across_project() {
        let dir = temp_project("package-index");
        let main = dir.join("paper.tex");
        let macros = dir.join("macros.tex");
        write(
            &main,
            "\\documentclass[sigconf]{acmart}\n\\usepackage{graphicx,xcolor}\n\\input{macros}\n",
        );
        write(&macros, "\\RequirePackage{amsmath}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.document_classes.len(), 1);
        assert_eq!(index.document_classes[0].name, "acmart");
        assert_eq!(index.document_classes[0].options, vec!["sigconf"]);
        let mut packages: Vec<_> = index
            .packages
            .iter()
            .map(|package| package.name.as_str())
            .collect();
        packages.sort();
        assert_eq!(packages, vec!["amsmath", "graphicx", "xcolor"]);
    }
}
