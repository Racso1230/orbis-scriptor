//! Importing images into a world's `assets/` folder.
//!
//! Import is split in two steps so the database lock is never held while
//! files are copied (the command in `commands::maps` does the locking):
//!
//! 1. [`copy_into_assets`] - validates the image, copies the file and reads its
//!    size. Touches the file system only.
//! 2. `db::assets::insert` - a quick row insert, done by the caller under the
//!    lock. Then [`spawn_thumbnail`] makes the thumbnail in the background.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use chrono::{SecondsFormat, Utc};
use image::{ImageFormat, ImageReader};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::map::Asset;

/// Longest side of a generated thumbnail, in pixels.
pub const THUMB_MAX_SIDE: u32 = 512;

/// The formats we accept, as (format, file extension).
const ACCEPTED: [(ImageFormat, &str); 4] = [
    (ImageFormat::Png, "png"),
    (ImageFormat::Jpeg, "jpg"),
    (ImageFormat::WebP, "webp"),
    (ImageFormat::Gif, "gif"),
];

fn extension_for(format: ImageFormat) -> Option<&'static str> {
    ACCEPTED.iter().find(|(f, _)| *f == format).map(|(_, e)| *e)
}

/// Extension for a stored mime type (the file name is `<id>.<ext>`).
fn extension_for_mime(mime: &str) -> &'static str {
    ACCEPTED
        .iter()
        .find(|(f, _)| f.to_mime_type() == mime)
        .map(|(_, e)| *e)
        .unwrap_or("png")
}

/// Absolute path of the asset's image file.
pub fn asset_file(assets_dir: &Path, id: &str, mime: &str) -> PathBuf {
    assets_dir.join(format!("{id}.{}", extension_for_mime(mime)))
}

/// Absolute path of the asset's thumbnail (it may not exist yet).
pub fn thumb_file(assets_dir: &Path, id: &str) -> PathBuf {
    assets_dir.join(format!("{id}.thumb.png"))
}

/// Fills in the two path fields that are not stored in the database.
pub fn with_paths(mut asset: Asset, assets_dir: &Path) -> Asset {
    asset.path = asset_file(assets_dir, &asset.id, &asset.mime)
        .to_string_lossy()
        .into_owned();
    let thumb = thumb_file(assets_dir, &asset.id);
    asset.thumb_path = thumb.exists().then(|| thumb.to_string_lossy().into_owned());
    asset
}

/// Validates `source` as an image, copies it to `<assets_dir>/<id>.<ext>`
/// and returns the new `Asset` (not yet in the database).
pub fn copy_into_assets(source: &Path, assets_dir: &Path) -> AppResult<Asset> {
    let not_an_image = || {
        AppError::Validation(
            "The selected file is not a supported image (png, jpg, webp or gif)".into(),
        )
    };

    // `with_guessed_format` sniffs the first bytes of the file ("magic
    // bytes"), so a renamed text file is not accepted. `into_dimensions`
    // reads only the header, never decoding the pixels.
    let reader = ImageReader::open(source)?
        .with_guessed_format()
        .map_err(|_| not_an_image())?;
    let format = reader.format().ok_or_else(not_an_image)?;
    let ext = extension_for(format).ok_or_else(not_an_image)?;
    let (width, height) = reader.into_dimensions().map_err(|_| not_an_image())?;

    let id = Uuid::now_v7().to_string();
    fs::create_dir_all(assets_dir)?;
    fs::copy(source, assets_dir.join(format!("{id}.{ext}")))?;

    let file_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{id}.{ext}"));
    Ok(with_paths(
        Asset {
            id,
            file_name,
            mime: format.to_mime_type().to_owned(),
            width: Some(width),
            height: Some(height),
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            path: String::new(),
            thumb_path: None,
        },
        assets_dir,
    ))
}

/// Deletes the image file, the thumbnail and any half-written thumbnail of
/// an asset. Files that are already gone are fine; other failures are only
/// logged (the database row is what matters, a leftover file is harmless).
pub fn delete_files(assets_dir: &Path, id: &str, mime: &str) {
    let candidates = [
        asset_file(assets_dir, id, mime),
        thumb_file(assets_dir, id),
        assets_dir.join(format!("{id}.thumb.part")),
    ];
    for path in candidates {
        if let Err(err) = fs::remove_file(&path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("cannot delete {}: {err}", path.display());
        }
    }
}

/// Makes `<id>.thumb.png` (longest side <= 512px). Blocking; call it from a
/// background thread.
pub fn make_thumbnail(assets_dir: &Path, id: &str, mime: &str) -> AppResult<()> {
    let img = image::open(asset_file(assets_dir, id, mime))
        .map_err(|e| AppError::Internal(format!("cannot decode image: {e}")))?;
    let thumb = img.thumbnail(THUMB_MAX_SIDE, THUMB_MAX_SIDE);
    // Write to a temporary name and rename, so the final file never appears
    // half-written.
    let final_path = thumb_file(assets_dir, id);
    let tmp_path = assets_dir.join(format!("{id}.thumb.part"));
    thumb
        .save_with_format(&tmp_path, ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("cannot write thumbnail: {e}")))?;
    fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

/// Runs [`make_thumbnail`] on a new OS thread so the command can return at
/// once. `move` hands ownership of the cloned strings/paths to the thread
/// (it may outlive this function). A failure is only logged: a missing
/// thumbnail is not worth failing the import.
pub fn spawn_thumbnail(assets_dir: PathBuf, id: String, mime: String) {
    thread::spawn(move || {
        if let Err(err) = make_thumbnail(&assets_dir, &id, &mime) {
            eprintln!("thumbnail for asset {id} failed: {err}");
        }
    });
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Writes a small solid-colour PNG and returns its path.
    fn tiny_png(dir: &Path, w: u32, h: u32) -> PathBuf {
        let path = dir.join("source.png");
        image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]))
            .save(&path)
            .unwrap();
        path
    }

    #[test]
    fn copies_image_and_reads_dimensions() {
        let tmp = tempfile::tempdir().unwrap();
        let assets = tmp.path().join("assets");
        let src = tiny_png(tmp.path(), 40, 30);

        let asset = copy_into_assets(&src, &assets).unwrap();
        assert_eq!((asset.width, asset.height), (Some(40), Some(30)));
        assert_eq!(asset.mime, "image/png");
        assert_eq!(asset.file_name, "source.png");
        assert!(Path::new(&asset.path).is_file());
        assert!(asset.path.ends_with(&format!("{}.png", asset.id)));
        assert!(asset.thumb_path.is_none());
    }

    #[test]
    fn detects_format_by_content_not_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tiny_png(tmp.path(), 8, 8);
        let renamed = tmp.path().join("looks-like.jpg");
        fs::copy(&src, &renamed).unwrap();
        let asset = copy_into_assets(&renamed, &tmp.path().join("assets")).unwrap();
        assert_eq!(asset.mime, "image/png");
        assert!(asset.path.ends_with(".png"));
    }

    #[test]
    fn rejects_non_images() {
        let tmp = tempfile::tempdir().unwrap();
        let fake = tmp.path().join("fake.png");
        fs::write(&fake, "this is just text").unwrap();
        assert!(matches!(
            copy_into_assets(&fake, &tmp.path().join("assets")),
            Err(AppError::Validation(_))
        ));
        // Nothing was copied.
        assert!(!tmp.path().join("assets").exists());
    }

    #[test]
    fn thumbnail_is_scaled_down() {
        let tmp = tempfile::tempdir().unwrap();
        let assets = tmp.path().join("assets");
        let src = tiny_png(tmp.path(), 1024, 512);
        let asset = copy_into_assets(&src, &assets).unwrap();

        make_thumbnail(&assets, &asset.id, &asset.mime).unwrap();
        let thumb = with_paths(asset, &assets).thumb_path.unwrap();
        let (w, h) = image::image_dimensions(thumb).unwrap();
        assert_eq!((w, h), (512, 256));
    }
}
