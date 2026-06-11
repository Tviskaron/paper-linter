use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct MissingAsset;

impl ProjectRule for MissingAsset {
    fn code(&self) -> &'static str {
        "FIG001"
    }

    fn name(&self) -> &'static str {
        "asset-missing"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .graphics
            .iter()
            .filter(|graphic| is_static_graphic_path(&graphic.raw_path))
            .filter(|graphic| project.resolve_graphic(graphic).is_none())
            .filter(|graphic| project.find_graphic_case_mismatch(graphic).is_none())
            .map(|graphic| {
                Diagnostic::new(
                    self.code(),
                    Severity::Error,
                    format!("asset '{}' not found", graphic.raw_path),
                    &graphic.location.file,
                    graphic.location.line,
                    graphic.location.column,
                )
                .with_hint("check the path or add the file")
            })
            .collect()
    }
}

pub(crate) fn is_static_graphic_path(path: &str) -> bool {
    !path.contains('#')
}

#[cfg(test)]
mod tests {
    use super::is_static_graphic_path;

    #[test]
    fn skips_unresolved_macro_parameters() {
        assert!(!is_static_graphic_path("#2"));
        assert!(!is_static_graphic_path("figures/#1_plot.pdf"));
        assert!(is_static_graphic_path("figures/model.pdf"));
    }
}
