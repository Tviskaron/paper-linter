use std::path::{Path, PathBuf};

use super::BibliographyDecl;

pub fn explicit_bib_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| is_bib_file(path))
        .cloned()
        .collect()
}

fn is_bib_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("bib"))
}

pub(super) fn bibliography_paths(
    declarations: &[BibliographyDecl],
    explicit_bib_files: &[PathBuf],
) -> Vec<PathBuf> {
    let mut paths: Vec<_> = declarations.iter().map(resolve_bib_path).collect();
    paths.extend(explicit_bib_files.iter().cloned());
    paths.sort();
    paths.dedup();
    paths
}

pub(super) fn resolve_bib_path(declaration: &BibliographyDecl) -> PathBuf {
    let mut path = PathBuf::from(declaration.path.trim());
    if path.extension().is_none() {
        path.set_extension("bib");
    }

    if path.is_absolute() {
        path
    } else {
        declaration
            .file
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(path)
    }
}
