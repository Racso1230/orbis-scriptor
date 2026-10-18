//! Writing exported files chosen by the user (map PNGs).
//!
//! The frontend renders the image (Konva), asks the user for a path with the
//! native save dialog, and sends the bytes here as a base64 string. Base64 is
//! plain text, which Tauri's JSON bridge handles far better than a
//! multi-megabyte array of numbers.

use std::fs;
use std::io::Write;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// Largest PNG we accept (decoded), 256 MiB.
const MAX_PNG_BYTES: usize = 256 * 1024 * 1024;
/// Every PNG file starts with these 8 bytes.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Decodes `png_base64` and writes it to `path`. The path must end in `.png`
/// and its folder must exist. The file is written to a temporary name in the
/// same folder and then renamed over the target, so a failure never leaves a
/// half-written export behind.
pub fn save_png(path: &str, png_base64: &str) -> AppResult<()> {
    let target = Path::new(path);
    let is_png = target
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if !is_png {
        return Err(AppError::Validation(
            "The export file name must end in .png".into(),
        ));
    }
    let dir = match target.parent() {
        Some(p) if p.as_os_str().is_empty() => Path::new("."),
        Some(p) => p,
        None => return Err(AppError::Validation("Invalid export path".into())),
    };
    if !dir.is_dir() {
        return Err(AppError::Validation(
            "The export folder does not exist".into(),
        ));
    }
    if png_base64.len() > MAX_PNG_BYTES / 3 * 4 + 4 {
        return Err(AppError::Validation("The image is too large".into()));
    }
    let bytes = STANDARD
        .decode(png_base64.trim())
        .map_err(|_| AppError::Validation("The image data is not valid base64".into()))?;
    if bytes.len() < PNG_SIGNATURE.len() || bytes[..8] != PNG_SIGNATURE {
        return Err(AppError::Validation("The data is not a PNG image".into()));
    }

    let tmp = dir.join(format!(".orbis-export-{}.tmp", Uuid::now_v7()));
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, target)
    })();
    if let Err(err) = written {
        let _ = fs::remove_file(&tmp);
        return Err(err.into());
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(4, 3, image::Rgba([200, 30, 30, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn writes_a_valid_png_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("map.PNG");
        let encoded = STANDARD.encode(tiny_png());
        save_png(path.to_str().unwrap(), &encoded).unwrap();
        let img = image::open(&path).unwrap();
        assert_eq!((img.width(), img.height()), (4, 3));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        // Exporting again over the same file works.
        save_png(path.to_str().unwrap(), &encoded).unwrap();
    }

    #[test]
    fn rejects_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        let ok = STANDARD.encode(tiny_png());
        let p = |name: &str| dir.path().join(name).to_str().unwrap().to_owned();
        for bad_path in [p("map.jpg"), p("map"), p("missing/map.png")] {
            assert!(matches!(
                save_png(&bad_path, &ok),
                Err(AppError::Validation(_))
            ));
        }
        assert!(matches!(
            save_png(&p("a.png"), "***not base64***"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            save_png(&p("a.png"), &STANDARD.encode(b"hello world")),
            Err(AppError::Validation(_))
        ));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
