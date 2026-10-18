//! Walking a Markdown / Obsidian vault folder: which notes and images are
//! in it. Reads the file system only.
//!
//! Rules:
//! - Hidden files and folders (names starting with `.`: `.obsidian`,
//!   `.trash`, `.git`, an export's `.orbis-export` marker) are skipped, and
//!   symbolic links are not followed.
//! - Notes are `.md` files (any case). At most [`MAX_NOTES`] are taken (the
//!   first ones in path order) and a note larger than [`MAX_NOTE_BYTES`] is
//!   skipped; each skip is reported as a warning.
//! - In an Orbis Scriptor export (the folder has a `.orbis-export` file) the
//!   index note `README.md` at the top is not a note.
//! - Images are `.png .jpg .jpeg .gif .webp .svg .bmp .avif` files anywhere
//!   in the vault; they are found by name the way Obsidian does.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::services::markdown_export::{ExportManifest, INDEX_FILE, MARKER_FILE};
use crate::services::markdown_import::is_image_path;

/// Most notes one import reads.
pub const MAX_NOTES: usize = 5_000;
/// Largest note we read, in bytes (2 MB).
pub const MAX_NOTE_BYTES: u64 = 2 * 1024 * 1024;
/// Largest image we import, in bytes (30 MB).
pub const MAX_IMAGE_BYTES: u64 = 30 * 1024 * 1024;
/// Deepest folder nesting we follow.
const MAX_DEPTH: usize = 32;
/// Most directory entries visited, a guard against enormous folders.
const MAX_ENTRIES: usize = 300_000;

/// One `.md` file of the vault.
#[derive(Debug, Clone)]
pub struct VaultNote {
    pub path: PathBuf,
    /// Path inside the vault with `/` separators, e.g. `Characters/Ada.md`.
    pub rel: String,
    /// File name without `.md`: what `[[links]]` use.
    pub stem: String,
    /// Names of the folders above the note, nearest first.
    pub folders: Vec<String>,
}

/// One image file of the vault.
#[derive(Debug, Clone)]
pub struct VaultImage {
    pub path: PathBuf,
    pub rel: String,
    pub size: u64,
}

#[derive(Debug, Default)]
pub struct Vault {
    pub root: PathBuf,
    pub notes: Vec<VaultNote>,
    pub images: Vec<VaultImage>,
    /// True when the folder is an Orbis Scriptor Markdown export.
    pub orbis_export: bool,
    /// The export's manifest (custom types), when readable.
    pub manifest: Option<ExportManifest>,
    pub warnings: Vec<String>,
}

/// Finds images by path or by file name.
#[derive(Debug, Default)]
pub struct ImageIndex {
    images: Vec<VaultImage>,
    by_rel: HashMap<String, usize>,
    by_name: HashMap<String, Vec<usize>>,
}

impl ImageIndex {
    pub fn new(images: Vec<VaultImage>) -> Self {
        let mut index = ImageIndex::default();
        for (i, image) in images.iter().enumerate() {
            index.by_rel.insert(image.rel.to_lowercase(), i);
            let name = image.rel.rsplit('/').next().unwrap_or("").to_lowercase();
            index.by_name.entry(name).or_default().push(i);
        }
        index.images = images;
        index
    }

    pub fn images(&self) -> &[VaultImage] {
        &self.images
    }

    /// Looks `src` up the way Obsidian and Markdown do: relative to the
    /// note's folder, then relative to the vault root, then by file name
    /// alone (the shortest path wins when several files share the name).
    /// `note_dir` is the note's folder inside the vault (`""` at the top).
    pub fn find(&self, note_dir: &str, src: &str) -> Option<&VaultImage> {
        let src = src.trim().trim_start_matches("./");
        if src.is_empty() {
            return None;
        }
        let relative = normalise(&format!("{note_dir}/{src}"));
        let candidates = [relative, normalise(src)];
        for candidate in candidates.iter().flatten() {
            if let Some(i) = self.by_rel.get(&candidate.to_lowercase()) {
                return Some(&self.images[*i]);
            }
        }
        let name = src.rsplit(['/', '\\']).next().unwrap_or("").to_lowercase();
        self.by_name
            .get(&name)?
            .iter()
            .map(|i| &self.images[*i])
            .min_by_key(|image| (image.rel.len(), image.rel.clone()))
    }
}

/// Resolves `.` and `..` in a `/` path; `None` if it climbs out of the vault.
fn normalise(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

fn lossy_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn is_markdown(name: &str) -> bool {
    name.len() > 3 && name[name.len() - 3..].eq_ignore_ascii_case(".md")
}

/// Walks `root`. Fails only when `root` is not a readable folder; problems
/// with single entries become warnings.
pub fn scan(root: &Path) -> AppResult<Vault> {
    if !root.is_dir() {
        return Err(AppError::Validation(
            "The vault folder does not exist".into(),
        ));
    }
    let mut vault = Vault {
        root: root.to_path_buf(),
        ..Default::default()
    };
    let marker = root.join(MARKER_FILE);
    vault.orbis_export = marker.is_file();
    if vault.orbis_export {
        // Older exports wrote `{}`; an unreadable manifest is not an error.
        vault.manifest = fs::read_to_string(&marker)
            .ok()
            .and_then(|text| serde_json::from_str::<ExportManifest>(&text).ok());
    }

    let mut notes: Vec<VaultNote> = Vec::new();
    let mut visited = 0usize;
    let mut stack: Vec<(PathBuf, Vec<String>)> = vec![(root.to_path_buf(), Vec::new())];
    while let Some((dir, folders)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(err) => {
                vault
                    .warnings
                    .push(format!("Cannot read the folder {}: {err}", dir.display()));
                continue;
            }
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_ENTRIES {
                vault
                    .warnings
                    .push("The folder is very large; the scan stopped early".into());
                stack.clear();
                break;
            }
            let name = lossy_name(&entry.path());
            if name.starts_with('.') {
                continue;
            }
            // `file_type` does not follow symbolic links, so links are neither
            // files nor folders here and are skipped.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            let rel_of = |name: &str| {
                if folders.is_empty() {
                    name.to_owned()
                } else {
                    let mut up: Vec<&str> = folders.iter().rev().map(String::as_str).collect();
                    up.push(name);
                    up.join("/")
                }
            };
            if kind.is_dir() {
                if folders.len() < MAX_DEPTH {
                    let mut child = vec![name.clone()];
                    child.extend(folders.iter().cloned());
                    stack.push((path, child));
                }
            } else if kind.is_file() && is_markdown(&name) {
                if vault.orbis_export && folders.is_empty() && name == INDEX_FILE {
                    continue;
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if size > MAX_NOTE_BYTES {
                    vault.warnings.push(format!(
                        "Skipped {}: the note is larger than {} MB",
                        rel_of(&name),
                        MAX_NOTE_BYTES / 1024 / 1024
                    ));
                    continue;
                }
                notes.push(VaultNote {
                    path,
                    rel: rel_of(&name),
                    stem: name[..name.len() - 3].to_owned(),
                    folders: folders.clone(),
                });
            } else if kind.is_file() && is_image_path(&name) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                vault.images.push(VaultImage {
                    path,
                    rel: rel_of(&name),
                    size,
                });
            }
        }
    }
    // Stable order whatever the file system returned: by path, ignoring case.
    notes.sort_by_cached_key(|n| (n.rel.to_lowercase(), n.rel.clone()));
    vault
        .images
        .sort_by_cached_key(|i| (i.rel.to_lowercase(), i.rel.clone()));
    if notes.len() > MAX_NOTES {
        vault.warnings.push(format!(
            "The vault has {} notes; only the first {MAX_NOTES} are imported",
            notes.len()
        ));
        notes.truncate(MAX_NOTES);
    }
    vault.notes = notes;
    Ok(vault)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn walks_nested_folders_and_skips_hidden_ones() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        write(r, "Top.md", "x");
        write(r, "People/Ada.MD", "x");
        write(r, "People/Heroes/Bob.md", "x");
        write(r, ".obsidian/workspace.md", "x");
        write(r, ".trash/Old.md", "x");
        write(r, "People/.hidden.md", "x");
        write(r, "notes.txt", "x");
        write(r, "pics/a.png", "not really");
        write(r, "pics/doc.pdf", "x");
        let vault = scan(r).unwrap();
        let rels: Vec<&str> = vault.notes.iter().map(|n| n.rel.as_str()).collect();
        assert_eq!(
            rels,
            vec!["People/Ada.MD", "People/Heroes/Bob.md", "Top.md"]
        );
        assert_eq!(vault.notes[1].stem, "Bob");
        assert_eq!(vault.notes[1].folders, vec!["Heroes", "People"]);
        assert_eq!(vault.images.len(), 1);
        assert!(!vault.orbis_export);
        assert!(vault.warnings.is_empty());
    }

    #[test]
    fn orbis_exports_skip_their_index_note() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        write(r, MARKER_FILE, "{}");
        write(r, "README.md", "index");
        write(r, "Notes/README.md", "a real note");
        let vault = scan(r).unwrap();
        assert!(vault.orbis_export);
        assert!(vault.manifest.is_some());
        let rels: Vec<&str> = vault.notes.iter().map(|n| n.rel.as_str()).collect();
        assert_eq!(rels, vec!["Notes/README.md"]);
        // Without the marker the same file is an ordinary note.
        fs::remove_file(r.join(MARKER_FILE)).unwrap();
        assert_eq!(scan(r).unwrap().notes.len(), 2);
    }

    #[test]
    fn limits_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        write(r, "small.md", "x");
        let big = vec![b'a'; (MAX_NOTE_BYTES + 1) as usize];
        fs::write(r.join("big.md"), big).unwrap();
        let vault = scan(r).unwrap();
        assert_eq!(vault.notes.len(), 1);
        assert!(vault.warnings.iter().any(|w| w.contains("big.md")));
        assert!(matches!(
            scan(&r.join("missing")),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn images_are_found_by_relative_path_then_by_name() {
        let img = |rel: &str| VaultImage {
            path: PathBuf::from(rel),
            rel: rel.to_owned(),
            size: 1,
        };
        let index = ImageIndex::new(vec![
            img("attachments/Map.PNG"),
            img("People/pics/ada.png"),
            img("zzz/deep/folder/ada.png"),
        ]);
        let find = |dir: &str, src: &str| index.find(dir, src).map(|i| i.rel.as_str());
        // By name alone, ignoring case; the shortest path wins.
        assert_eq!(find("", "map.png"), Some("attachments/Map.PNG"));
        assert_eq!(find("People", "ada.png"), Some("People/pics/ada.png"));
        // Relative to the note, with `..` and `./`.
        assert_eq!(find("People", "pics/ada.png"), Some("People/pics/ada.png"));
        assert_eq!(
            find("People/Heroes", "../pics/ada.png"),
            Some("People/pics/ada.png")
        );
        assert_eq!(
            find("People", "./pics/ada.png"),
            Some("People/pics/ada.png")
        );
        // Relative to the vault root.
        assert_eq!(
            find("People", "attachments/Map.PNG"),
            Some("attachments/Map.PNG")
        );
        // A path that exists nowhere falls back to the file name.
        assert_eq!(find("", "nowhere/ada.png"), Some("People/pics/ada.png"));
        assert_eq!(find("", "missing.png"), None);
        assert_eq!(find("", "  "), None);
    }
}
