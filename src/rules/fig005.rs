use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::fig001::is_static_graphic_path;
use crate::rules::ProjectRule;

pub struct UnsafeGraphicPath;

impl ProjectRule for UnsafeGraphicPath {
    fn code(&self) -> &'static str {
        "FIG005"
    }

    fn name(&self) -> &'static str {
        "unsafe-graphic-path"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .graphics
            .iter()
            .filter(|graphic| is_static_graphic_path(&graphic.raw_path))
            .filter(|graphic| is_unsafe_graphic_path(&graphic.raw_path))
            .map(|graphic| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!("graphic path '{}' is not portable", graphic.raw_path),
                    &graphic.location.file,
                    graphic.location.line,
                    graphic.location.column,
                )
                .with_hint("use a project-relative path inside the repository")
            })
            .collect()
    }
}

fn is_unsafe_graphic_path(path: &str) -> bool {
    Path::new(path).is_absolute()
        || path.starts_with('\\')
        || has_windows_drive_prefix(path)
        || has_parent_traversal(path)
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn has_parent_traversal(path: &str) -> bool {
    path.split(['/', '\\']).any(|part| part == "..")
}

#[cfg(test)]
mod tests {
    use super::is_unsafe_graphic_path;

    #[test]
    fn detects_non_portable_graphic_paths() {
        assert!(is_unsafe_graphic_path("/tmp/model.pdf"));
        assert!(is_unsafe_graphic_path("C:\\figures\\model.pdf"));
        assert!(is_unsafe_graphic_path("../figures/model.pdf"));
        assert!(is_unsafe_graphic_path("figures/../model.pdf"));
        assert!(!is_unsafe_graphic_path("figures/model.pdf"));
    }
}
