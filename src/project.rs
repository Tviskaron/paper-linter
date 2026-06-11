use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::latex::scan::{scan_latex, FloatEnv, Graphic, GraphicsPath, Include, Label, Ref};

const GRAPHICS_EXTENSIONS: [&str; 6] = ["pdf", "png", "jpg", "jpeg", "eps", "svg"];

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ProjectIndex {
    pub root: PathBuf,
    pub files: Vec<SourceFile>,
    pub labels: Vec<Label>,
    pub refs: Vec<Ref>,
    pub graphics: Vec<Graphic>,
    pub graphics_paths: Vec<GraphicsPath>,
    pub floats: Vec<FloatEnv>,
}

impl ProjectIndex {
    pub fn build(input_paths: &[PathBuf], discovered_files: &[PathBuf]) -> io::Result<Self> {
        let root = infer_project_root(input_paths, discovered_files)?;
        let mut builder = ProjectBuilder {
            root,
            seen: BTreeSet::new(),
            files: Vec::new(),
            labels: Vec::new(),
            refs: Vec::new(),
            graphics: Vec::new(),
            graphics_paths: Vec::new(),
            floats: Vec::new(),
        };

        for file in discovered_files {
            builder.add_file(file)?;
        }

        Ok(builder.finish())
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

struct ProjectBuilder {
    root: PathBuf,
    seen: BTreeSet<PathBuf>,
    files: Vec<SourceFile>,
    labels: Vec<Label>,
    refs: Vec<Ref>,
    graphics: Vec<Graphic>,
    graphics_paths: Vec<GraphicsPath>,
    floats: Vec<FloatEnv>,
}

impl ProjectBuilder {
    fn add_file(&mut self, path: &Path) -> io::Result<()> {
        let canonical = canonicalize_existing(path)?;
        if !canonical.starts_with(&self.root) || !self.seen.insert(canonical.clone()) {
            return Ok(());
        }

        let content = fs::read_to_string(&canonical)?;
        let scan = scan_latex(canonical.clone(), &content);
        let includes = scan.includes.clone();

        self.labels.extend(scan.labels);
        self.refs.extend(scan.refs);
        self.graphics.extend(scan.graphics);
        self.graphics_paths.extend(scan.graphics_paths);
        self.floats.extend(scan.floats);
        self.files.push(SourceFile {
            path: canonical.clone(),
            content,
        });

        for include in includes {
            if let Some(path) = resolve_include_path(&self.root, &canonical, &include) {
                self.add_file(&path)?;
            }
        }

        Ok(())
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
        self.floats
            .sort_by(|left, right| left.location.file.cmp(&right.location.file));

        ProjectIndex {
            root: self.root,
            files: self.files,
            labels: self.labels,
            refs: self.refs,
            graphics: self.graphics,
            graphics_paths: self.graphics_paths,
            floats: self.floats,
        }
    }
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
    let base = current_file.parent()?;
    let raw = Path::new(&include.raw_path);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        base.join(raw)
    };

    let candidates = if candidate.extension().is_some() {
        if candidate
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("tex")
        {
            return None;
        }
        vec![candidate]
    } else {
        vec![candidate.clone(), candidate.with_extension("tex")]
    };

    candidates.into_iter().find_map(|candidate| {
        let canonical = candidate.canonicalize().ok()?;
        canonical.starts_with(root).then_some(canonical)
    })
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
    if candidate.exists() {
        return None;
    }

    let parent = candidate.parent()?;
    let target_name = candidate.file_name()?.to_str()?;

    for entry in fs::read_dir(parent).ok()? {
        let entry = entry.ok()?;
        let file_name = entry.file_name();
        let file_name = file_name.to_str()?;

        if file_name != target_name && file_name.eq_ignore_ascii_case(target_name) {
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

    fn canonical_option(path: Option<PathBuf>) -> Option<PathBuf> {
        path.map(|path| canonical(&path))
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
    fn ignores_non_tex_input_files() {
        let dir = temp_project("non-tex-input");
        let main = dir.join("paper.tex");
        let plot = dir.join("figures/plot.pgf");
        write(&main, "\\input{figures/plot.pgf}\n\\label{sec:main}\n");
        write(&plot, "\\label{fig:plot-data}\n");

        let index = ProjectIndex::build(std::slice::from_ref(&main), std::slice::from_ref(&main))
            .expect("project should index");

        assert_eq!(index.files.len(), 1);
        assert!(index.labels.iter().any(|label| label.key == "sec:main"));
        assert!(!index
            .labels
            .iter()
            .any(|label| label.key == "fig:plot-data"));
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
            canonical_option(index.resolve_graphic(&index.graphics[0])),
            Some(canonical(&asset))
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
            canonical_option(index.resolve_graphic(&index.graphics[0])),
            Some(canonical(&asset))
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
            canonical_option(index.resolve_graphic(&index.graphics[0])),
            Some(canonical(&asset))
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
        let resolved = canonical_option(index.resolve_graphic(&index.graphics[0]));
        let mismatch = canonical_option(index.find_graphic_case_mismatch(&index.graphics[0]));
        assert_eq!(resolved.or(mismatch), Some(canonical(&asset)));
    }
}
