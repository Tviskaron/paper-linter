use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::fig001::is_static_graphic_path;
use crate::rules::ProjectRule;

pub struct AssetCaseMismatch;

impl ProjectRule for AssetCaseMismatch {
    fn code(&self) -> &'static str {
        "FIG003"
    }

    fn name(&self) -> &'static str {
        "asset-case-mismatch"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .graphics
            .iter()
            .filter(|graphic| is_static_graphic_path(&graphic.raw_path))
            .filter_map(|graphic| {
                project
                    .find_graphic_case_mismatch(graphic)
                    .map(|matched_path| (graphic, matched_path))
            })
            .map(|(graphic, matched_path)| {
                Diagnostic::new(
                    self.code(),
                    Severity::Error,
                    format!("asset '{}' differs only by filename case", graphic.raw_path),
                    &graphic.location.file,
                    graphic.location.line,
                    graphic.location.column,
                )
                .with_hint(format!(
                    "match the file name case: {}",
                    matched_path.display()
                ))
            })
            .collect()
    }
}
