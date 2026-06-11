use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::fig001::is_static_graphic_path;
use crate::rules::ProjectRule;

const SUPPORTED_IMAGE_EXTENSIONS: [&str; 6] = ["pdf", "png", "jpg", "jpeg", "eps", "svg"];

pub struct ImageFormatPolicy;

impl ProjectRule for ImageFormatPolicy {
    fn code(&self) -> &'static str {
        "FIG006"
    }

    fn name(&self) -> &'static str {
        "image-format"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .graphics
            .iter()
            .filter(|graphic| is_static_graphic_path(&graphic.raw_path))
            .filter_map(|graphic| {
                unsupported_extension(&graphic.raw_path).map(|extension| (graphic, extension))
            })
            .map(|(graphic, extension)| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!("image format '.{}' is not in the supported set", extension),
                    &graphic.location.file,
                    graphic.location.line,
                    graphic.location.column,
                )
                .with_hint("use pdf, png, jpg, jpeg, eps, or svg")
            })
            .collect()
    }
}

fn unsupported_extension(path: &str) -> Option<String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();
    (!SUPPORTED_IMAGE_EXTENSIONS.contains(&extension.as_str())).then_some(extension)
}

#[cfg(test)]
mod tests {
    use super::unsupported_extension;

    #[test]
    fn detects_unsupported_explicit_extensions() {
        assert_eq!(
            unsupported_extension("figures/model.bmp"),
            Some("bmp".to_string())
        );
        assert_eq!(unsupported_extension("figures/model.PDF"), None);
        assert_eq!(unsupported_extension("figures/model"), None);
    }
}
