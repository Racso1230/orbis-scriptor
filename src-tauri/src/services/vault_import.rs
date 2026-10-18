//! Importing a Markdown / Obsidian vault into the open world.
//!
//! The work is split in two so the database is only locked for a moment:
//!
//! 1. **Plan** (no writes, can take a while): read every note, work out its
//!    type, name and fields, give it a new id (pass 1), then convert each
//!    body to a TipTap document, resolving `[[links]]` to those ids (pass 2).
//!    [`preview`] runs only this step and reports what it found.
//! 2. **Write** (one transaction, all or nothing): create the custom types,
//!    copy the images, insert every entity, then documents, mentions,
//!    `field_ref` links, tags, gallery images, relationships and the search
//!    index. If anything fails the transaction is rolled back and the copied
//!    image files are removed, so a failed import leaves the world exactly as
//!    it was.
//!
//! Mapping rules (also in the Phase 13 plan):
//!
//! - **Type** of a note: front matter `type_key`, else `type` (matched with a
//!   type's label or plural), else the folder (your `folderTypeMap`, then a
//!   folder named like a type's plural/label, nearest folder first), else the
//!   default type. Folders that matched nothing are reported as
//!   `unresolvedTypes`.
//! - **Name**: the file name; for an Orbis export note (it has `id` and
//!   `type_key`) the first `aliases` entry, which holds the exact name.
//! - **Links**: `[[Target]]`, `[[Target|Alias]]` and `[[Target#Heading]]`
//!   become mentions of the entity whose file name (else name, else alias)
//!   matches, ignoring case. Unresolved links stay as their text, or become
//!   empty Note entities with `createMissingLinksAsNotes`.
//! - **Fields**: front matter values are converted per field kind (dates from
//!   `<key>_day`, else parsed with the world's display calendar; entity
//!   references resolved like links). A value that does not fit its field is
//!   dropped with a warning. Keys that match no field are kept as properties
//!   (scalars as they are, lists as lists of text, mappings as JSON text), so
//!   nothing is lost.
//! - **Orbis exports** also bring back relationships (`## Relationships`),
//!   event participants (`## Involved`), the gallery (`portrait` / `images`),
//!   a map's background image, timestamps, and the custom types recorded in
//!   `.orbis-export`; the generated parts of the note (title, summary,
//!   `**Date:**`, `## Events`, `## Pins`, ...) are not copied into the lore.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::calendar::CalendarDefinition;
use crate::models::document::Document;
use crate::models::entity::Entity;
use crate::models::entity_type::{EntityTypeDef, FieldDef, FieldInput, FieldKind};
use crate::models::import::{
    FolderTypeMapping, ImportOptions, ImportPreview, NewTypeSpec, TypeCount, UnmappedFolder,
};
use crate::models::relationship as catalog;
use crate::services::front_matter;
use crate::services::markdown::plain_paragraph;
use crate::services::markdown_export::RESERVED_KEYS;
use crate::services::markdown_import::{self, Hooks, ImageAction};
use crate::services::vault_scan::{self, ImageIndex, MAX_IMAGE_BYTES, Vault, VaultNote};
use crate::services::world::{ASSETS_DIR, DB_FILE};
use crate::services::{
    assets, calendar, calendar_manager, entity_types, events, gallery, links, maps, properties,
    relationships, search, tags,
};

const MAX_WARNINGS_PREVIEW: usize = 50;
const MAX_WARNINGS_IMPORT: usize = 100;
/// Most gallery images one note may bring.
const MAX_NOTE_IMAGES: usize = gallery::MAX_IMAGES as usize;
/// Longest name we accept for a placeholder note made from a link.
const MAX_PLACEHOLDER_CHARS: usize = 200;
/// Headings of the sections an Orbis export writes at the end of a note.
const GENERATED_SECTIONS: [&str; 6] = [
    "Relationships",
    "Events",
    "Involved",
    "Pins",
    "Areas",
    crate::services::markdown_export::GALLERY_HEADING,
];

/// Called as `(done, total, phase)`. Returning an error stops the import
/// (and rolls it back), which the tests use to simulate a failure.
pub type ProgressFn<'a> = &'a mut dyn FnMut(u32, u32, &str) -> AppResult<()>;

/// What a finished import created.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub created: u32,
    pub skipped: u32,
    pub images: u32,
    pub relationships: u32,
    pub types_created: u32,
    pub warnings: Vec<String>,
    pub more_warnings: u32,
}

/// Keeps the first `cap` warnings and counts the rest.
struct Warnings {
    list: Vec<String>,
    cap: usize,
    more: u32,
}

impl Warnings {
    fn new(cap: usize) -> Self {
        Warnings {
            list: Vec::new(),
            cap,
            more: 0,
        }
    }

    fn push(&mut self, message: impl Into<String>) {
        if self.list.len() < self.cap {
            self.list.push(message.into());
        } else {
            self.more += 1;
        }
    }
}

/// Reports progress at most about 100 times per phase.
struct Throttle {
    step: u32,
}

impl Throttle {
    fn new(total: usize) -> Self {
        Throttle {
            step: (total as u32 / 100).max(1),
        }
    }

    fn tick(&self, done: usize, total: usize, phase: &str, f: ProgressFn) -> AppResult<()> {
        if (done as u32).is_multiple_of(self.step) || done == total {
            f(done as u32, total as u32, phase)?;
        }
        Ok(())
    }
}

// ---- the world as the import sees it ----------------------------------------

struct WorldInfo {
    types: Vec<EntityTypeDef>,
    /// `(type key, lower-case name)` -> id of an existing entity.
    existing: HashMap<(String, String), String>,
    /// The world's display calendar, for reading dates written as text.
    calendar: CalendarDefinition,
}

fn read_world(conn: &Connection) -> AppResult<WorldInfo> {
    let mut existing = HashMap::new();
    for e in db::entities::list(conn, None)? {
        existing
            .entry((e.entity_type.clone(), e.name.to_lowercase()))
            .or_insert(e.id);
    }
    Ok(WorldInfo {
        types: entity_types::merged(conn)?,
        existing,
        calendar: calendar_manager::get_display(conn)?.definition,
    })
}

// ---- types -------------------------------------------------------------------

/// A type notes can have in this import: one the world already has, or one
/// the import creates (its `key` is then provisional, `new:<n>`).
struct PlanType {
    key: String,
    label: String,
    plural: String,
    fields: Vec<FieldDef>,
    /// Some for types the import creates.
    spec: Option<NewTypeSpec>,
    /// For a created type that came from an export's manifest: its fields.
    manifest_fields: Option<Vec<FieldDef>>,
    builtin: bool,
}

#[derive(Default)]
struct Types {
    list: Vec<PlanType>,
    /// Lower-case key written in notes (`custom-deity`) -> index in `list`.
    alias: HashMap<String, usize>,
    /// Extra fields for built-in types from an export's manifest, by index.
    template_additions: BTreeMap<usize, Vec<FieldDef>>,
}

impl Types {
    fn from_world(world: &[EntityTypeDef]) -> Types {
        let mut types = Types::default();
        for t in world {
            types.alias.insert(t.key.to_lowercase(), types.list.len());
            types.list.push(PlanType {
                key: t.key.clone(),
                label: t.label.clone(),
                plural: t.plural.clone(),
                fields: t.fields.clone(),
                spec: None,
                manifest_fields: None,
                builtin: t.builtin,
            });
        }
        types
    }

    fn by_key(&self, key: &str) -> Option<usize> {
        self.alias.get(&key.trim().to_lowercase()).copied()
    }

    /// A type whose label or plural equals `name` (ignoring case).
    fn by_name(&self, name: &str) -> Option<usize> {
        let wanted = name.trim().to_lowercase();
        if wanted.is_empty() {
            return None;
        }
        self.list
            .iter()
            .position(|t| t.label.to_lowercase() == wanted || t.plural.to_lowercase() == wanted)
    }

    /// Adds a type the import will create. Fails when its names clash with
    /// another type.
    fn add_new(
        &mut self,
        spec: NewTypeSpec,
        manifest_fields: Option<Vec<FieldDef>>,
    ) -> AppResult<usize> {
        for t in &self.list {
            if t.label.to_lowercase() == spec.label.trim().to_lowercase() {
                return Err(AppError::Validation(format!(
                    "A type called \"{}\" already exists",
                    t.label
                )));
            }
            if t.plural.to_lowercase() == spec.plural.trim().to_lowercase() {
                return Err(AppError::Validation(format!(
                    "A type with the plural \"{}\" already exists",
                    t.plural
                )));
            }
        }
        let idx = self.list.len();
        self.list.push(PlanType {
            key: format!("new:{idx}"),
            label: spec.label.trim().to_owned(),
            plural: spec.plural.trim().to_owned(),
            fields: manifest_fields.clone().unwrap_or_default(),
            spec: Some(spec),
            manifest_fields,
            builtin: false,
        });
        Ok(idx)
    }
}

/// Registers the custom types and extra fields recorded in an export.
fn apply_manifest(
    types: &mut Types,
    manifest: &crate::services::markdown_export::ExportManifest,
    warnings: &mut Warnings,
) {
    let mut created: Vec<usize> = Vec::new();
    for m in &manifest.custom_types {
        // Reuse a type of the world with the same key and label, or the same label.
        let same_key = types
            .by_key(&m.key)
            .filter(|i| types.list[*i].label.to_lowercase() == m.label.to_lowercase());
        if let Some(i) = same_key.or_else(|| types.by_name(&m.label)) {
            types.alias.insert(m.key.to_lowercase(), i);
            continue;
        }
        let spec = NewTypeSpec {
            label: m.label.clone(),
            plural: m.plural.clone(),
            icon: m.icon.clone(),
            color: m.color.clone(),
        };
        match types.add_new(spec, Some(m.fields.clone())) {
            Ok(i) => {
                types.alias.insert(m.key.to_lowercase(), i);
                created.push(i);
            }
            Err(e) => warnings.push(format!("Type \"{}\" from the export: {e}", m.label)),
        }
    }
    // Entity-reference fields name types by their OLD keys: translate them.
    let lookup: HashMap<String, String> = types
        .alias
        .iter()
        .map(|(k, i)| (k.clone(), types.list[*i].key.clone()))
        .collect();
    let remap = |fields: &mut Vec<FieldDef>| {
        for f in fields {
            if let FieldKind::EntityRef { allowed_types, .. } = &mut f.kind {
                for t in allowed_types.iter_mut() {
                    if let Some(new) = lookup.get(&t.to_lowercase()) {
                        *t = new.clone();
                    }
                }
            }
        }
    };
    for i in created {
        let t = &mut types.list[i];
        remap(&mut t.fields);
        if let Some(mf) = &mut t.manifest_fields {
            remap(mf);
        }
    }
    // Extra fields of built-in types.
    for (key, extra) in &manifest.templates {
        let Some(i) = types.by_key(key) else {
            continue;
        };
        if !types.list[i].builtin {
            continue;
        }
        let mut extra = extra.clone();
        remap(&mut extra);
        let have: HashSet<String> = types.list[i].fields.iter().map(|f| f.key.clone()).collect();
        extra.retain(|f| !have.contains(&f.key));
        if !extra.is_empty() {
            types.list[i].fields.extend(extra.iter().cloned());
            types.template_additions.insert(i, extra);
        }
    }
}

// ---- notes -------------------------------------------------------------------

/// A note read from disk and given its identity (pass 1).
struct PlanNote {
    vault: VaultNote,
    id: String,
    name: String,
    type_idx: usize,
    front: Map<String, Value>,
    /// The Markdown body after the front matter and, for an Orbis export,
    /// without the generated title / summary / sections.
    body: String,
    /// A note of the same type and name exists: do not create it.
    skip: bool,
    aliases: Vec<String>,
    summary: String,
    created_at: String,
    updated_at: String,
    relationship_lines: Vec<(String, String)>,
    involved_lines: Vec<String>,
    map_embed: Option<String>,
}

/// A gallery image of one note.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanImage {
    /// Path of the image inside the vault.
    rel: String,
    caption: String,
    portrait: bool,
}

/// Everything pass 2 works out about one note.
#[derive(Default)]
struct NoteResult {
    doc: Option<Value>,
    plain_text: String,
    props: Map<String, Value>,
    tags: Vec<String>,
    images: Vec<PlanImage>,
    map_background: Option<String>,
    event: Option<(i64, Option<i64>, i32)>,
    /// `(label, target id)` from `## Relationships`.
    rels: Vec<(String, String)>,
    involved: Vec<String>,
    unresolved: Vec<String>,
    warnings: Vec<String>,
}

/// Looks notes up by name: file name first, then the entity name, then
/// aliases; all ignoring case. Values are entity ids.
#[derive(Default)]
struct NoteIndex {
    by_stem: HashMap<String, String>,
    by_name: HashMap<String, String>,
    by_alias: HashMap<String, String>,
    /// Placeholder notes made from unresolved links.
    placeholders: HashMap<String, String>,
    /// Id -> name, for the plain text of mentions.
    names: HashMap<String, String>,
    /// Id -> index of the note's type (for `allowedTypes` checks).
    type_of: HashMap<String, usize>,
}

impl NoteIndex {
    fn resolve(&self, name: &str) -> Option<String> {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            return None;
        }
        [
            &self.by_stem,
            &self.by_name,
            &self.by_alias,
            &self.placeholders,
        ]
        .iter()
        .find_map(|m| m.get(&key).cloned())
    }
}

fn lower_text(text: &str) -> String {
    text.trim().to_lowercase()
}

/// Of several notes with the same name, the one with the shortest path.
fn insert_shortest(
    map: &mut HashMap<String, (String, String)>,
    key: String,
    rel: &str,
    id: &str,
) -> bool {
    match map.get(&key) {
        Some((old_rel, _)) if (old_rel.len(), old_rel.as_str()) <= (rel.len(), rel) => true,
        Some(_) => {
            map.insert(key, (rel.to_owned(), id.to_owned()));
            true
        }
        None => {
            map.insert(key, (rel.to_owned(), id.to_owned()));
            false
        }
    }
}

// ---- the plan ----------------------------------------------------------------

struct Plan {
    notes: Vec<PlanNote>,
    results: Vec<NoteResult>,
    /// `(id, name)` of empty notes made from unresolved links.
    placeholders: Vec<(String, String)>,
    types: Types,
    index: NoteIndex,
    images: ImageIndex,
    unmapped: BTreeMap<String, u32>,
    tag_names: BTreeSet<String>,
    orbis_export: bool,
    warnings: Warnings,
}

fn validate_options(options: &ImportOptions, world: &WorldInfo) -> AppResult<()> {
    if !world.types.iter().any(|t| t.key == options.default_type) {
        return Err(AppError::Validation(format!(
            "Unknown default type \"{}\"",
            options.default_type
        )));
    }
    for m in &options.folder_type_map {
        match (&m.type_key, &m.create_custom) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(AppError::Validation(format!(
                    "Folder \"{}\": choose either an existing type or a new type",
                    m.folder
                )));
            }
            (Some(key), None) => {
                if !world.types.iter().any(|t| &t.key == key) {
                    return Err(AppError::Validation(format!(
                        "Folder \"{}\": unknown type \"{key}\"",
                        m.folder
                    )));
                }
            }
            (None, Some(_)) => {}
        }
    }
    Ok(())
}

/// How a note's type was found.
enum TypeChoice {
    Idx(usize),
    /// Nothing matched: the default type. `Some(folder)` names the nearest
    /// folder, which the wizard can offer to map.
    Default(Option<String>),
}

/// Picks the type of a note (see the module docs for the order).
fn choose_type(
    types: &mut Types,
    note: &VaultNote,
    front: &Map<String, Value>,
    options: &ImportOptions,
    conn: &Connection,
    mapping_cache: &mut HashMap<String, usize>,
    unknown_labels: &mut BTreeSet<String>,
) -> AppResult<TypeChoice> {
    if let Some(key) = front.get("type_key").and_then(Value::as_str)
        && let Some(i) = types.by_key(key)
    {
        return Ok(TypeChoice::Idx(i));
    }
    if let Some(label) = front.get("type").and_then(front_matter::scalar_text) {
        match types.by_name(&label) {
            Some(i) => return Ok(TypeChoice::Idx(i)),
            None => {
                unknown_labels.insert(label);
            }
        }
    }
    // Your mapping, nearest folder first.
    for folder in &note.folders {
        let wanted = folder.to_lowercase();
        if let Some(mapping) = options
            .folder_type_map
            .iter()
            .find(|m| m.folder.to_lowercase() == wanted)
        {
            return resolve_mapping(types, mapping, conn, mapping_cache).map(TypeChoice::Idx);
        }
    }
    // A folder named like a type.
    for folder in &note.folders {
        if let Some(i) = types.by_name(folder) {
            return Ok(TypeChoice::Idx(i));
        }
    }
    Ok(TypeChoice::Default(note.folders.first().cloned()))
}

fn resolve_mapping(
    types: &mut Types,
    mapping: &FolderTypeMapping,
    conn: &Connection,
    cache: &mut HashMap<String, usize>,
) -> AppResult<usize> {
    if let Some(key) = &mapping.type_key {
        return types
            .by_key(key)
            .ok_or_else(|| AppError::Validation(format!("Unknown type \"{key}\"")));
    }
    let Some(spec) = &mapping.create_custom else {
        return Err(AppError::Validation(format!(
            "Folder \"{}\" has no type",
            mapping.folder
        )));
    };
    let cache_key = mapping.folder.to_lowercase();
    if let Some(i) = cache.get(&cache_key) {
        return Ok(*i);
    }
    // Same checks as creating the type for real (icon, colour, lengths).
    entity_types::prepare_new(
        conn,
        &spec.label,
        &spec.plural,
        &spec.icon,
        &spec.color,
        Vec::new(),
    )?;
    let i = types.add_new(spec.clone(), None)?;
    cache.insert(cache_key, i);
    Ok(i)
}

fn read_note(note: &VaultNote) -> AppResult<String> {
    let bytes = std::fs::read(&note.path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn parse_timestamp(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    DateTime::parse_from_rfc3339(text).ok().map(|d| {
        d.with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    })
}

/// Parts of an Orbis export note that the importer reads and then removes.
struct Stripped {
    body: String,
    relationships: Vec<(String, String)>,
    involved: Vec<String>,
    map_embed: Option<String>,
}

fn unescape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(next) = chars.peek()
            && next.is_ascii_punctuation()
        {
            out.push(*next);
            chars.next();
            continue;
        }
        out.push(c);
    }
    out
}

/// The `[[target]]` of a Markdown line, without alias or heading.
fn first_wikilink(line: &str) -> Option<(usize, String)> {
    let start = line.find("[[")?;
    let end = line[start..].find("]]")? + start;
    let inner = &line[start + 2..end];
    let target = inner.split('|').next().unwrap_or("");
    Some((
        start,
        markdown_import::note_name_of(target.trim_end_matches('\\')),
    ))
}

/// Removes what an Orbis export generated around the lore (see the module
/// docs) and returns the pieces worth keeping.
fn strip_generated(body: &str, summary: &str, is_event: bool, is_map: bool) -> Stripped {
    let text = body.replace("\r\n", "\n");
    let lines: Vec<&str> = text.lines().collect();

    // Generated sections sit at the end: find the first `## X` heading such
    // that every later `## ` heading is a generated one too.
    let mut fenced = false;
    let mut headings: Vec<(usize, &str)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fenced = !fenced;
        }
        if !fenced && let Some(title) = line.strip_prefix("## ") {
            headings.push((i, title.trim()));
        }
    }
    let mut cut = lines.len();
    for (i, title) in headings.iter().rev() {
        if GENERATED_SECTIONS.contains(title) {
            cut = *i;
        } else {
            break;
        }
    }
    let mut relationships = Vec::new();
    let mut involved = Vec::new();
    let mut section = "";
    for line in &lines[cut..] {
        if let Some(title) = line.strip_prefix("## ") {
            section = GENERATED_SECTIONS
                .iter()
                .find(|s| **s == title.trim())
                .copied()
                .unwrap_or("");
            continue;
        }
        let Some(item) = line.strip_prefix("- ") else {
            continue;
        };
        match section {
            "Relationships" => {
                if let Some((start, target)) = first_wikilink(item) {
                    let label = unescape_markdown(item[..start].trim());
                    if !label.is_empty() && !target.is_empty() {
                        relationships.push((label, target));
                    }
                }
            }
            "Involved" => {
                if let Some((_, target)) = first_wikilink(item) {
                    involved.push(target);
                }
            }
            _ => {}
        }
    }

    // Generated start: title, summary, date line, map picture.
    let mut head: Vec<&str> = lines[..cut].to_vec();
    let mut map_embed = None;
    let take_block = |head: &mut Vec<&str>, accept: &dyn Fn(&str) -> bool| -> Option<String> {
        let start = head.iter().position(|l| !l.trim().is_empty())?;
        let end = head[start..]
            .iter()
            .position(|l| l.trim().is_empty())
            .map_or(head.len(), |p| start + p);
        let block = head[start..end].join("\n");
        if accept(block.trim()) {
            head.drain(..end);
            Some(block)
        } else {
            None
        }
    };
    take_block(&mut head, &|b| b.starts_with("# "));
    if !summary.trim().is_empty() {
        let expected = plain_paragraph(summary);
        take_block(&mut head, &|b| b == expected);
    }
    if is_event {
        take_block(&mut head, &|b| b.starts_with("**Date:**"));
    }
    if is_map
        && let Some(block) = take_block(&mut head, &|b| b.starts_with("![[") && b.ends_with("]]"))
    {
        let inner = block
            .trim()
            .trim_start_matches("![[")
            .trim_end_matches("]]");
        map_embed = Some(inner.split('|').next().unwrap_or("").to_owned());
    }
    Stripped {
        body: head.join("\n"),
        relationships,
        involved,
        map_embed,
    }
}

/// Pass 1: reads the notes, picks types and names, gives every note an id.
fn build_plan(
    vault: &Vault,
    world: &WorldInfo,
    conn: &Connection,
    options: &ImportOptions,
    warnings_cap: usize,
    on_progress: ProgressFn,
) -> AppResult<Plan> {
    validate_options(options, world)?;
    let mut warnings = Warnings::new(warnings_cap);
    for w in &vault.warnings {
        warnings.push(w.clone());
    }
    let mut types = Types::from_world(&world.types);
    if let Some(manifest) = &vault.manifest {
        apply_manifest(&mut types, manifest, &mut warnings);
    }
    let default_idx = types
        .by_key(&options.default_type)
        .ok_or_else(|| AppError::Validation("Unknown default type".into()))?;

    // ---- read, parse, choose types ----
    on_progress(0, 0, "scanning")?;
    let total = vault.notes.len();
    let throttle = Throttle::new(total);
    let mut notes: Vec<PlanNote> = Vec::with_capacity(total);
    let mut unmapped: BTreeMap<String, u32> = BTreeMap::new();
    let mut unknown_labels: BTreeSet<String> = BTreeSet::new();
    let mut mapping_cache: HashMap<String, usize> = HashMap::new();
    for (n, note) in vault.notes.iter().enumerate() {
        throttle.tick(n, total, "reading", on_progress)?;
        let text = match read_note(note) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(format!("Skipped {}: {e}", note.rel));
                continue;
            }
        };
        let split = front_matter::split(&text);
        let front = match split.yaml {
            Some(yaml) => front_matter::parse(yaml).unwrap_or_else(|e| {
                warnings.push(format!("{}: the front matter was ignored ({e})", note.rel));
                Map::new()
            }),
            None => Map::new(),
        };
        let generated = front.get("id").is_some_and(Value::is_string)
            && front.get("type_key").is_some_and(Value::is_string);

        let choice = choose_type(
            &mut types,
            note,
            &front,
            options,
            conn,
            &mut mapping_cache,
            &mut unknown_labels,
        )?;
        let type_idx = match choice {
            TypeChoice::Idx(i) => i,
            TypeChoice::Default(folder) => {
                if let Some(folder) = folder {
                    *unmapped.entry(folder).or_insert(0) += 1;
                }
                default_idx
            }
        };

        // Aliases: `aliases` (or the older `alias`).
        let aliases: Vec<String> = ["aliases", "alias"]
            .iter()
            .filter_map(|k| front.get(*k))
            .flat_map(front_matter::string_list)
            .collect();
        let name = if generated {
            aliases
                .first()
                .cloned()
                .unwrap_or_else(|| note.stem.clone())
        } else {
            note.stem.clone()
        };
        let summary = front
            .get("summary")
            .and_then(front_matter::scalar_text)
            .unwrap_or_default()
            .trim()
            .to_owned();

        let type_key = &types.list[type_idx].key;
        let is_event = type_key == "event";
        let is_map = type_key == "map";
        let (body, rels, involved, map_embed) = if generated {
            let s = strip_generated(split.body, &summary, is_event, is_map);
            (s.body, s.relationships, s.involved, s.map_embed)
        } else {
            (split.body.to_owned(), Vec::new(), Vec::new(), None)
        };

        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let created_at = parse_timestamp(front.get("created")).unwrap_or_else(|| now.clone());
        let updated_at = parse_timestamp(front.get("updated")).unwrap_or(now);
        let skip = options.skip_duplicates
            && !types.list[type_idx].key.starts_with("new:")
            && world
                .existing
                .contains_key(&(types.list[type_idx].key.clone(), lower_text(&name)));
        let id = if skip {
            world
                .existing
                .get(&(types.list[type_idx].key.clone(), lower_text(&name)))
                .cloned()
                .unwrap_or_else(|| Uuid::now_v7().to_string())
        } else {
            Uuid::now_v7().to_string()
        };
        notes.push(PlanNote {
            vault: note.clone(),
            id,
            name,
            type_idx,
            front,
            body,
            skip,
            aliases,
            summary,
            created_at,
            updated_at,
            relationship_lines: rels,
            involved_lines: involved,
            map_embed,
        });
    }
    if !unknown_labels.is_empty() {
        let shown: Vec<&str> = unknown_labels.iter().take(5).map(String::as_str).collect();
        warnings.push(format!(
            "Some notes have a type that matches no entity type ({}); they use the folder or the default type",
            shown.join(", ")
        ));
    }

    // ---- index (pass 1 result) ----
    let mut index = NoteIndex::default();
    let mut stems: HashMap<String, (String, String)> = HashMap::new();
    let mut names: HashMap<String, (String, String)> = HashMap::new();
    let mut alias_map: HashMap<String, (String, String)> = HashMap::new();
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();
    for note in &notes {
        if insert_shortest(
            &mut stems,
            lower_text(&note.vault.stem),
            &note.vault.rel,
            &note.id,
        ) {
            ambiguous.insert(note.vault.stem.clone());
        }
        insert_shortest(
            &mut names,
            lower_text(&note.name),
            &note.vault.rel,
            &note.id,
        );
        for alias in &note.aliases {
            insert_shortest(&mut alias_map, lower_text(alias), &note.vault.rel, &note.id);
        }
        index.names.insert(note.id.clone(), note.name.clone());
        index.type_of.insert(note.id.clone(), note.type_idx);
    }
    let unwrap = |m: HashMap<String, (String, String)>| -> HashMap<String, String> {
        m.into_iter().map(|(k, (_, id))| (k, id)).collect()
    };
    index.by_stem = unwrap(stems);
    index.by_name = unwrap(names);
    index.by_alias = unwrap(alias_map);
    if !ambiguous.is_empty() {
        let shown: Vec<&str> = ambiguous.iter().take(5).map(String::as_str).collect();
        warnings.push(format!(
            "Several notes share a name ({}); links go to the one with the shortest path",
            shown.join(", ")
        ));
    }

    Ok(Plan {
        notes,
        results: Vec::new(),
        placeholders: Vec::new(),
        types,
        index,
        images: ImageIndex::new(vault.images.clone()),
        unmapped,
        tag_names: BTreeSet::new(),
        orbis_export: vault.orbis_export,
        warnings,
    })
}

// ---- pass 2: converting one note ----------------------------------------------

struct Ctx<'a> {
    index: &'a NoteIndex,
    images: &'a ImageIndex,
    types: &'a Types,
    calendar: &'a CalendarDefinition,
    options: &'a ImportOptions,
}

/// The converter's view of the vault for one note.
struct NoteHooks<'a> {
    ctx: &'a Ctx<'a>,
    note_dir: String,
    found: Vec<PlanImage>,
    unresolved: Vec<String>,
    warnings: Vec<String>,
}

fn file_name_of(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

impl Hooks for NoteHooks<'_> {
    fn resolve_note(&mut self, target: &str) -> Option<String> {
        let found = self.ctx.index.resolve(target);
        if found.is_none()
            && !self
                .unresolved
                .iter()
                .any(|u| lower_text(u) == lower_text(target))
        {
            self.unresolved.push(target.trim().to_owned());
        }
        found
    }

    fn image(&mut self, src: &str, alt: &str) -> ImageAction {
        let lower = src.to_lowercase();
        if lower.starts_with("data:") {
            return ImageAction::Text("[image]".into());
        }
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return ImageAction::Text(src.to_owned());
        }
        let placeholder = format!("[{}]", file_name_of(src));
        if !self.ctx.options.import_images {
            return ImageAction::Text(placeholder);
        }
        let Some(image) = self.ctx.images.find(&self.note_dir, src) else {
            self.warnings.push(format!("Image not found: {src}"));
            return ImageAction::Text(placeholder);
        };
        let supported = ["png", "jpg", "jpeg", "gif", "webp"]
            .iter()
            .any(|e| image.rel.to_lowercase().ends_with(&format!(".{e}")));
        if !supported {
            self.warnings.push(format!(
                "Image {} is not a png, jpg, webp or gif and was not imported",
                image.rel
            ));
            return ImageAction::Text(placeholder);
        }
        if image.size > MAX_IMAGE_BYTES {
            self.warnings.push(format!(
                "Image {} is larger than {} MB and was not imported",
                image.rel,
                MAX_IMAGE_BYTES / 1024 / 1024
            ));
            return ImageAction::Text(placeholder);
        }
        if !self.found.iter().any(|f| f.rel == image.rel) {
            self.found.push(PlanImage {
                rel: image.rel.clone(),
                caption: alt.trim().to_owned(),
                portrait: false,
            });
        }
        ImageAction::Drop
    }
}

/// Plain text of a TipTap document, for the search index. Block elements are
/// separated by blank lines; a mention is the entity's name.
fn plain_text_of(node: &Value, names: &HashMap<String, String>) -> String {
    fn walk(node: &Value, names: &HashMap<String, String>, out: &mut String) {
        let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
        if let Some(text) = node.get("text").and_then(Value::as_str) {
            out.push_str(text);
            return;
        }
        match kind {
            "hardBreak" => out.push('\n'),
            "mention" => {
                let id = node
                    .pointer("/attrs/id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                out.push_str(names.get(id).map_or("", String::as_str));
            }
            _ => {
                let children = node.get("content").and_then(Value::as_array);
                let block_children = children.is_some_and(|c| {
                    c.iter().any(|n| {
                        !matches!(
                            n.get("type").and_then(Value::as_str),
                            Some("text" | "hardBreak" | "mention" | "image")
                        )
                    })
                });
                for (i, child) in children.into_iter().flatten().enumerate() {
                    if i > 0 && block_children {
                        out.push_str("\n\n");
                    }
                    walk(child, names, out);
                }
            }
        }
    }
    let mut out = String::new();
    walk(node, names, &mut out);
    out
}

fn doc_is_empty(doc: &Value) -> bool {
    doc.get("content")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
}

/// Value in front matter -> the entity id it refers to. Accepts `[[Name]]`,
/// `[[Name|Alias]]` and plain names.
fn resolve_reference(ctx: &Ctx, text: &str, unresolved: &mut Vec<String>) -> Option<String> {
    let name = match first_wikilink(text) {
        Some((_, target)) => target,
        None => text.trim().to_owned(),
    };
    let found = ctx.index.resolve(&name);
    if found.is_none()
        && !name.is_empty()
        && !unresolved
            .iter()
            .any(|u| lower_text(u) == lower_text(&name))
    {
        unresolved.push(name);
    }
    found
}

fn option_match(field: &FieldDef, text: &str) -> Option<String> {
    field
        .options
        .iter()
        .find(|o| o.to_lowercase() == text.trim().to_lowercase())
        .cloned()
}

fn bool_from(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "yes" | "y" | "1" => Some(true),
            "false" | "no" | "n" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Converts one front matter value to the JSON a field stores. `Ok(None)`
/// means "nothing to store"; `Err` carries a warning for a value that does
/// not fit at all. A list that only partly fits keeps the good items and
/// adds a warning to `warnings`.
fn convert_field(
    ctx: &Ctx,
    field: &FieldDef,
    value: &Value,
    day: Option<&Value>,
    unresolved: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<Option<Value>, String> {
    let bad = |why: &str| format!("{}: {why}", field.label);
    match &field.kind {
        FieldKind::Text | FieldKind::LongText => Ok(front_matter::scalar_text(value)
            .filter(|s| !s.is_empty())
            .map(Value::String)),
        FieldKind::Url => match front_matter::scalar_text(value) {
            Some(s) if !s.is_empty() => {
                let ok = ["http://", "https://", "mailto:"]
                    .iter()
                    .any(|p| s.starts_with(p));
                if ok {
                    Ok(Some(Value::String(s)))
                } else {
                    Err(bad(&format!(
                        "\"{s}\" is not a link starting with http://, https:// or mailto:"
                    )))
                }
            }
            _ => Ok(None),
        },
        FieldKind::Number => match value {
            Value::Number(_) => Ok(Some(value.clone())),
            Value::String(s) if s.trim().is_empty() => Ok(None),
            Value::String(s) => s
                .trim()
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(|n| Some(Value::Number(n)))
                .ok_or_else(|| bad(&format!("\"{s}\" is not a number"))),
            _ => Err(bad("expected a number")),
        },
        FieldKind::Boolean => bool_from(value)
            .map(|b| Some(Value::Bool(b)))
            .ok_or_else(|| bad("expected true or false")),
        FieldKind::Select => match front_matter::scalar_text(value) {
            Some(s) if !s.trim().is_empty() => option_match(field, &s)
                .map(|o| Some(Value::String(o)))
                .ok_or_else(|| bad(&format!("\"{s}\" is not one of the options"))),
            _ => Ok(None),
        },
        FieldKind::MultiSelect => {
            let mut chosen: Vec<Value> = Vec::new();
            let mut dropped: Vec<String> = Vec::new();
            for item in front_matter::string_list(value) {
                match option_match(field, &item) {
                    Some(o) if !chosen.contains(&Value::String(o.clone())) => {
                        chosen.push(Value::String(o));
                    }
                    Some(_) => {}
                    None => dropped.push(item),
                }
            }
            if !dropped.is_empty() {
                warnings.push(bad(&format!(
                    "{} not an option and left out",
                    dropped.join(", ")
                )));
            }
            Ok((!chosen.is_empty()).then_some(Value::Array(chosen)))
        }
        FieldKind::Date => {
            if let Some(d) = day.and_then(Value::as_i64) {
                return calendar::check_day(d)
                    .map(|()| Some(json!(d)))
                    .map_err(|e| bad(&e.to_string()));
            }
            match front_matter::scalar_text(value) {
                Some(s) if !s.trim().is_empty() => calendar::parse_date(ctx.calendar, &s)
                    .map(|d| Some(json!(d)))
                    .ok_or_else(|| bad(&format!("could not read the date \"{s}\""))),
                _ => Ok(None),
            }
        }
        FieldKind::EntityRef {
            allowed_types,
            multiple,
        } => {
            let texts: Vec<String> = match value {
                Value::Array(items) => items.iter().filter_map(front_matter::scalar_text).collect(),
                other => front_matter::scalar_text(other).into_iter().collect(),
            };
            let mut ids: Vec<String> = Vec::new();
            for text in texts.iter().filter(|t| !t.trim().is_empty()) {
                let Some(id) = resolve_reference(ctx, text, unresolved) else {
                    continue;
                };
                let allowed = allowed_types.is_empty()
                    || ctx
                        .index
                        .type_of
                        .get(&id)
                        .is_some_and(|t| allowed_types.contains(&ctx.types.list[*t].key));
                if allowed && !ids.contains(&id) {
                    ids.push(id);
                }
            }
            if ids.is_empty() {
                return if texts.iter().any(|t| !t.trim().is_empty()) {
                    Err(bad("no matching note was found for the reference"))
                } else {
                    Ok(None)
                };
            }
            if *multiple {
                Ok(Some(json!(ids)))
            } else {
                Ok(Some(json!(ids[0])))
            }
        }
    }
}

/// Front matter keys with a meaning of their own (not kept as properties).
const HANDLED_KEYS: [&str; 12] = [
    "id",
    "type",
    "type_key",
    "aliases",
    "alias",
    "summary",
    "tags",
    "tag",
    "cssclasses",
    "cssclass",
    "portrait",
    "images",
];

/// An unknown front matter value as something a property can hold.
fn preserve(value: &Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::Array(items) => {
            let texts: Vec<Value> = items
                .iter()
                .filter_map(|v| match v {
                    Value::Object(_) | Value::Array(_) => Some(Value::String(v.to_string())),
                    other => front_matter::scalar_text(other).map(Value::String),
                })
                .collect();
            Some(Value::Array(texts))
        }
        Value::Object(_) => Some(Value::String(value.to_string())),
        other => Some(other.clone()),
    }
}

/// Pass 2 for one note: converts the body and the front matter.
fn process_note(ctx: &Ctx, note: &PlanNote) -> NoteResult {
    let mut result = NoteResult::default();
    let type_def = &ctx.types.list[note.type_idx];
    let is_event = type_def.key == "event";

    // ---- the lore ----
    let note_dir = note
        .vault
        .folders
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("/");
    let mut hooks = NoteHooks {
        ctx,
        note_dir,
        found: Vec::new(),
        unresolved: Vec::new(),
        warnings: Vec::new(),
    };
    let converted = markdown_import::convert(&note.body, &mut hooks);
    let mut doc = converted.doc;
    downgrade_for_editor(&mut doc);
    if !doc_is_empty(&doc) {
        result.plain_text = plain_text_of(&doc, &ctx.index.names);
        result.doc = Some(doc);
    }
    let mut unresolved = hooks.unresolved;
    result.warnings.extend(hooks.warnings);
    let mut images = hooks.found;

    // ---- front matter: fields ----
    let mut consumed: HashSet<String> = HashSet::new();
    for field in &type_def.fields {
        let key = if RESERVED_KEYS.contains(&field.key.as_str()) {
            format!("field_{}", field.key)
        } else {
            field.key.clone()
        };
        let day_key = format!("{key}_day");
        consumed.insert(key.clone());
        consumed.insert(day_key.clone());
        let Some(value) = note.front.get(&key).or_else(|| note.front.get(&day_key)) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let day = note.front.get(&day_key);
        match convert_field(
            ctx,
            field,
            value,
            day,
            &mut unresolved,
            &mut result.warnings,
        ) {
            Ok(Some(v)) => {
                result.props.insert(field.key.clone(), v);
            }
            Ok(None) => {}
            Err(w) => result.warnings.push(w),
        }
    }

    // ---- event dates ----
    if is_event {
        let day_of = |day_key: &str, text_key: &str| -> Option<i64> {
            note.front.get(day_key).and_then(Value::as_i64).or_else(|| {
                note.front
                    .get(text_key)
                    .and_then(front_matter::scalar_text)
                    .and_then(|t| calendar::parse_date(ctx.calendar, &t))
            })
        };
        let start = day_of("start_day", "start");
        let end = day_of("end_day", "end");
        let importance = note
            .front
            .get("importance")
            .and_then(Value::as_i64)
            .map_or(1, |i| i.clamp(1, 5) as i32);
        if start.is_none() {
            result
                .warnings
                .push("The event has no readable start date; it starts on day 0".into());
        }
        let start = start.unwrap_or(0);
        let end = end.filter(|e| *e >= start);
        if calendar::check_day(start).is_ok() && end.is_none_or(|e| calendar::check_day(e).is_ok())
        {
            result.event = Some((start, end, importance));
        }
        for k in ["start", "start_day", "end", "end_day", "importance"] {
            consumed.insert(k.to_owned());
        }
    }

    // ---- unknown keys are kept ----
    for (key, value) in &note.front {
        let lower = key.to_lowercase();
        let timestamp = matches!(lower.as_str(), "created" | "updated")
            && parse_timestamp(Some(value)).is_some();
        if consumed.contains(key)
            || HANDLED_KEYS.contains(&lower.as_str())
            || timestamp
            || key.trim().is_empty()
            || key.chars().count() > 100
        {
            continue;
        }
        if let Some(kept) = preserve(value) {
            result.props.insert(key.clone(), kept);
        }
    }

    // ---- tags ----
    let mut tag_names: Vec<String> = Vec::new();
    let mut add_tag = |raw: &str, warnings: &mut Vec<String>| {
        let cleaned = raw.trim().trim_start_matches('#');
        match tags::normalize_name(cleaned) {
            Ok(name) => {
                if !tag_names
                    .iter()
                    .any(|t| t.to_lowercase() == name.to_lowercase())
                {
                    tag_names.push(name);
                }
            }
            Err(e) => warnings.push(format!("Tag ignored: {e}")),
        }
    };
    for key in ["tags", "tag"] {
        if let Some(v) = note.front.get(key) {
            for t in front_matter::string_list(v) {
                add_tag(&t, &mut result.warnings);
            }
        }
    }
    for t in &converted.tags {
        add_tag(t, &mut result.warnings);
    }
    if tag_names.len() > tags::MAX_TAGS_PER_ENTITY {
        result.warnings.push(format!(
            "Only the first {} tags were kept",
            tags::MAX_TAGS_PER_ENTITY
        ));
        tag_names.truncate(tags::MAX_TAGS_PER_ENTITY);
    }
    result.tags = tag_names;

    // ---- gallery (Orbis export front matter) ----
    let mut front_images: Vec<PlanImage> = Vec::new();
    if let Some(Value::Array(items)) = note.front.get("images") {
        for item in items {
            let (file, caption) = match item {
                Value::String(s) => (s.clone(), String::new()),
                Value::Object(o) => (
                    o.get("file")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    o.get("caption")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                ),
                _ => continue,
            };
            if file.is_empty() || !ctx.options.import_images {
                continue;
            }
            match ctx.images.find("", &file) {
                Some(found) if found.size <= MAX_IMAGE_BYTES => front_images.push(PlanImage {
                    rel: found.rel.clone(),
                    caption,
                    portrait: false,
                }),
                Some(_) => result
                    .warnings
                    .push(format!("Image {file} is too large and was not imported")),
                None => result.warnings.push(format!("Image not found: {file}")),
            }
        }
    }
    if let Some(portrait) = note.front.get("portrait").and_then(Value::as_str)
        && let Some(found) = ctx.images.find("", portrait)
        && let Some(image) = front_images.iter_mut().find(|i| i.rel == found.rel)
    {
        image.portrait = true;
    }
    for image in images.drain(..) {
        if !front_images.iter().any(|i| i.rel == image.rel) {
            front_images.push(image);
        }
    }
    if front_images.len() > MAX_NOTE_IMAGES {
        result.warnings.push(format!(
            "Only the first {MAX_NOTE_IMAGES} images of the note were imported"
        ));
        front_images.truncate(MAX_NOTE_IMAGES);
    }
    result.images = front_images;

    // ---- a map's picture ----
    if type_def.key == "map"
        && ctx.options.import_images
        && let Some(embed) = &note.map_embed
    {
        result.map_background = ctx.images.find("", embed).map(|i| i.rel.clone());
    }

    // ---- relationships and event participants (Orbis export) ----
    for (label, target) in &note.relationship_lines {
        match ctx.index.resolve(target) {
            Some(id) => result.rels.push((label.clone(), id)),
            None => result.warnings.push(format!(
                "Relationship \"{label} {target}\": no matching note"
            )),
        }
    }
    if is_event {
        for target in &note.involved_lines {
            match ctx.index.resolve(target) {
                Some(id) => result.involved.push(id),
                None => result
                    .warnings
                    .push(format!("Involved \"{target}\": no matching note")),
            }
        }
    }
    result.unresolved = unresolved;
    result
}

/// The editor knows fewer nodes than the converter can produce; make the
/// document something it shows completely. Task lists become bullet lists
/// whose items start with a check box character, highlight marks are
/// removed, and leftover image nodes become their text.
fn downgrade_for_editor(node: &mut Value) {
    if let Some(marks) = node.get_mut("marks").and_then(Value::as_array_mut) {
        marks.retain(|m| m.get("type").and_then(Value::as_str) != Some("highlight"));
        if marks.is_empty()
            && let Some(obj) = node.as_object_mut()
        {
            obj.remove("marks");
        }
    }
    let kind = node
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    match kind.as_str() {
        "taskList" => node["type"] = json!("bulletList"),
        "taskItem" => {
            let checked = node
                .pointer("/attrs/checked")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            node["type"] = json!("listItem");
            if let Some(obj) = node.as_object_mut() {
                obj.remove("attrs");
            }
            let prefix = if checked { "\u{2611} " } else { "\u{2610} " };
            if let Some(first) = node.pointer_mut("/content/0/content")
                && let Some(items) = first.as_array_mut()
            {
                items.insert(0, json!({ "type": "text", "text": prefix }));
            } else if let Some(first) = node.pointer_mut("/content/0")
                && first.get("type").and_then(Value::as_str) == Some("paragraph")
            {
                first["content"] = json!([{ "type": "text", "text": prefix }]);
            }
        }
        "image" => {
            let src = node
                .pointer("/attrs/src")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            *node = json!({ "type": "text", "text": format!("[{}]", file_name_of(&src)) });
            return;
        }
        _ => {}
    }
    if let Some(children) = node.get_mut("content").and_then(Value::as_array_mut) {
        for child in children.iter_mut() {
            downgrade_for_editor(child);
        }
        // A block image became text: wrap loose text in a paragraph.
        let is_block_container = matches!(kind.as_str(), "doc" | "blockquote" | "listItem");
        if is_block_container {
            for child in children.iter_mut() {
                if child.get("type").and_then(Value::as_str) == Some("text") {
                    let text = child.clone();
                    *child = json!({ "type": "paragraph", "content": [text] });
                }
            }
        }
    }
}

/// Runs pass 2 over every note, adding placeholder notes for unresolved
/// links when asked.
fn convert_all(
    plan: &mut Plan,
    world: &WorldInfo,
    options: &ImportOptions,
    on_progress: ProgressFn,
) -> AppResult<()> {
    let total = plan.notes.len();
    let throttle = Throttle::new(total);
    let mut results: Vec<NoteResult> = Vec::with_capacity(total);
    {
        let ctx = Ctx {
            index: &plan.index,
            images: &plan.images,
            types: &plan.types,
            calendar: &world.calendar,
            options,
        };
        for (n, note) in plan.notes.iter().enumerate() {
            throttle.tick(n, total, "converting", on_progress)?;
            results.push(process_note(&ctx, note));
        }
    }

    if options.create_missing_links_as_notes {
        // Names that no note answers to become empty Note entities, then the
        // notes that mentioned them are converted again.
        let note_idx = plan
            .types
            .by_key("note")
            .ok_or_else(|| AppError::Internal("the note type is missing".into()))?;
        let mut added = false;
        for r in &results {
            for name in &r.unresolved {
                let clean = name.split_whitespace().collect::<Vec<_>>().join(" ");
                let key = lower_text(&clean);
                if key.is_empty()
                    || clean.chars().count() > MAX_PLACEHOLDER_CHARS
                    || plan.index.resolve(&clean).is_some()
                {
                    continue;
                }
                let id = Uuid::now_v7().to_string();
                plan.index.placeholders.insert(key, id.clone());
                plan.index.names.insert(id.clone(), clean.clone());
                plan.index.type_of.insert(id.clone(), note_idx);
                plan.placeholders.push((id, clean));
                added = true;
            }
        }
        if added {
            let ctx = Ctx {
                index: &plan.index,
                images: &plan.images,
                types: &plan.types,
                calendar: &world.calendar,
                options,
            };
            for (n, note) in plan.notes.iter().enumerate() {
                if !results[n].unresolved.is_empty() {
                    results[n] = process_note(&ctx, note);
                }
            }
        }
    }

    for (note, r) in plan.notes.iter().zip(&results) {
        for w in &r.warnings {
            plan.warnings.push(format!("{}: {w}", note.vault.rel));
        }
        if !note.skip {
            plan.tag_names
                .extend(r.tags.iter().map(|t| t.to_lowercase()));
        }
    }
    plan.results = results;
    Ok(())
}

// ---- preview -------------------------------------------------------------------

fn open_read_only(world_dir: &Path) -> AppResult<Connection> {
    Ok(Connection::open_with_flags(
        world_dir.join(DB_FILE),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}

/// What importing `vault_dir` would do. Reads the vault and the world, writes
/// nothing. Blocking: call it from a background thread.
pub fn preview(
    world_dir: &Path,
    vault_dir: &Path,
    options: &ImportOptions,
) -> AppResult<ImportPreview> {
    let conn = open_read_only(world_dir)?;
    let world = read_world(&conn)?;
    let vault = vault_scan::scan(vault_dir)?;
    let mut ignore = |_: u32, _: u32, _: &str| Ok(());
    let mut plan = build_plan(
        &vault,
        &world,
        &conn,
        options,
        MAX_WARNINGS_PREVIEW,
        &mut ignore,
    )?;
    convert_all(&mut plan, &world, options, &mut ignore)?;

    let mut counts: BTreeMap<usize, u32> = BTreeMap::new();
    let mut skipped = 0;
    let mut images: BTreeSet<&str> = BTreeSet::new();
    let mut relationships = 0;
    for (note, result) in plan.notes.iter().zip(&plan.results) {
        if note.skip {
            skipped += 1;
            continue;
        }
        *counts.entry(note.type_idx).or_insert(0) += 1;
        images.extend(result.images.iter().map(|i| i.rel.as_str()));
        images.extend(result.map_background.as_deref());
        relationships += result.rels.len() as u32;
    }
    let note_idx = plan.types.by_key("note");
    if let Some(i) = note_idx
        && !plan.placeholders.is_empty()
    {
        *counts.entry(i).or_insert(0) += plan.placeholders.len() as u32;
    }
    let by_type: Vec<TypeCount> = counts
        .iter()
        .map(|(i, count)| {
            let t = &plan.types.list[*i];
            TypeCount {
                type_key: t.key.clone(),
                label: t.label.clone(),
                count: *count,
                new_type: t.spec.is_some(),
            }
        })
        .collect();
    let mapped: HashSet<String> = options
        .folder_type_map
        .iter()
        .map(|m| m.folder.to_lowercase())
        .collect();
    let unresolved_types = plan
        .unmapped
        .iter()
        .filter(|(folder, _)| !mapped.contains(&folder.to_lowercase()))
        .map(|(folder, count)| UnmappedFolder {
            folder: folder.clone(),
            count: *count,
        })
        .collect();
    Ok(ImportPreview {
        notes: by_type.iter().map(|t| t.count).sum(),
        by_type,
        unresolved_types,
        images: images.len() as u32,
        tags: plan.tag_names.len() as u32,
        skipped,
        relationships,
        orbis_export: plan.orbis_export,
        warnings: plan.warnings.list,
        more_warnings: plan.warnings.more,
    })
}

// ---- the import ------------------------------------------------------------------

/// Imports `vault_dir` into the world in `world_dir`. Blocking: call it from
/// a background thread. Everything is written in one transaction; on any
/// error nothing is changed (copied image files are removed again).
pub fn import(
    world_dir: &Path,
    vault_dir: &Path,
    options: &ImportOptions,
    on_progress: ProgressFn,
) -> AppResult<ImportSummary> {
    let conn = db::open(&world_dir.join(DB_FILE))?;
    let world = read_world(&conn)?;
    on_progress(0, 0, "scanning")?;
    let vault = vault_scan::scan(vault_dir)?;
    let mut plan = build_plan(
        &vault,
        &world,
        &conn,
        options,
        MAX_WARNINGS_IMPORT,
        on_progress,
    )?;
    convert_all(&mut plan, &world, options, on_progress)?;

    let assets_dir = world_dir.join(ASSETS_DIR);
    // Files copied into assets/ so far; removed again if the import fails.
    let mut copied: Vec<(String, String)> = Vec::new();
    let result = write_all(&conn, &mut plan, &assets_dir, &mut copied, on_progress);
    match result {
        Ok(mut summary) => {
            // Thumbnails come last, on one background thread.
            assets::spawn_thumbnails(assets_dir, copied);
            summary.warnings = std::mem::take(&mut plan.warnings.list);
            summary.more_warnings = plan.warnings.more;
            Ok(summary)
        }
        Err(err) => {
            for (id, mime) in &copied {
                assets::delete_files(&assets_dir, id, mime);
            }
            Err(err)
        }
    }
}

/// Provisional type keys -> real keys, after the new types were created.
fn final_key(keys: &HashMap<String, String>, key: &str) -> String {
    keys.get(key).cloned().unwrap_or_else(|| key.to_owned())
}

fn remap_kind(kind: &FieldKind, keys: &HashMap<String, String>) -> FieldKind {
    match kind {
        FieldKind::EntityRef {
            allowed_types,
            multiple,
        } => FieldKind::EntityRef {
            allowed_types: allowed_types.iter().map(|t| final_key(keys, t)).collect(),
            multiple: *multiple,
        },
        other => other.clone(),
    }
}

fn to_inputs(fields: &[FieldDef], keys: &HashMap<String, String>) -> Vec<FieldInput> {
    fields
        .iter()
        .map(|f| FieldInput {
            key: Some(f.key.clone()),
            label: f.label.clone(),
            kind: remap_kind(&f.kind, keys),
            options: f.options.clone(),
        })
        .collect()
}

/// Creates the custom types and extra fields the import needs; returns the
/// provisional -> real key map.
fn create_types(conn: &Connection, plan: &Plan) -> AppResult<(HashMap<String, String>, u32)> {
    let mut keys: HashMap<String, String> = HashMap::new();
    let mut created = 0;
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    for t in plan.types.list.iter().filter(|t| t.spec.is_some()) {
        let Some(spec) = &t.spec else { continue };
        let def = entity_types::prepare_new(
            conn,
            &spec.label,
            &spec.plural,
            &spec.icon,
            &spec.color,
            Vec::new(),
        )?;
        entity_types::insert_prepared(conn, &def)?;
        keys.insert(t.key.clone(), def.key);
        created += 1;
    }
    // Fields of types that came from an export (they may refer to each
    // other, so this waits until every new type has its real key).
    for t in plan
        .types
        .list
        .iter()
        .filter(|t| t.manifest_fields.is_some())
    {
        let (Some(fields), Some(real)) = (&t.manifest_fields, keys.get(&t.key)) else {
            continue;
        };
        let known = entity_types::known_type_keys(&entity_types::merged(conn)?);
        let normalized =
            entity_types::normalize_fields(to_inputs(fields, &keys), &HashSet::new(), &known)?;
        if let Some(mut def) = db::entity_types::get_custom(conn, real)? {
            def.fields = normalized;
            db::entity_types::update_custom(conn, &def, &now)?;
        }
    }
    for (idx, extra) in &plan.types.template_additions {
        let key = &plan.types.list[*idx].key;
        let mut all = db::entity_types::get_template(conn, key)?;
        all.extend(extra.iter().cloned());
        let known = entity_types::known_type_keys(&entity_types::merged(conn)?);
        let normalized = entity_types::normalize_fields(
            to_inputs(&all, &keys),
            &entity_types::builtin_field_keys(key),
            &known,
        )?;
        db::entity_types::set_template(conn, key, &normalized)?;
    }
    Ok((keys, created))
}

fn write_all(
    conn: &Connection,
    plan: &mut Plan,
    assets_dir: &Path,
    copied: &mut Vec<(String, String)>,
    on_progress: ProgressFn,
) -> AppResult<ImportSummary> {
    let mut summary = ImportSummary::default();
    let total = plan.notes.len() * 2 + plan.placeholders.len() + 2;
    let throttle = Throttle::new(total);
    let mut done = 0usize;
    on_progress(0, total as u32, "writing")?;

    let tx = conn.unchecked_transaction()?;
    let (keys, types_created) = create_types(&tx, plan)?;
    summary.types_created = types_created;

    // ---- images: each file once ----
    let mut wanted: Vec<String> = Vec::new();
    for r in &plan.results {
        for rel in r
            .images
            .iter()
            .map(|i| &i.rel)
            .chain(r.map_background.as_ref())
        {
            if !wanted.contains(rel) {
                wanted.push(rel.clone());
            }
        }
    }
    let mut asset_of: HashMap<String, String> = HashMap::new();
    for rel in &wanted {
        let Some(image) = plan.images.images().iter().find(|i| &i.rel == rel) else {
            continue;
        };
        match assets::copy_into_assets(&image.path, assets_dir) {
            Ok(asset) => {
                copied.push((asset.id.clone(), asset.mime.clone()));
                db::assets::insert(&tx, &asset)?;
                asset_of.insert(rel.clone(), asset.id);
                summary.images += 1;
            }
            Err(AppError::Validation(_)) => plan
                .warnings
                .push(format!("Image {rel} is not a valid png, jpg, webp or gif")),
            Err(e) => return Err(e),
        }
    }

    // ---- entities ----
    let default_calendar = calendar::get_default(&tx)?;
    let mut tag_cache = tags::load_cache(&tx)?;
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let mut written: Vec<usize> = Vec::new();
    for (n, note) in plan.notes.iter().enumerate() {
        done += 1;
        throttle.tick(done, total, "writing", on_progress)?;
        if note.skip {
            summary.skipped += 1;
            continue;
        }
        let type_key = final_key(&keys, &plan.types.list[note.type_idx].key);
        let result = &plan.results[n];
        let entity = Entity {
            id: note.id.clone(),
            entity_type: type_key.clone(),
            name: note.name.trim().to_owned(),
            summary: note.summary.clone(),
            properties: Value::Object(result.props.clone()),
            created_at: note.created_at.clone(),
            updated_at: note.updated_at.clone(),
            favorite: false,
            tags: Vec::new(),
            portrait: None,
        };
        db::entities::insert(&tx, &entity)?;
        if type_key == "event" {
            let (start, end, importance) = result.event.unwrap_or((0, None, 1));
            db::events::insert(
                &tx,
                &entity.id,
                &default_calendar.id,
                start,
                end,
                importance,
            )?;
        }
        if type_key == "map" {
            maps::create_for_entity(&tx, &entity.id)?;
            if let Some(asset) = result.map_background.as_ref().and_then(|r| asset_of.get(r)) {
                maps::set_background(&tx, assets_dir, &entity.id, asset)?;
            }
        }
        written.push(n);
        summary.created += 1;
    }
    for (id, name) in &plan.placeholders {
        done += 1;
        throttle.tick(done, total, "writing", on_progress)?;
        let entity = Entity {
            id: id.clone(),
            entity_type: "note".into(),
            name: name.clone(),
            summary: String::new(),
            properties: json!({}),
            created_at: now.clone(),
            updated_at: now.clone(),
            favorite: false,
            tags: Vec::new(),
            portrait: None,
        };
        db::entities::insert(&tx, &entity)?;
        search::reindex_entity(&tx, id)?;
        summary.created += 1;
    }

    // ---- everything that points at other entities ----
    let catalog_kinds = catalog::all();
    let mut seen_relationships: HashSet<(String, String, String)> = HashSet::new();
    for n in written {
        let note = &plan.notes[n];
        let result = &plan.results[n];
        done += 1;
        throttle.tick(done, total, "writing", on_progress)?;
        let id = &note.id;
        let type_key = final_key(&keys, &plan.types.list[note.type_idx].key);

        if let Some(doc) = &result.doc {
            let document = Document {
                entity_id: id.clone(),
                content: doc.to_string(),
                plain_text: result.plain_text.clone(),
                updated_at: note.updated_at.clone(),
            };
            db::documents::upsert(&tx, &document)?;
            links::sync_mentions(&tx, id, doc)?;
        }
        let defs = &plan.types.list[note.type_idx].fields;
        properties::sync_field_refs(&tx, id, defs, &Value::Object(result.props.clone()))?;
        tags::apply_entity_tags(&tx, id, &result.tags, &mut tag_cache)?;

        for image in &result.images {
            let Some(asset) = asset_of.get(&image.rel) else {
                continue;
            };
            let added = gallery::append_asset(&tx, id, asset, &image.caption)?;
            if image.portrait {
                db::gallery::set_portrait(&tx, id, Some(&added.id))?;
            }
        }

        for (label, other) in &result.rels {
            let (kind, source, target, text) = match relationship_kind(&catalog_kinds, label) {
                Some((kind, true)) => (kind, id.clone(), other.clone(), None),
                Some((kind, false)) => (kind, other.clone(), id.clone(), None),
                None => (
                    catalog::KIND_CUSTOM.to_owned(),
                    id.clone(),
                    other.clone(),
                    Some(label.clone()),
                ),
            };
            let symmetric = catalog_kinds.iter().any(|k| k.key == kind && k.symmetric);
            let key = if symmetric || text.is_some() {
                // The same fact is written in both notes: count it once.
                let (a, b) = if source < target {
                    (source.clone(), target.clone())
                } else {
                    (target.clone(), source.clone())
                };
                (
                    a,
                    b,
                    format!("{kind}:{}", text.as_deref().unwrap_or("").to_lowercase()),
                )
            } else {
                (source.clone(), target.clone(), kind.clone())
            };
            if !seen_relationships.insert(key) {
                continue;
            }
            match relationships::create(&tx, &source, &target, &kind, text) {
                Ok(_) => summary.relationships += 1,
                Err(AppError::Validation(m)) => plan.warnings.push(format!(
                    "{}: relationship \"{label}\" skipped ({m})",
                    note.vault.rel
                )),
                Err(e) => return Err(e),
            }
        }
        if type_key == "event" {
            for who in &result.involved {
                match events::add_involved(&tx, id, who) {
                    Ok(()) => {}
                    Err(AppError::Validation(m) | AppError::NotFound(m)) => plan
                        .warnings
                        .push(format!("{}: participant skipped ({m})", note.vault.rel)),
                    Err(e) => return Err(e),
                }
            }
        }
        search::reindex_entity(&tx, id)?;
    }
    on_progress(done as u32, total as u32, "finishing")?;
    tx.commit()?;
    Ok(summary)
}

/// The catalog kind a relationship label belongs to, and whether the note's
/// entity is the SOURCE (`true`: "this is <forward label> other") or the
/// target. `None` for text that matches no kind (a custom relationship).
fn relationship_kind(
    kinds: &[catalog::RelationshipKindDef],
    label: &str,
) -> Option<(String, bool)> {
    let wanted = label.trim().to_lowercase();
    for k in kinds.iter().filter(|k| k.key != catalog::KIND_CUSTOM) {
        if k.forward_label.to_lowercase() == wanted {
            return Some((k.key.clone(), true));
        }
        if k.inverse_label.to_lowercase() == wanted {
            return Some((k.key.clone(), false));
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::export::MarkdownExportOptions;
    use crate::services::{documents, entities, markdown_export, sample_world, world};
    use crate::state::WorldHandle;
    use std::fs;

    fn new_world(parent: &Path, name: &str) -> WorldHandle {
        world::create_world(&parent.join(name), name).unwrap()
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn png(root: &Path, rel: &str, shade: u8) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::from_pixel(6, 4, image::Rgb([shade, 40, 90]))
            .save(path)
            .unwrap();
    }

    fn options() -> ImportOptions {
        ImportOptions {
            default_type: "note".into(),
            folder_type_map: Vec::new(),
            create_missing_links_as_notes: false,
            import_images: true,
            skip_duplicates: false,
        }
    }

    fn run(world: &WorldHandle, vault: &Path, o: &ImportOptions) -> AppResult<ImportSummary> {
        let mut quiet = |_: u32, _: u32, _: &str| Ok(());
        import(&world.path, vault, o, &mut quiet)
    }

    fn entity_named(conn: &Connection, name: &str) -> Entity {
        let all = db::entities::list(conn, None).unwrap();
        let found: Vec<_> = all.iter().filter(|e| e.name == name).collect();
        assert_eq!(found.len(), 1, "entities called {name}: {}", found.len());
        db::entities::get(conn, &found[0].id).unwrap().unwrap()
    }

    fn doc_text(conn: &Connection, id: &str) -> String {
        documents::get(conn, id).unwrap().plain_text
    }

    fn counts(conn: &Connection) -> (i64, i64, i64) {
        let n = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        (
            n("SELECT count(*) FROM entities"),
            n("SELECT count(*) FROM custom_entity_types"),
            n("SELECT count(*) FROM assets"),
        )
    }

    fn files_in(dir: &Path) -> usize {
        fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
    }

    /// A small hand-made Obsidian vault.
    fn make_vault(root: &Path) {
        write(
            root,
            "Characters/Queen Isolde.md",
            "---\nsummary: Young ruler\ntags:\n  - royalty\naliases:\n  - The Queen\nspecies: Human\nage: 34\nstatus: Alive\nmood: grim\nnested:\n  a: 1\n---\nRules [[Port Veyra]] from the coast #coast. See [[queen isolde|me]]!\n\n![[portrait.png]] %%a secret%%\n\n> [!note] Heads up\n> keep calm\n\n- [ ] todo\n- [x] done\n",
        );
        write(
            root,
            "Places/Cities/Port Veyra.md",
            "---\ntags: places/cities\n---\nThe capital. Home of [[The Queen]] and [[Missing Note]].\n\n## Relationships\n\nnot generated here\n",
        );
        write(
            root,
            "Random/Idea.md",
            "Just an idea with ![alt text](attachments/map%20one.png).",
        );
        png(root, "attachments/portrait.png", 200);
        png(root, "attachments/map one.png", 100);
        write(root, ".obsidian/app.json", "{}");
        write(root, ".trash/Gone.md", "deleted");
    }

    fn with_cities_mapped() -> ImportOptions {
        ImportOptions {
            folder_type_map: vec![FolderTypeMapping {
                folder: "Cities".into(),
                type_key: Some("location".into()),
                create_custom: None,
            }],
            ..options()
        }
    }

    #[test]
    fn preview_reports_types_folders_images_and_tags_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        make_vault(&vault);
        let w = new_world(dir.path(), "W");
        let before = counts(&w.conn);

        let p = preview(&w.path, &vault, &options()).unwrap();
        assert_eq!(p.notes, 3);
        assert!(!p.orbis_export);
        // Characters matches the Character type's plural; the other two
        // folders match nothing and go to the default type.
        let by: Vec<(&str, u32)> = p
            .by_type
            .iter()
            .map(|t| (t.type_key.as_str(), t.count))
            .collect();
        assert_eq!(by, vec![("character", 1), ("note", 2)]);
        assert!(p.by_type.iter().all(|t| !t.new_type));
        let folders: Vec<&str> = p
            .unresolved_types
            .iter()
            .map(|f| f.folder.as_str())
            .collect();
        assert_eq!(folders, vec!["Cities", "Random"]);
        assert_eq!(p.images, 2);
        assert_eq!(p.tags, 3, "royalty, coast and places/cities");
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        assert_eq!(counts(&w.conn), before, "a preview writes nothing");

        // Mapping a folder removes it from the unresolved list.
        let p = preview(&w.path, &vault, &with_cities_mapped()).unwrap();
        let folders: Vec<&str> = p
            .unresolved_types
            .iter()
            .map(|f| f.folder.as_str())
            .collect();
        assert_eq!(folders, vec!["Random"]);
        assert!(p.by_type.iter().any(|t| t.type_key == "location"));
    }

    #[test]
    fn imports_a_hand_made_vault() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        make_vault(&vault);
        let w = new_world(dir.path(), "W");

        let summary = run(&w, &vault, &with_cities_mapped()).unwrap();
        assert_eq!(summary.created, 3);
        assert_eq!(summary.images, 2);
        assert_eq!(summary.skipped, 0);

        // Types, summary, aliases and fields.
        let queen = entity_named(&w.conn, "Queen Isolde");
        assert_eq!(queen.entity_type, "character");
        assert_eq!(queen.summary, "Young ruler");
        assert_eq!(queen.properties["species"], "Human");
        assert_eq!(queen.properties["age"], 34);
        // "Alive" matches the option "alive" ignoring case.
        assert_eq!(queen.properties["status"], "alive");
        // Unknown keys are kept: scalars as they are, mappings as JSON text.
        assert_eq!(queen.properties["mood"], "grim");
        assert_eq!(queen.properties["nested"], "{\"a\":1}");
        // Tags from the front matter and from the text.
        assert_eq!(queen.tags, vec!["coast", "royalty"]);

        let port = entity_named(&w.conn, "Port Veyra");
        assert_eq!(port.entity_type, "location");
        assert_eq!(port.tags, vec!["places/cities"]);
        let idea = entity_named(&w.conn, "Idea");
        assert_eq!(idea.entity_type, "note");

        // Links: by file name, by alias and ignoring case; the alias text
        // is dropped, the unresolved link stays as text.
        let queen_doc = documents::get(&w.conn, &queen.id).unwrap();
        let mentions = crate::services::links::extract_mention_ids(
            &serde_json::from_str(&queen_doc.content).unwrap(),
        );
        assert_eq!(mentions, vec![port.id.clone(), queen.id.clone()]);
        assert!(
            queen_doc
                .plain_text
                .contains("Rules Port Veyra from the coast #coast")
        );
        // Comment removed, callout turned into a quote, tasks into bullets.
        assert!(!queen_doc.plain_text.contains("secret"));
        assert!(queen_doc.plain_text.contains("Heads up"));
        assert!(queen_doc.plain_text.contains("\u{2610} todo"));
        assert!(queen_doc.plain_text.contains("\u{2611} done"));
        let port_doc = documents::get(&w.conn, &port.id).unwrap();
        let port_mentions = crate::services::links::extract_mention_ids(
            &serde_json::from_str(&port_doc.content).unwrap(),
        );
        assert_eq!(port_mentions, vec![queen.id.clone()]);
        assert!(port_doc.plain_text.contains("Missing Note"));
        // Not an Orbis export: a "## Relationships" heading stays lore.
        assert!(port_doc.plain_text.contains("Relationships"));
        // Backlinks work.
        let back = crate::services::links::backlinks(&w.conn, &port.id, "Port Veyra").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].source_id, queen.id);

        // Images: the embed became a gallery image, the standard Markdown
        // image of "Idea" too; the originals are copied into assets/.
        let assets_dir = w.path.join(ASSETS_DIR);
        let q_images = gallery::list(&w.conn, &assets_dir, &queen.id).unwrap();
        assert_eq!(q_images.len(), 1);
        assert_eq!(q_images[0].caption, "");
        assert!(Path::new(&q_images[0].path).is_file());
        let i_images = gallery::list(&w.conn, &assets_dir, &idea.id).unwrap();
        assert_eq!(i_images.len(), 1);
        assert_eq!(i_images[0].caption, "alt text");
        assert!(!idea_text(&w.conn, &idea.id).contains("map one"));

        // Search finds the new content, including by tag.
        let hits = search::search(&w.conn, "tag:royalty rules", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_id, queen.id);
        assert_eq!(search::search(&w.conn, "capital", 10).unwrap().len(), 1);
        // Hidden folders were skipped.
        assert!(
            db::entities::list(&w.conn, None)
                .unwrap()
                .iter()
                .all(|e| e.name != "Gone")
        );
    }

    fn idea_text(conn: &Connection, id: &str) -> String {
        doc_text(conn, id)
    }

    #[test]
    fn images_can_be_left_out_and_missing_ones_warn() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        write(
            &vault,
            "A.md",
            "Pic ![[nowhere.png]] and ![[kept.png]] and ![[v.svg]]",
        );
        png(&vault, "kept.png", 1);
        write(&vault, "v.svg", "<svg/>");
        let w = new_world(dir.path(), "W");
        let s = run(&w, &vault, &options()).unwrap();
        assert_eq!(s.images, 1);
        assert!(
            s.warnings.iter().any(|m| m.contains("not found")),
            "{:?}",
            s.warnings
        );
        assert!(
            s.warnings.iter().any(|m| m.contains("v.svg")),
            "{:?}",
            s.warnings
        );
        let a = entity_named(&w.conn, "A");
        assert!(doc_text(&w.conn, &a.id).contains("[nowhere.png]"));

        let w2 = new_world(dir.path(), "W2");
        let no_images = ImportOptions {
            import_images: false,
            ..options()
        };
        let s = run(&w2, &vault, &no_images).unwrap();
        assert_eq!(s.images, 0);
        assert_eq!(files_in(&w2.path.join(ASSETS_DIR)), 0);
        let a = entity_named(&w2.conn, "A");
        assert!(doc_text(&w2.conn, &a.id).contains("[kept.png]"));
    }

    #[test]
    fn unresolved_links_can_become_placeholder_notes() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        write(
            &vault,
            "Home.md",
            "Go to [[Faraway]] or [[faraway|there]] or [[Home]].",
        );
        write(&vault, "Other.md", "Also [[Faraway#Section]].");
        let w = new_world(dir.path(), "W");
        let o = ImportOptions {
            create_missing_links_as_notes: true,
            ..options()
        };
        let s = run(&w, &vault, &o).unwrap();
        assert_eq!(s.created, 3, "two notes and one placeholder");
        let far = entity_named(&w.conn, "Faraway");
        assert_eq!(far.entity_type, "note");
        assert_eq!(documents::get(&w.conn, &far.id).unwrap().plain_text, "");
        let back = crate::services::links::backlinks(&w.conn, &far.id, "Faraway").unwrap();
        assert_eq!(back.len(), 2);
        // The preview counts it.
        let w2 = new_world(dir.path(), "W2");
        let p = preview(&w2.path, &vault, &o).unwrap();
        assert_eq!(p.notes, 3);
    }

    #[test]
    fn duplicates_can_be_skipped_and_links_still_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        write(&vault, "Idea.md", "text");
        write(&vault, "Other.md", "About [[Idea]].");
        let w = new_world(dir.path(), "W");
        let existing = entities::create(
            &w.conn,
            crate::models::entity::NewEntity {
                entity_type: "note".into(),
                name: "idea".into(),
            },
        )
        .unwrap();
        let o = ImportOptions {
            skip_duplicates: true,
            ..options()
        };
        let p = preview(&w.path, &vault, &o).unwrap();
        assert_eq!((p.notes, p.skipped), (1, 1));
        let s = run(&w, &vault, &o).unwrap();
        assert_eq!((s.created, s.skipped), (1, 1));
        let other = entity_named(&w.conn, "Other");
        let doc: Value =
            serde_json::from_str(&documents::get(&w.conn, &other.id).unwrap().content).unwrap();
        assert_eq!(
            crate::services::links::extract_mention_ids(&doc),
            vec![existing.id]
        );
        // Without the option every note is created.
        let w2 = new_world(dir.path(), "W2");
        entities::create(
            &w2.conn,
            crate::models::entity::NewEntity {
                entity_type: "note".into(),
                name: "Idea".into(),
            },
        )
        .unwrap();
        assert_eq!(run(&w2, &vault, &options()).unwrap().created, 2);
    }

    #[test]
    fn folders_can_create_custom_types() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        write(&vault, "Deities/Sol.md", "Sun god");
        write(&vault, "Deities/Luna.md", "Moon goddess");
        write(&vault, "Other/Thing.md", "x");
        let w = new_world(dir.path(), "W");
        let spec = NewTypeSpec {
            label: "Deity".into(),
            plural: "Deities".into(),
            icon: "sparkles".into(),
            color: "#a855f7".into(),
        };
        let o = ImportOptions {
            folder_type_map: vec![FolderTypeMapping {
                folder: "deities".into(),
                type_key: None,
                create_custom: Some(spec.clone()),
            }],
            ..options()
        };
        let p = preview(&w.path, &vault, &o).unwrap();
        let deity = p.by_type.iter().find(|t| t.label == "Deity").unwrap();
        assert!(deity.new_type);
        assert_eq!(deity.count, 2);
        let s = run(&w, &vault, &o).unwrap();
        assert_eq!(s.types_created, 1);
        let sol = entity_named(&w.conn, "Sol");
        assert_eq!(sol.entity_type, "custom-deity");
        assert!(entity_types::is_known(&w.conn, "custom-deity").unwrap());
        assert_eq!(entity_named(&w.conn, "Thing").entity_type, "note");

        // A mapping must name exactly one target, and names must be free.
        let both = ImportOptions {
            folder_type_map: vec![FolderTypeMapping {
                folder: "x".into(),
                type_key: Some("note".into()),
                create_custom: Some(spec.clone()),
            }],
            ..options()
        };
        assert!(matches!(
            preview(&w.path, &vault, &both),
            Err(AppError::Validation(_))
        ));
        // "Deity" now exists: a second import cannot create it again.
        let again = run(&new_world(dir.path(), "W2"), &vault, &o);
        assert!(again.is_ok());
        assert!(matches!(run(&w, &vault, &o), Err(AppError::Validation(_))));
        let bad_default = ImportOptions {
            default_type: "nope".into(),
            ..options()
        };
        assert!(matches!(
            preview(&w.path, &vault, &bad_default),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn front_matter_values_are_converted_per_field_kind() {
        use crate::models::entity_type::FieldInput;
        let dir = tempfile::tempdir().unwrap();
        let w = new_world(dir.path(), "W");
        let f = |label: &str, kind: FieldKind, options: &[&str]| FieldInput {
            key: None,
            label: label.into(),
            kind,
            options: options.iter().map(|o| (*o).to_owned()).collect(),
        };
        let thing = entity_types::create(
            &w.conn,
            "Thing",
            "Things",
            "box",
            "#112233",
            vec![
                f("Text", FieldKind::Text, &[]),
                f("Long", FieldKind::LongText, &[]),
                f("Num", FieldKind::Number, &[]),
                f("Flag", FieldKind::Boolean, &[]),
                f("Pick", FieldKind::Select, &["a", "b"]),
                f("Picks", FieldKind::MultiSelect, &["x", "y", "z"]),
                f("When", FieldKind::Date, &[]),
                f("Later", FieldKind::Date, &[]),
                f("Site", FieldKind::Url, &[]),
                f(
                    "Boss",
                    FieldKind::EntityRef {
                        allowed_types: vec!["character".into()],
                        multiple: false,
                    },
                    &[],
                ),
                f(
                    "Allies",
                    FieldKind::EntityRef {
                        allowed_types: vec![],
                        multiple: true,
                    },
                    &[],
                ),
            ],
        )
        .unwrap();
        let vault = dir.path().join("vault");
        write(
            &vault,
            "Things/Gizmo.md",
            &format!(
                "---\ntype_key: {}\ntext: 42\nlong: \"a\\nb\"\nnum: \"5.5\"\nflag: yes\npick: B\npicks: [X, z, nope]\nwhen: \"ignored text\"\nwhen_day: -400\nlater: \"15 Month 3, 1203 AF\"\nsite: ftp://bad\nboss: \"[[Ada]]\"\nallies:\n  - \"[[Ada]]\"\n  - Bob\n  - \"[[Stranger]]\"\n---\nbody\n",
                thing.key
            ),
        );
        write(&vault, "Characters/Ada.md", "x");
        write(
            &vault,
            "Things/Bob.md",
            "---\ntype_key: custom-thing\n---\n",
        );
        let s = run(&w, &vault, &options()).unwrap();
        assert_eq!(s.created, 3);
        let g = entity_named(&w.conn, "Gizmo");
        let ada = entity_named(&w.conn, "Ada");
        let bob = entity_named(&w.conn, "Bob");
        let p = &g.properties;
        assert_eq!(p["text"], "42");
        assert_eq!(p["long"], "a\nb");
        assert_eq!(p["num"], 5.5);
        assert_eq!(p["flag"], true);
        assert_eq!(p["pick"], "b");
        assert_eq!(p["picks"], json!(["x", "z"]));
        assert_eq!(p["when"], -400);
        let cal = calendar_manager::get_display(&w.conn).unwrap().definition;
        assert_eq!(
            p["later"],
            calendar::from_date(&cal, 0, 1203, 3, 15).unwrap()
        );
        assert!(p.get("site").is_none());
        assert_eq!(p["boss"], json!(ada.id));
        // "Stranger" has no note and is dropped; Bob matches a plain name.
        assert_eq!(p["allies"], json!([ada.id, bob.id]));
        let warns = s.warnings.join("\n");
        assert!(warns.contains("nope"), "{warns}");
        assert!(warns.contains("Site"), "{warns}");
        // Entity references also became field_ref links.
        let refs = db::links::targets_for_source(&w.conn, &g.id, "field_ref").unwrap();
        assert_eq!(refs.len(), 2);
    }

    fn orbis_note(name: &str, type_key: &str, relationships: &[&str]) -> String {
        let mut text = format!(
            "---\nid: \"old-{name}\"\ntype: \"Character\"\ntype_key: \"{type_key}\"\nsummary: \"About {name}\"\ncreated: \"2020-01-02T03:04:05.000Z\"\nupdated: \"2021-02-03T04:05:06.000Z\"\n---\n# {name}\n\nAbout {name}\n\nLore of {name}.\n"
        );
        if !relationships.is_empty() {
            text.push_str("\n## Relationships\n\n");
            for r in relationships {
                text.push_str(&format!("- {r}\n"));
            }
        }
        text
    }

    #[test]
    fn orbis_notes_restore_relationships_and_drop_generated_parts() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        write(&vault, ".orbis-export", "{}");
        write(&vault, "README.md", "# index\n- [[Ada]]");
        write(
            &vault,
            "Characters/Ada.md",
            &orbis_note(
                "Ada",
                "character",
                &[
                    "child of [[Bob]]",
                    "friend of [[Cy]]",
                    "rivals with [[Cy]]",
                    "leader of [[Nowhere]]",
                ],
            ),
        );
        write(
            &vault,
            "Characters/Bob.md",
            &orbis_note("Bob", "character", &["parent of [[Ada]]"]),
        );
        write(
            &vault,
            "Characters/Cy.md",
            &orbis_note(
                "Cy",
                "character",
                &["friend of [[Ada]]", "rivals with [[Ada]]"],
            ),
        );
        let w = new_world(dir.path(), "W");
        let p = preview(&w.path, &vault, &options()).unwrap();
        assert!(p.orbis_export);
        assert_eq!(p.notes, 3, "the index note is not a note");
        // The link to "Nowhere" has no note and is not counted.
        assert_eq!(p.relationships, 3 + 1 + 2);

        let s = run(&w, &vault, &options()).unwrap();
        // Bob parent_of Ada (once), Ada friend_of Cy (once), one custom.
        assert_eq!(s.relationships, 3);
        assert!(
            s.warnings.iter().any(|m| m.contains("Nowhere")),
            "{:?}",
            s.warnings
        );
        let ada = entity_named(&w.conn, "Ada");
        let bob = entity_named(&w.conn, "Bob");
        let rels = relationships::list_for_entity(&w.conn, &ada.id).unwrap();
        let mut seen: Vec<(String, String)> = rels
            .iter()
            .map(|r| (r.label.clone(), r.other_name.clone()))
            .collect();
        seen.sort();
        assert_eq!(
            seen,
            vec![
                ("child of".to_owned(), "Bob".to_owned()),
                ("friend of".to_owned(), "Cy".to_owned()),
                ("rivals with".to_owned(), "Cy".to_owned()),
            ]
        );
        assert!(
            rels.iter()
                .any(|r| r.other_id == bob.id && r.kind == "parent_of" && !r.outgoing)
        );
        // Title, summary and the relationships section are not lore; the
        // timestamps of the export are kept.
        assert_eq!(doc_text(&w.conn, &ada.id), "Lore of Ada.");
        assert_eq!(ada.summary, "About Ada");
        assert_eq!(ada.created_at, "2020-01-02T03:04:05.000Z");
        assert_eq!(ada.updated_at, "2021-02-03T04:05:06.000Z");
    }

    #[test]
    fn a_failure_in_the_middle_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        make_vault(&vault);
        let w = new_world(dir.path(), "W");
        let before = counts(&w.conn);
        let assets_before = files_in(&w.path.join(ASSETS_DIR));
        // Create a custom type too, so rollback of types is covered.
        let o = ImportOptions {
            folder_type_map: vec![FolderTypeMapping {
                folder: "Random".into(),
                type_key: None,
                create_custom: Some(NewTypeSpec {
                    label: "Idea".into(),
                    plural: "Ideas".into(),
                    icon: "star".into(),
                    color: "".into(),
                }),
            }],
            ..with_cities_mapped()
        };

        // Fail once the writing phase is well under way.
        let mut writes = 0;
        let mut failing = |_: u32, _: u32, phase: &str| {
            if phase == "writing" {
                writes += 1;
                if writes >= 3 {
                    return Err(AppError::Internal("boom".into()));
                }
            }
            Ok(())
        };
        let err = import(&w.path, &vault, &o, &mut failing).unwrap_err();
        assert!(err.to_string().contains("boom"));
        assert_eq!(counts(&w.conn), before);
        assert_eq!(files_in(&w.path.join(ASSETS_DIR)), assets_before);
        let leftovers: i64 = w
            .conn
            .query_row(
                "SELECT (SELECT count(*) FROM links) + (SELECT count(*) FROM documents)
                 + (SELECT count(*) FROM tags) + (SELECT count(*) FROM entity_images)
                 + (SELECT count(*) FROM search_index)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(leftovers, 0);

        // Failing while reading also leaves nothing, and a later import works.
        let mut early = |_: u32, _: u32, phase: &str| {
            if phase == "reading" {
                Err(AppError::Internal("early".into()))
            } else {
                Ok(())
            }
        };
        assert!(import(&w.path, &vault, &o, &mut early).is_err());
        assert_eq!(counts(&w.conn), before);
        assert!(run(&w, &vault, &o).is_ok());
        assert_eq!(counts(&w.conn).0, 3);
    }

    #[test]
    fn progress_is_reported_in_phases() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        make_vault(&vault);
        let w = new_world(dir.path(), "W");
        let mut phases: Vec<String> = Vec::new();
        let mut record = |_: u32, _: u32, phase: &str| {
            if phases.last().map(String::as_str) != Some(phase) {
                phases.push(phase.to_owned());
            }
            Ok(())
        };
        import(&w.path, &vault, &options(), &mut record).unwrap();
        assert_eq!(
            phases,
            vec!["scanning", "reading", "converting", "writing", "finishing"]
        );
    }

    #[test]
    fn size_limits_and_bad_front_matter_become_warnings() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        write(&vault, "Fine.md", "---\nbroken: [yaml\n---\nbody here");
        fs::write(
            vault.join("Huge.md"),
            vec![b'x'; (vault_scan::MAX_NOTE_BYTES + 10) as usize],
        )
        .unwrap();
        let w = new_world(dir.path(), "W");
        let p = preview(&w.path, &vault, &options()).unwrap();
        assert_eq!(p.notes, 1);
        assert!(
            p.warnings.iter().any(|m| m.contains("Huge.md")),
            "{:?}",
            p.warnings
        );
        assert!(
            p.warnings.iter().any(|m| m.contains("front matter")),
            "{:?}",
            p.warnings
        );
        let s = run(&w, &vault, &options()).unwrap();
        assert_eq!(s.created, 1);
        let fine = entity_named(&w.conn, "Fine");
        assert_eq!(doc_text(&w.conn, &fine.id), "body here");

        // Warnings are capped.
        let many = dir.path().join("many");
        for i in 0..120 {
            write(&many, &format!("n{i}.md"), "![[nope.png]]");
        }
        let w2 = new_world(dir.path(), "W2");
        let s = run(&w2, &many, &options()).unwrap();
        assert_eq!(s.warnings.len(), MAX_WARNINGS_IMPORT);
        assert!(s.more_warnings >= 20);
        let p = preview(&w2.path, &many, &options()).unwrap();
        assert_eq!(p.warnings.len(), MAX_WARNINGS_PREVIEW);
    }

    #[test]
    fn two_runs_of_the_same_import_do_not_interfere() {
        // Importing twice creates the notes twice (unless skipping).
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        write(&vault, "A.md", "[[B]]");
        write(&vault, "B.md", "[[A]]");
        let w = new_world(dir.path(), "W");
        run(&w, &vault, &options()).unwrap();
        run(&w, &vault, &options()).unwrap();
        assert_eq!(counts(&w.conn).0, 4);
        let o = ImportOptions {
            skip_duplicates: true,
            ..options()
        };
        let s = run(&w, &vault, &o).unwrap();
        assert_eq!((s.created, s.skipped), (0, 2));
    }

    // ---- the full round trip -----------------------------------------------

    /// Everything about a world that an export + import must preserve,
    /// keyed by `(type key, name)` and with entity ids replaced by names.
    #[derive(Debug, PartialEq)]
    struct Snap {
        summary: String,
        props: BTreeMap<String, Value>,
        tags: Vec<String>,
        lore: String,
        mentions: BTreeSet<String>,
        field_refs: BTreeSet<String>,
        created_at: String,
        updated_at: String,
        event: Option<(i64, Option<i64>, i32)>,
        involved: BTreeSet<String>,
        gallery: Vec<(String, bool)>,
        has_map_image: bool,
        favorite: bool,
    }

    fn snapshot(w: &WorldHandle) -> BTreeMap<(String, String), Snap> {
        let conn = &w.conn;
        let types = entity_types::merged(conn).unwrap();
        let all = db::entities::list_all(conn).unwrap();
        let names: HashMap<String, String> =
            all.iter().map(|e| (e.id.clone(), e.name.clone())).collect();
        let assets_dir = w.path.join(ASSETS_DIR);
        let name_of = |id: &str| names.get(id).cloned().unwrap_or_else(|| format!("?{id}"));
        let mut out = BTreeMap::new();
        for e in &all {
            let def = types.iter().find(|t| t.key == e.entity_type).unwrap();
            let mut props = BTreeMap::new();
            for (k, v) in e.properties.as_object().unwrap() {
                // Empty values mean "not set" and are not exported.
                if matches!(v, Value::Array(a) if a.is_empty()) || v == "" || v.is_null() {
                    continue;
                }
                let field = def.fields.iter().find(|f| &f.key == k);
                let v = match (field.map(|f| &f.kind), v) {
                    (Some(FieldKind::EntityRef { .. }), Value::String(id)) => json!(name_of(id)),
                    (Some(FieldKind::EntityRef { .. }), Value::Array(ids)) => {
                        let mut n: Vec<String> =
                            ids.iter().filter_map(Value::as_str).map(&name_of).collect();
                        n.sort();
                        json!(n)
                    }
                    _ => v.clone(),
                };
                props.insert(k.clone(), v);
            }
            let doc = documents::get(conn, &e.id).unwrap();
            let mention_names = |kind: &str| -> BTreeSet<String> {
                db::links::targets_for_source(conn, &e.id, kind)
                    .unwrap()
                    .iter()
                    .map(|id| name_of(id))
                    .collect()
            };
            let event = db::events::get(conn, &e.id)
                .unwrap()
                .map(|ev| (ev.start_day, ev.end_day, ev.importance));
            let involved: BTreeSet<String> = if event.is_some() {
                events::list_involved(conn, &e.id)
                    .unwrap()
                    .iter()
                    .map(|s| s.name.clone())
                    .collect()
            } else {
                BTreeSet::new()
            };
            let gallery: Vec<(String, bool)> = gallery::list(conn, &assets_dir, &e.id)
                .unwrap()
                .into_iter()
                .map(|i| (i.caption, i.is_portrait))
                .collect();
            let has_map_image = db::maps::get_map_by_entity(conn, &e.id)
                .unwrap()
                .is_some_and(|m| m.background.is_some());
            let snap = Snap {
                summary: e.summary.clone(),
                props,
                tags: e.tags.clone(),
                lore: doc.plain_text,
                mentions: mention_names("mention"),
                field_refs: mention_names("field_ref"),
                created_at: e.created_at.clone(),
                updated_at: e.updated_at.clone(),
                event,
                involved,
                gallery,
                has_map_image,
                favorite: e.favorite,
            };
            out.insert((e.entity_type.clone(), e.name.clone()), snap);
        }
        out
    }

    /// `(source name, kind, target name, label)` of every relationship.
    fn relationship_set(w: &WorldHandle) -> Vec<(String, String, String, String)> {
        let all = db::entities::list_all(&w.conn).unwrap();
        let name_of = |id: &str| all.iter().find(|e| e.id == id).unwrap().name.clone();
        let mut out = Vec::new();
        for e in &all {
            for r in relationships::list_for_entity(&w.conn, &e.id).unwrap() {
                if r.outgoing {
                    let label = db::links::get_link(&w.conn, &r.link_id)
                        .unwrap()
                        .and_then(|l| l.label)
                        .unwrap_or_default();
                    let (a, b) = (e.name.clone(), name_of(&r.other_id));
                    // Symmetric kinds are stored with the smaller id first, and
                    // ids differ between worlds: compare them as unordered pairs.
                    let (a, b) = if r.symmetric && b < a { (b, a) } else { (a, b) };
                    out.push((a, r.kind, b, label));
                }
            }
        }
        out.sort();
        out
    }

    fn export_options() -> MarkdownExportOptions {
        MarkdownExportOptions {
            include_assets: true,
            include_maps: true,
            include_events: true,
            overwrite: false,
        }
    }

    #[test]
    fn exporting_the_sample_world_and_importing_it_again_keeps_everything() {
        let dir = tempfile::tempdir().unwrap();
        let source = sample_world::create(dir.path()).unwrap();
        let exports = dir.path().join("exports");
        fs::create_dir_all(&exports).unwrap();
        let mut quiet = |_: u32, _: u32, _: &str| {};
        let summary = markdown_export::export_markdown(
            &source.path,
            sample_world::SAMPLE_NAME,
            &exports,
            &export_options(),
            &mut quiet,
        )
        .unwrap();

        let target = new_world(dir.path(), "Target");
        // A preview of the export in the empty world.
        let p = preview(&target.path, &summary.path, &options()).unwrap();
        assert!(p.orbis_export);
        assert!(p.by_type.iter().any(|t| t.label == "Deity" && t.new_type));
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);

        let result = run(&target, &summary.path, &options()).unwrap();
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        assert_eq!(result.types_created, 1);

        let before = snapshot(&source);
        let mut after = snapshot(&target);
        // Favourites are a personal marker and are not exported.
        let strip_favorite = |m: &mut BTreeMap<(String, String), Snap>| {
            for s in m.values_mut() {
                s.favorite = false;
            }
        };
        let mut before = before;
        strip_favorite(&mut before);
        strip_favorite(&mut after);

        // Same entities per type, with the same names.
        let keys = |m: &BTreeMap<(String, String), Snap>| m.keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&before), keys(&after));
        assert_eq!(result.created as usize, before.len());
        // And the same content, field values, dates, links and pictures.
        for (key, expected) in &before {
            assert_eq!(&after[key], expected, "entity {key:?} differs");
        }
        assert_eq!(relationship_set(&source), relationship_set(&target));
        assert!(!relationship_set(&target).is_empty());

        // The custom type came with its fields and the references work.
        let deity = entity_types::find(&target.conn, "custom-deity")
            .unwrap()
            .unwrap();
        assert_eq!(deity.fields.len(), 3);
        assert!(deity.fields.iter().all(|f| f.custom));
        // Mentions and backlinks of the queen are the same.
        let queen_src = entity_named(&source.conn, "Queen Isolde Varn");
        let queen_dst = entity_named(&target.conn, "Queen Isolde Varn");
        let names = |w: &WorldHandle, id: &str, name: &str| -> Vec<String> {
            let mut v: Vec<String> = crate::services::links::backlinks(&w.conn, id, name)
                .unwrap()
                .into_iter()
                .map(|b| b.source_name)
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            names(&source, &queen_src.id, &queen_src.name),
            names(&target, &queen_dst.id, &queen_dst.name)
        );
        // A second import of the same export, skipping duplicates, adds nothing.
        let o = ImportOptions {
            skip_duplicates: true,
            ..options()
        };
        let again = run(&target, &summary.path, &o).unwrap();
        assert_eq!(again.created, 0);
        assert_eq!(again.skipped as usize, before.len());
    }
}
