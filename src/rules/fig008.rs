use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::fig001::is_static_graphic_path;
use crate::rules::ProjectRule;

const MAX_HEADER_BYTES: u64 = 64 * 1024;
const MIN_BYTES_FOR_HEADER_CHECK: u64 = 24;

pub struct CorruptImage;

impl ProjectRule for CorruptImage {
    fn code(&self) -> &'static str {
        "FIG008"
    }

    fn name(&self) -> &'static str {
        "corrupt image"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for graphic in &project.graphics {
            if !is_static_graphic_path(&graphic.raw_path) {
                continue;
            }

            let Some(asset_path) = project.resolve_graphic(graphic) else {
                continue;
            };

            let reason = if let Some(header) = project.asset_header(&asset_path) {
                validate_image_header(&asset_path, header)
            } else {
                validate_image_file(&asset_path)
            };
            if let Some(reason) = reason {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Error,
                        format!(
                            "image '{}' appears corrupt or unreadable: {reason}",
                            graphic.raw_path
                        ),
                        &graphic.location.file,
                        graphic.location.line,
                        graphic.location.column,
                    )
                    .with_hint("replace the asset with a valid PNG, JPEG, or PDF file"),
                );
            }
        }

        diagnostics
    }
}

fn validate_image_file(path: &Path) -> Option<&'static str> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() < MIN_BYTES_FOR_HEADER_CHECK {
        return None;
    }

    let header = read_header(path).ok()?;
    validate_image_header(path, &header)
}

fn validate_image_header(path: &Path, header: &[u8]) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if header.is_empty() {
        return None;
    }
    match extension.as_str() {
        "png" => {
            if !is_valid_png(header) {
                Some("invalid PNG header")
            } else {
                None
            }
        }
        "jpg" | "jpeg" => {
            if !is_valid_jpeg(header) {
                Some("invalid JPEG header")
            } else {
                None
            }
        }
        "pdf" => {
            if !header.starts_with(b"%PDF-") {
                Some("invalid PDF header")
            } else {
                None
            }
        }
        _ => None,
    }
}

fn read_header(path: &Path) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut reader = file.take(MAX_HEADER_BYTES);
    let mut header = Vec::new();
    reader.read_to_end(&mut header)?;
    Ok(header)
}

fn is_valid_png(bytes: &[u8]) -> bool {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    bytes.len() >= 24
        && &bytes[..8] == PNG_SIGNATURE
        && &bytes[12..16] == b"IHDR"
        && u32::from_be_bytes(bytes[16..20].try_into().unwrap_or([0; 4])) > 0
        && u32::from_be_bytes(bytes[20..24].try_into().unwrap_or([0; 4])) > 0
}

fn is_valid_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0xff && bytes[1] == 0xd8
}

#[cfg(test)]
mod tests {
    use super::{is_valid_jpeg, is_valid_png};

    #[test]
    fn validates_png_signature() {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&100u32.to_be_bytes());
        bytes.extend_from_slice(&100u32.to_be_bytes());
        assert!(is_valid_png(&bytes));
        assert!(!is_valid_png(b"not-a-png"));
    }

    #[test]
    fn validates_jpeg_signature() {
        assert!(is_valid_jpeg(&[0xff, 0xd8, 0xff, 0xdb]));
        assert!(!is_valid_jpeg(b"abcd"));
    }
}
