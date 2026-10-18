//! Options, preview and event payloads for importing a Markdown / Obsidian
//! vault (see `services::vault_import`).

use serde::{Deserialize, Serialize};
use specta::Type;

/// A custom type the import should create for a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewTypeSpec {
    pub label: String,
    pub plural: String,
    /// An icon name from `list_type_icons`.
    pub icon: String,
    /// `#rrggbb`; empty means the default colour.
    pub color: String,
}

/// What notes from one folder become. Set exactly one of `typeKey` (an
/// existing type) and `createCustom` (a new custom type).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderTypeMapping {
    /// The folder NAME (not the whole path), as listed in
    /// `ImportPreview.unresolvedTypes`.
    pub folder: String,
    pub type_key: Option<String>,
    pub create_custom: Option<NewTypeSpec>,
}

/// Options for `preview_import` and `import_vault`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportOptions {
    /// Type for notes whose type cannot be worked out (for example `note`).
    pub default_type: String,
    /// Where notes of folders that match no type should go.
    pub folder_type_map: Vec<FolderTypeMapping>,
    /// A `[[link]]` to a note that is not in the vault becomes an empty Note
    /// entity (instead of plain text).
    pub create_missing_links_as_notes: bool,
    /// Import images as gallery images (otherwise a `[file.png]` placeholder
    /// is left in the text).
    pub import_images: bool,
    /// Skip a note when the world already has an entity of the same type
    /// with the same name (ignoring case). Links to it still work. When
    /// false, every note creates a new entity.
    pub skip_duplicates: bool,
}

/// How many notes will become entities of one type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TypeCount {
    pub type_key: String,
    pub label: String,
    pub count: u32,
    /// True when the type does not exist yet and the import creates it.
    pub new_type: bool,
}

/// A folder whose notes matched no type (they use the default type unless
/// the folder is mapped in `ImportOptions.folderTypeMap`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnmappedFolder {
    pub folder: String,
    pub count: u32,
}

/// What an import would do, computed without writing anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    /// Notes that will become entities (after limits and skipped duplicates).
    pub notes: u32,
    pub by_type: Vec<TypeCount>,
    pub unresolved_types: Vec<UnmappedFolder>,
    /// Distinct image files that notes embed and that can be imported.
    pub images: u32,
    /// Distinct tags (front matter and inline `#tags`).
    pub tags: u32,
    /// Notes skipped because the world already has them (`skipDuplicates`).
    pub skipped: u32,
    /// Relationships found in an Orbis export (`## Relationships` sections).
    pub relationships: u32,
    /// True when the folder is an Orbis Scriptor Markdown export.
    pub orbis_export: bool,
    /// The first problems found (at most 50), in plain words.
    pub warnings: Vec<String>,
    /// How many warnings were left out of the list.
    pub more_warnings: u32,
}

/// Progress of a running import. Sent as the event `import-progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    /// The id returned by `import_vault`.
    pub job_id: String,
    pub done: u32,
    pub total: u32,
    /// `scanning`, `reading`, `converting`, `writing` or `finishing`.
    pub phase: String,
}

/// Sent once as the event `import-finished`, after success or failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ImportFinished {
    pub job_id: String,
    pub ok: bool,
    /// Entities created (notes plus placeholder notes). 0 on failure: a
    /// failed import changes nothing.
    pub created: u32,
    pub skipped: u32,
    pub images: u32,
    pub relationships: u32,
    /// Custom types the import created.
    pub types_created: u32,
    /// The first problems found (at most 100).
    pub warnings: Vec<String>,
    pub more_warnings: u32,
    /// The error message on failure.
    pub error: Option<String>,
}
