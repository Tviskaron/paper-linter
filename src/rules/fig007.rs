use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::fig001::is_static_graphic_path;
use crate::rules::ProjectRule;

const MAX_HEADER_BYTES: u64 = 64 * 1024;
const MIN_RASTER_SIDE_PX: u32 = 300;

pub struct ImageHeaderMetadata;

impl ProjectRule for ImageHeaderMetadata {
    fn code(&self) -> &'static str {
        "FIG007"
    }

    fn name(&self) -> &'static str {
        "image-header-metadata"
    }

    fn strict_only(&self) -> bool {
        true
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
            let dimensions = if let Some(header) = project.asset_header(&asset_path) {
                read_image_dimensions_from_header(&asset_path, header)
            } else {
                read_image_dimensions(&asset_path)
            };
            let Some(dimensions) = dimensions else {
                continue;
            };

            if dimensions.width < MIN_RASTER_SIDE_PX || dimensions.height < MIN_RASTER_SIDE_PX {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        format!(
                            "image '{}' is very small ({}x{} px)",
                            graphic.raw_path, dimensions.width, dimensions.height
                        ),
                        &graphic.location.file,
                        graphic.location.line,
                        graphic.location.column,
                    )
                    .with_hint("use a higher-resolution source image or a vector format"),
                );
            }
        }

        diagnostics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImageDimensions {
    width: u32,
    height: u32,
}

fn read_image_dimensions(path: &Path) -> Option<ImageDimensions> {
    let header = read_header(path).ok()?;
    read_image_dimensions_from_header(path, &header)
}

fn read_image_dimensions_from_header(path: &Path, header: &[u8]) -> Option<ImageDimensions> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => png_dimensions(header),
        "jpg" | "jpeg" => jpeg_dimensions(header),
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

fn png_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }

    Some(ImageDimensions {
        width: u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        height: u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    })
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }

    let mut index = 2;
    while index + 4 <= bytes.len() {
        while index < bytes.len() && bytes[index] != 0xff {
            index += 1;
        }
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }

        let marker = bytes[index];
        index += 1;

        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if index + 2 > bytes.len() {
            return None;
        }

        let segment_len = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
        if segment_len < 2 || index + segment_len > bytes.len() {
            return None;
        }

        if is_jpeg_sof_marker(marker) {
            if segment_len < 7 {
                return None;
            }
            let data_start = index + 2;
            return Some(ImageDimensions {
                height: u16::from_be_bytes([bytes[data_start + 1], bytes[data_start + 2]]) as u32,
                width: u16::from_be_bytes([bytes[data_start + 3], bytes[data_start + 4]]) as u32,
            });
        }

        index += segment_len;
    }

    None
}

fn is_jpeg_sof_marker(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

#[cfg(test)]
mod tests {
    use super::{jpeg_dimensions, png_dimensions, ImageDimensions};

    #[test]
    fn reads_png_dimensions() {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&640u32.to_be_bytes());
        bytes.extend_from_slice(&480u32.to_be_bytes());

        assert_eq!(
            png_dimensions(&bytes),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );
    }

    #[test]
    fn reads_jpeg_dimensions() {
        let bytes = [
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x01, 0xe0, 0x02, 0x80, 0x03, 0x00, 0x11,
            0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00,
        ];

        assert_eq!(
            jpeg_dimensions(&bytes),
            Some(ImageDimensions {
                width: 640,
                height: 480
            })
        );
    }
}
