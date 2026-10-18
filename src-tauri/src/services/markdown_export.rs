//! Export a world as a folder of Markdown notes that Obsidian can open.
//!
//! ```text
//! <target>/<World name>/
//! ├─ .orbis-export        marker: this folder may be replaced by a later export
//! ├─ README.md            index of every note
//! ├─ Characters/Queen Isolde Varn.md   one folder per type (plural label)
//! ├─ Locations/...
//! └─ assets/<id>.png      map backgrounds (only with `include_assets`)
//! ```
//!
//! Link rules (Obsidian links by FILE NAME, not path):
//! - Every note gets a file name that is unique in the whole export, ignoring
//!   case. The first entity (type order, then name) keeps its plain name;
//!   later ones with the same name become `Name (Type)`, then
//!   `Name (Type) (2)`, ...
//! - Links are written `[[File Name]]`, or `[[File Name|Display Name]]` when
//!   the file name had to differ from the entity's name.
//!
//! Safety: everything is written into a hidden temporary folder next to the
//! final one and renamed at the end, so a failure never leaves a half-written
//! export. An existing `<World name>` folder is only replaced (with
//! `overwrite`) when it carries the marker file of an earlier export.
//!
//! Reading: the export runs on a background thread (see
//! `commands::export`) with its OWN read-only connection to `world.db`, in
//! one read transaction. SQLite's WAL mode lets that run while the app keeps
//! working, and the export sees one consistent snapshot of the world. This is
//! the same approach as `services::backups`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::calendar::{CalendarDefinition, DateStyle};
use crate::models::entity::Entity;
use crate::models::entity_type::{EntityTypeDef, FieldDef, FieldKind};
use crate::models::export::MarkdownExportOptions;
use crate::models::map::MapGeometry;
use crate::services::world::{ASSETS_DIR, DB_FILE};
use crate::services::{
    assets, calendar, calendar_manager, documents, events, markdown, relationships,
};

/// Name of the marker file inside every export folder.
pub const MARKER_FILE: &str = ".orbis-export";
/// Name of the index note.
pub const INDEX_FILE: &str = "README.md";
/// Longest file name stem we write (in characters).
const MAX_STEM_CHARS: usize = 100;

/// What an export produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    /// The exported folder, `<target>/<World name>`.
    pub path: PathBuf,
    /// Notes, index and copied images.
    pub files: u32,
}

/// Called as `(done, total, phase)` while the export runs.
pub type ProgressFn<'a> = &'a mut dyn FnMut(u32, u32, &str);

// ---- file names ------------------------------------------------------------

/// Names Windows refuses as file names (with or without an extension).
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Makes `name` safe as a file or folder name on Windows, Linux and macOS,
/// and safe inside an Obsidian `[[link]]`:
/// - `/ \ : * ? " < > |` become `-`; `# ^ [ ]` are removed (they break links);
/// - control characters and runs of whitespace become one space;
/// - leading dots and trailing dots/spaces are removed;
/// - Windows reserved names (`CON`, `LPT1`, ...) get a trailing `_`;
/// - at most 100 characters; an empty result becomes `Untitled`.
pub fn sanitize_file_name(name: &str) -> String {
    let mut cleaned = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => cleaned.push('-'),
            '#' | '^' | '[' | ']' => {}
            c if c.is_control() => cleaned.push(' '),
            c => cleaned.push(c),
        }
    }
    // `split_whitespace` + `join` collapses any run of whitespace to one space.
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = collapsed
        .trim_start_matches('.')
        .chars()
        .take(MAX_STEM_CHARS)
        .collect();
    while out.ends_with('.') || out.ends_with(' ') {
        out.pop();
    }
    if out.is_empty() {
        return "Untitled".into();
    }
    let stem = out.split('.').next().unwrap_or("").to_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        out.push('_');
    }
    out
}

/// Hands out names that are unique ignoring case.
struct NameAllocator {
    taken: HashSet<String>,
}

impl NameAllocator {
    fn new(reserved: &[&str]) -> Self {
        NameAllocator {
            taken: reserved.iter().map(|r| r.to_lowercase()).collect(),
        }
    }

    fn is_free(&self, name: &str) -> bool {
        !self.taken.contains(&name.to_lowercase())
    }

    fn take(&mut self, name: &str) {
        self.taken.insert(name.to_lowercase());
    }

    /// `base`, else `base (qualifier)`, else `base (qualifier) 2`, ...
    fn allocate(&mut self, base: &str, qualifier: &str) -> String {
        if self.is_free(base) {
            self.take(base);
            return base.to_owned();
        }
        let qualified = format!("{base} ({qualifier})");
        let mut candidate = qualified.clone();
        let mut n = 2;
        while !self.is_free(&candidate) {
            candidate = format!("{qualified} {n}");
            n += 1;
        }
        self.take(&candidate);
        candidate
    }
}

/// Text for the part of a `[[File|alias]]` link after the bar.
fn clean_alias(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('[', "(")
        .replace(']', ")")
        .replace('|', "/")
}

// ---- the plan: which note each entity gets ---------------------------------

struct Entry {
    entity: Entity,
    /// Folder name (the type's plural).
    folder: String,
    /// File name without `.md`.
    stem: String,
    /// False for maps/events left out by the options.
    included: bool,
}

struct Plan {
    entries: Vec<Entry>,
    by_id: HashMap<String, usize>,
    types: Vec<EntityTypeDef>,
}

impl Plan {
    fn entry(&self, id: &str) -> Option<&Entry> {
        self.by_id.get(id).map(|i| &self.entries[*i])
    }

    /// How to refer to an entity inside a note: a wikilink for exported
    /// entities, plain (escaped) text for entities left out of the export,
    /// `None` for ids that do not exist.
    fn link(&self, id: &str) -> Option<String> {
        let e = self.entry(id)?;
        if !e.included {
            return Some(markdown::escape_text(&clean_alias(&e.entity.name)));
        }
        let alias = clean_alias(&e.entity.name);
        Some(if alias == e.stem {
            format!("[[{}]]", e.stem)
        } else {
            format!("[[{}|{alias}]]", e.stem)
        })
    }

    fn type_def(&self, key: &str) -> Option<&EntityTypeDef> {
        self.types.iter().find(|t| t.key == key)
    }
}

fn type_included(key: &str, options: &MarkdownExportOptions) -> bool {
    !(key == "map" && !options.include_maps || key == "event" && !options.include_events)
}

fn build_plan(conn: &Connection, options: &MarkdownExportOptions) -> AppResult<Plan> {
    let types = crate::services::entity_types::merged(conn)?;
    let mut entities = db::entities::list_all(conn)?;
    // Type order first (the order of the sidebar), unknown types last.
    let rank = |key: &str| {
        types
            .iter()
            .position(|t| t.key == key)
            .unwrap_or(types.len())
    };
    entities.sort_by_cached_key(|e| (rank(&e.entity_type), e.name.to_lowercase(), e.id.clone()));

    // Folder names, one per type that has entities.
    let mut folders = NameAllocator::new(&[ASSETS_DIR]);
    let mut folder_of: HashMap<String, String> = HashMap::new();
    for e in &entities {
        if folder_of.contains_key(&e.entity_type) {
            continue;
        }
        let plural = types
            .iter()
            .find(|t| t.key == e.entity_type)
            .map_or(e.entity_type.as_str(), |t| t.plural.as_str());
        let base = sanitize_file_name(plural);
        let folder = folders.allocate(&base, &e.entity_type);
        folder_of.insert(e.entity_type.clone(), folder);
    }

    // File names, unique over the whole export. The index note is reserved.
    let mut names = NameAllocator::new(&["README"]);
    let mut entries = Vec::with_capacity(entities.len());
    let mut by_id = HashMap::new();
    for entity in entities {
        let type_label = types
            .iter()
            .find(|t| t.key == entity.entity_type)
            .map_or_else(|| entity.entity_type.clone(), |t| t.label.clone());
        let included = type_included(&entity.entity_type, options);
        let base = sanitize_file_name(&entity.name);
        // Entities left out of the export do not use up a name.
        let stem = if included {
            names.allocate(&base, &sanitize_file_name(&type_label))
        } else {
            base
        };
        by_id.insert(entity.id.clone(), entries.len());
        entries.push(Entry {
            folder: folder_of
                .get(&entity.entity_type)
                .cloned()
                .unwrap_or_default(),
            stem,
            included,
            entity,
        });
    }
    Ok(Plan {
        entries,
        by_id,
        types,
    })
}

// ---- YAML front matter -----------------------------------------------------

/// A tiny YAML writer for the front matter. Strings are written as JSON
/// strings, which are valid YAML double-quoted scalars.
#[derive(Default)]
struct Front {
    lines: Vec<String>,
}

impl Front {
    fn string(&mut self, key: &str, value: &str) {
        let quoted = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into());
        self.lines.push(format!("{key}: {quoted}"));
    }

    fn raw(&mut self, key: &str, value: impl std::fmt::Display) {
        self.lines.push(format!("{key}: {value}"));
    }

    fn list(&mut self, key: &str, values: &[String]) {
        if values.is_empty() {
            return;
        }
        self.lines.push(format!("{key}:"));
        for v in values {
            let quoted = serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into());
            self.lines.push(format!("  - {quoted}"));
        }
    }

    fn finish(self) -> String {
        format!("---\n{}\n---\n", self.lines.join("\n"))
    }
}

/// Front matter keys the export writes itself; a field with one of these keys
/// is written as `field_<key>` instead.
const RESERVED_KEYS: [&str; 14] = [
    "id",
    "type",
    "type_key",
    "aliases",
    "summary",
    "created",
    "updated",
    "tags",
    "cssclasses",
    "start",
    "start_day",
    "end",
    "end_day",
    "importance",
];

fn format_day(def: &CalendarDefinition, day: i64) -> String {
    calendar::format(def, day, DateStyle::Full).unwrap_or_else(|_| format!("day {day}"))
}

fn write_field(front: &mut Front, plan: &Plan, cal: &CalendarDefinition, f: &FieldDef, v: &Value) {
    let key = if RESERVED_KEYS.contains(&f.key.as_str()) {
        format!("field_{}", f.key)
    } else {
        f.key.clone()
    };
    match (&f.kind, v) {
        (_, Value::Null) => {}
        (FieldKind::Text | FieldKind::LongText | FieldKind::Select | FieldKind::Url, v) => {
            if let Some(s) = v.as_str().filter(|s| !s.is_empty()) {
                front.string(&key, s);
            }
        }
        (FieldKind::Number, v) => {
            if v.is_number() {
                front.raw(&key, v);
            }
        }
        (FieldKind::Boolean, v) => {
            if let Some(b) = v.as_bool() {
                front.raw(&key, b);
            }
        }
        (FieldKind::MultiSelect, v) => {
            let items: Vec<String> = v
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            front.list(&key, &items);
        }
        (FieldKind::Date, v) => {
            if let Some(day) = v.as_i64() {
                front.string(&key, &format_day(cal, day));
                front.raw(&format!("{key}_day"), day);
            }
        }
        (FieldKind::EntityRef { multiple, .. }, v) => {
            let ids: Vec<&str> = match v {
                Value::String(s) => vec![s.as_str()],
                Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                _ => vec![],
            };
            // Ids of deleted entities are dropped.
            let links: Vec<String> = ids.iter().filter_map(|id| plan.link(id)).collect();
            if *multiple {
                front.list(&key, &links);
            } else if let Some(first) = links.first() {
                front.string(&key, first);
            }
        }
    }
}

// ---- one note ----------------------------------------------------------------

/// Everything `render_note` needs besides the entry itself.
struct RenderCtx<'a> {
    conn: &'a Connection,
    plan: &'a Plan,
    calendar: &'a CalendarDefinition,
    options: &'a MarkdownExportOptions,
    assets_dir: &'a Path,
}

/// A map background that must be copied: (source file, file name in `assets/`).
type AssetCopy = (PathBuf, String);

fn render_note(ctx: &RenderCtx, entry: &Entry) -> AppResult<(String, Option<AssetCopy>)> {
    let conn = ctx.conn;
    let plan = ctx.plan;
    let e = &entry.entity;
    let type_def = plan.type_def(&e.entity_type);

    let mut front = Front::default();
    front.string("id", &e.id);
    front.string(
        "type",
        type_def.map_or(e.entity_type.as_str(), |t| t.label.as_str()),
    );
    front.string("type_key", &e.entity_type);
    let alias = clean_alias(&e.name);
    if alias != entry.stem {
        front.list("aliases", std::slice::from_ref(&e.name));
    }
    if !e.summary.trim().is_empty() {
        front.string("summary", e.summary.trim());
    }
    // Tags (nested ones keep their `/`, which Obsidian understands).
    front.list("tags", &e.tags);
    front.string("created", &e.created_at);
    front.string("updated", &e.updated_at);
    if let Some(def) = type_def {
        for f in &def.fields {
            if let Some(v) = e.properties.get(&f.key) {
                write_field(&mut front, plan, ctx.calendar, f, v);
            }
        }
    }

    let mut body: Vec<String> = Vec::new();
    body.push(format!(
        "# {}",
        e.name.split_whitespace().collect::<Vec<_>>().join(" ")
    ));
    if !e.summary.trim().is_empty() {
        body.push(markdown::plain_paragraph(&e.summary));
    }

    // Event dates.
    if e.entity_type == "event"
        && let Some(ev) = db::events::get(conn, &e.id)?
    {
        front.raw("start_day", ev.start_day);
        front.string("start", &format_day(ctx.calendar, ev.start_day));
        if let Some(end) = ev.end_day {
            front.raw("end_day", end);
            front.string("end", &format_day(ctx.calendar, end));
        }
        front.raw("importance", ev.importance);
        body.push(format!(
            "**Date:** {}",
            event_dates(ctx.calendar, ev.start_day, ev.end_day)
        ));
    }

    // Map picture.
    let mut copy: Option<AssetCopy> = None;
    let mut map_lists: Vec<String> = Vec::new();
    if e.entity_type == "map"
        && let Some(info) = db::maps::get_map_by_entity(conn, &e.id)?
    {
        if ctx.options.include_assets
            && let Some(bg) = &info.background
        {
            let source = assets::asset_file(ctx.assets_dir, &bg.id, &bg.mime);
            if source.is_file()
                && let Some(file_name) =
                    source.file_name().map(|n| n.to_string_lossy().into_owned())
            {
                body.push(format!("![[{ASSETS_DIR}/{file_name}]]"));
                copy = Some((source, file_name));
            }
        }
        map_lists = map_object_lists(conn, plan, &info.id)?;
    }

    // Lore.
    // (`documents::get` returns an empty document when none was saved.)
    let doc = documents::get(conn, &e.id)?;
    if let Ok(tree) = serde_json::from_str::<Value>(&doc.content) {
        let resolver = |id: &str| plan.link(id);
        let lore = markdown::tiptap_to_markdown(&tree, &resolver);
        if !lore.trim().is_empty() {
            body.push(lore);
        }
    }
    body.extend(map_lists);

    // Events: who took part / what the entity took part in.
    if ctx.options.include_events {
        if e.entity_type == "event" {
            let involved = events::list_involved(conn, &e.id)?;
            let lines: Vec<String> = involved
                .iter()
                .filter_map(|s| plan.link(&s.id))
                .map(|l| format!("- {l}"))
                .collect();
            if !lines.is_empty() {
                body.push(format!("## Involved\n\n{}", lines.join("\n")));
            }
        } else {
            let list = events::list_for_entity(conn, &e.id)?;
            let lines: Vec<String> = list
                .iter()
                .filter_map(|ev| {
                    plan.link(&ev.entity_id).map(|l| {
                        format!(
                            "- {l} ({})",
                            event_dates(ctx.calendar, ev.start_day, ev.end_day)
                        )
                    })
                })
                .collect();
            if !lines.is_empty() {
                body.push(format!("## Events\n\n{}", lines.join("\n")));
            }
        }
    }

    // Relationships, phrased from this entity's point of view.
    let rels = relationships::list_for_entity(conn, &e.id)?;
    let lines: Vec<String> = rels
        .iter()
        .filter_map(|r| {
            plan.link(&r.other_id)
                .map(|l| format!("- {} {l}", markdown::escape_text(&r.label)))
        })
        .collect();
    if !lines.is_empty() {
        body.push(format!("## Relationships\n\n{}", lines.join("\n")));
    }

    let text = format!("{}\n{}\n", front.finish(), body.join("\n\n"));
    Ok((text, copy))
}

fn event_dates(cal: &CalendarDefinition, start: i64, end: Option<i64>) -> String {
    match end {
        Some(end) if end != start => {
            format!(
                "{} \u{2013} {}",
                format_day(cal, start),
                format_day(cal, end)
            )
        }
        _ => format_day(cal, start),
    }
}

/// "## Pins" and "## Areas" sections listing the entities a map links to.
fn map_object_lists(conn: &Connection, plan: &Plan, map_id: &str) -> AppResult<Vec<String>> {
    let mut sections = Vec::new();
    for (kind, title) in [("pin", "Pins"), ("area", "Areas")] {
        let mut lines: Vec<String> = Vec::new();
        for o in db::maps::list_objects(conn, map_id)? {
            let matches = matches!(
                (&o.geometry, kind),
                (MapGeometry::Pin { .. }, "pin") | (MapGeometry::Area { .. }, "area")
            );
            if !matches {
                continue;
            }
            if let Some(link) = o.entity_id.as_deref().and_then(|id| plan.link(id)) {
                let line = format!("- {link}");
                if !lines.contains(&line) {
                    lines.push(line);
                }
            }
        }
        lines.sort_by_key(|l| l.to_lowercase());
        if !lines.is_empty() {
            sections.push(format!("## {title}\n\n{}", lines.join("\n")));
        }
    }
    Ok(sections)
}

fn index_note(plan: &Plan, world_name: &str) -> String {
    let mut parts = vec![
        format!(
            "# {}",
            world_name.split_whitespace().collect::<Vec<_>>().join(" ")
        ),
        format!(
            "Exported from Orbis Scriptor on {}.",
            Utc::now().format("%Y-%m-%d")
        ),
    ];
    let mut keys: Vec<&str> = Vec::new();
    for e in plan.entries.iter().filter(|e| e.included) {
        if !keys.contains(&e.entity.entity_type.as_str()) {
            keys.push(&e.entity.entity_type);
        }
    }
    for key in keys {
        let title = plan.type_def(key).map_or(key, |t| t.plural.as_str());
        let lines: Vec<String> = plan
            .entries
            .iter()
            .filter(|e| e.included && e.entity.entity_type == key)
            .filter_map(|e| plan.link(&e.entity.id))
            .map(|l| format!("- {l}"))
            .collect();
        parts.push(format!(
            "## {}\n\n{}",
            markdown::escape_text(title),
            lines.join("\n")
        ));
    }
    format!("{}\n", parts.join("\n\n"))
}

// ---- the export ------------------------------------------------------------

/// Writes the Markdown export of the world in `world_dir` into
/// `<target_dir>/<World name>/`. Blocking: call it from a background thread.
/// `on_progress` receives `(done, total, phase)`.
pub fn export_markdown(
    world_dir: &Path,
    world_name: &str,
    target_dir: &Path,
    options: &MarkdownExportOptions,
    on_progress: ProgressFn,
) -> AppResult<ExportSummary> {
    if !target_dir.is_dir() {
        return Err(AppError::Validation(
            "The export folder does not exist".into(),
        ));
    }
    let final_dir = target_dir.join(sanitize_file_name(world_name));
    if final_dir.exists() {
        if !options.overwrite {
            return Err(AppError::Validation(format!(
                "A folder called \"{}\" already exists there. Choose another location or replace the earlier export.",
                sanitize_file_name(world_name)
            )));
        }
        if !final_dir.join(MARKER_FILE).is_file() {
            return Err(AppError::Validation(
                "That folder was not created by an earlier export, so it will not be replaced"
                    .into(),
            ));
        }
    }

    // Everything is written into a hidden temporary folder in the same
    // parent (so the final rename stays on one disk).
    let tmp = target_dir.join(format!(".orbis-export-{}", Uuid::now_v7()));
    fs::create_dir(&tmp)?;
    // `match` on the result so the temporary folder is removed on failure.
    match write_all(world_dir, world_name, &tmp, options, on_progress)
        .and_then(|files| swap_into_place(&tmp, &final_dir, options.overwrite).map(|()| files))
    {
        Ok(files) => Ok(ExportSummary {
            path: final_dir,
            files,
        }),
        Err(err) => {
            let _ = fs::remove_dir_all(&tmp);
            Err(err)
        }
    }
}

/// Renders and writes every file into `out`; returns how many were written.
fn write_all(
    world_dir: &Path,
    world_name: &str,
    out: &Path,
    options: &MarkdownExportOptions,
    on_progress: ProgressFn,
) -> AppResult<u32> {
    on_progress(0, 0, "preparing");
    // A separate read-only connection, in one read transaction (a snapshot).
    let conn = Connection::open_with_flags(
        world_dir.join(DB_FILE),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.execute_batch("BEGIN")?;
    let plan = build_plan(&conn, options)?;
    let display = calendar_manager::get_display(&conn)?;
    let ctx = RenderCtx {
        conn: &conn,
        plan: &plan,
        calendar: &display.definition,
        options,
        assets_dir: &world_dir.join(ASSETS_DIR),
    };

    // +1 for the index note; copied images are counted as they are found.
    let notes = plan.entries.iter().filter(|e| e.included).count() as u32;
    let mut total = notes + 1;
    let mut done = 0u32;
    let step = (total / 100).max(1);
    let mut files = 0u32;
    let mut copied: HashSet<String> = HashSet::new();

    fs::write(out.join(MARKER_FILE), "{}\n")?;
    for entry in plan.entries.iter().filter(|e| e.included) {
        let (text, copy) = render_note(&ctx, entry)?;
        let dir = out.join(&entry.folder);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(format!("{}.md", entry.stem)), text)?;
        files += 1;
        done += 1;
        if let Some((source, name)) = copy
            && copied.insert(name.clone())
        {
            fs::create_dir_all(out.join(ASSETS_DIR))?;
            fs::copy(&source, out.join(ASSETS_DIR).join(&name))?;
            files += 1;
            total += 1;
            done += 1;
            on_progress(done, total, "assets");
        }
        if done.is_multiple_of(step) {
            on_progress(done, total, "writing");
        }
    }
    fs::write(out.join(INDEX_FILE), index_note(&plan, world_name))?;
    files += 1;
    done += 1;
    on_progress(done, total, "finishing");
    conn.execute_batch("COMMIT")?;
    Ok(files)
}

/// Moves the finished temporary folder to its final name. With `overwrite`
/// the old export is moved aside first (and put back if the move fails);
/// its `.obsidian` settings folder is carried over.
fn swap_into_place(tmp: &Path, final_dir: &Path, overwrite: bool) -> AppResult<()> {
    if !final_dir.exists() {
        fs::rename(tmp, final_dir)?;
        return Ok(());
    }
    if !overwrite {
        return Err(AppError::Validation(
            "The destination folder appeared while exporting".into(),
        ));
    }
    let aside = final_dir.with_file_name(format!(".orbis-old-{}", Uuid::now_v7()));
    fs::rename(final_dir, &aside)?;
    // Keep the user's Obsidian settings (themes, plugins) across re-exports.
    let settings = aside.join(".obsidian");
    if settings.is_dir() {
        let _ = fs::rename(&settings, tmp.join(".obsidian"));
    }
    if let Err(e) = fs::rename(tmp, final_dir) {
        let _ = fs::rename(&aside, final_dir);
        return Err(e.into());
    }
    let _ = fs::remove_dir_all(&aside);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::entity::{EntityPatch, NewEntity};
    use crate::services::{entities, sample_world};
    use serde_json::json;

    fn opts() -> MarkdownExportOptions {
        MarkdownExportOptions {
            include_assets: true,
            include_maps: true,
            include_events: true,
            overwrite: false,
        }
    }

    fn run(
        world: &crate::state::WorldHandle,
        target: &Path,
        options: &MarkdownExportOptions,
    ) -> AppResult<ExportSummary> {
        export_markdown(
            &world.path,
            &world.manifest.name,
            target,
            options,
            &mut |_, _, _| {},
        )
    }

    fn all_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.extend(all_files(&path));
            } else {
                out.push(path);
            }
        }
        out.sort();
        out
    }

    #[test]
    fn sanitises_file_names() {
        assert_eq!(sanitize_file_name("Queen Isolde"), "Queen Isolde");
        assert_eq!(
            sanitize_file_name("a/b\\c:d*e?f\"g<h>i|j"),
            "a-b-c-d-e-f-g-h-i-j"
        );
        assert_eq!(sanitize_file_name("C# [draft] ^x"), "C draft x");
        assert_eq!(
            sanitize_file_name("  many   spaces\tand\nlines "),
            "many spaces and lines"
        );
        assert_eq!(sanitize_file_name("..hidden. "), "hidden");
        assert_eq!(sanitize_file_name("???"), "---");
        assert_eq!(sanitize_file_name("  "), "Untitled");
        assert_eq!(sanitize_file_name("#^[]"), "Untitled");
        assert_eq!(sanitize_file_name("CON"), "CON_");
        assert_eq!(sanitize_file_name("lpt1"), "lpt1_");
        assert_eq!(sanitize_file_name("aux.txt"), "aux.txt_");
        assert_eq!(sanitize_file_name("Console"), "Console");
        assert_eq!(sanitize_file_name(&"x".repeat(300)).chars().count(), 100);
        assert_eq!(sanitize_file_name("Zoë \u{9fa0}"), "Zoë \u{9fa0}");
    }

    #[test]
    fn allocator_disambiguates_case_insensitively() {
        let mut a = NameAllocator::new(&["README"]);
        assert_eq!(a.allocate("Ada", "Character"), "Ada");
        assert_eq!(a.allocate("ada", "Location"), "ada (Location)");
        assert_eq!(a.allocate("ADA", "Location"), "ADA (Location) 2");
        assert_eq!(a.allocate("readme", "Note"), "readme (Note)");
    }

    /// A small world with name collisions, odd names and a deleted mention.
    fn tricky_world(dir: &Path) -> crate::state::WorldHandle {
        let w =
            crate::services::world::create_world(&dir.join("tricky"), "Tricky: World?").unwrap();
        let make = |t: &str, n: &str| {
            entities::create(
                &w.conn,
                NewEntity {
                    entity_type: t.into(),
                    name: n.into(),
                },
            )
            .unwrap()
        };
        let ada = make("character", "Ada");
        let ada_loc = make("location", "Ada");
        let con = make("note", "CON");
        let slash = make("note", "What/Why? [x]");
        let readme = make("note", "README");
        let gone = make("note", "Gone");
        crate::services::relationships::create(
            &w.conn,
            &ada.id,
            &make("character", "Bob").id,
            "friend_of",
            None,
        )
        .unwrap();
        let mention = |id: &str| json!({ "type": "mention", "attrs": { "id": id } });
        let doc = json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
            { "type": "text", "text": "Met " }, mention(&ada_loc.id), { "type": "text", "text": ", " },
            mention(&con.id), { "type": "text", "text": ", " }, mention(&slash.id),
            { "type": "text", "text": ", " }, mention(&readme.id), { "type": "text", "text": " and " },
            mention(&gone.id),
        ]}]});
        documents::save(&w.conn, &ada.id, doc.to_string(), "x".into()).unwrap();
        entities::delete(&w.conn, &gone.id).unwrap();
        w
    }

    #[test]
    fn unique_file_names_and_links_that_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let w = tricky_world(dir.path());
        let target = dir.path().join("out");
        fs::create_dir(&target).unwrap();
        let summary = run(&w, &target, &opts()).unwrap();
        assert_eq!(summary.path, target.join("Tricky- World-"));

        let names: Vec<String> = all_files(&summary.path)
            .iter()
            .map(|p| {
                p.strip_prefix(&summary.path)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert!(names.contains(&"Characters/Ada.md".to_owned()), "{names:?}");
        assert!(names.contains(&"Locations/Ada (Location).md".to_owned()));
        assert!(names.contains(&"Notes/CON_.md".to_owned()));
        assert!(names.contains(&"Notes/What-Why- x.md".to_owned()));
        assert!(names.contains(&"Notes/README (Note).md".to_owned()));
        assert!(names.contains(&"README.md".to_owned()));
        assert!(names.contains(&MARKER_FILE.to_owned()));

        let ada = fs::read_to_string(summary.path.join("Characters/Ada.md")).unwrap();
        assert!(ada.contains("[[Ada (Location)|Ada]]"), "{ada}");
        assert!(ada.contains("[[CON_|CON]]"));
        assert!(ada.contains("[[What-Why- x|What/Why? (x)]]"));
        assert!(ada.contains("[[README (Note)|README]]"));
        assert!(ada.contains("and (deleted entity)"));
        assert!(ada.contains("## Relationships\n\n- friend of [[Bob]]"));
        // The index lists everything.
        let index = fs::read_to_string(summary.path.join("README.md")).unwrap();
        assert!(index.contains("## Characters"));
        assert!(index.contains("- [[Ada (Location)|Ada]]"));
        // Only one temporary folder-like thing is left: nothing.
        let leftovers: Vec<_> = fs::read_dir(&target)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(leftovers, vec!["Tricky- World-".to_owned()]);
    }

    /// Splits a note into its YAML front matter and the Markdown below it.
    fn front_matter(text: &str) -> (serde_yaml::Value, String) {
        let rest = text
            .strip_prefix("---\n")
            .expect("starts with front matter");
        let (yaml, body) = rest.split_once("\n---\n").expect("front matter ends");
        (
            serde_yaml::from_str(yaml).expect("valid YAML"),
            body.to_owned(),
        )
    }

    /// Every `[[target]]` / `[[target|alias]]` in `text`.
    fn wikilinks(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(i) = rest.find("[[") {
            let after = &rest[i + 2..];
            let Some(j) = after.find("]]") else { break };
            let inner = &after[..j];
            out.push(inner.split('|').next().unwrap().replace('\\', ""));
            rest = &after[j + 2..];
        }
        out
    }

    #[test]
    fn exports_the_sample_world_with_valid_front_matter_and_resolving_links() {
        let dir = tempfile::tempdir().unwrap();
        let w = sample_world::create(dir.path()).unwrap();
        let target = dir.path().join("out");
        fs::create_dir(&target).unwrap();

        let mut progress: Vec<(u32, u32, String)> = Vec::new();
        let summary = export_markdown(
            &w.path,
            &w.manifest.name,
            &target,
            &opts(),
            &mut |d, t, p| progress.push((d, t, p.to_owned())),
        )
        .unwrap();

        let files = all_files(&summary.path);
        assert_eq!(files.len() as u32, summary.files + 1); // + the marker file
        let entity_count = db::entities::list_all(&w.conn).unwrap().len();
        let notes: Vec<&PathBuf> = files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        assert_eq!(notes.len(), entity_count + 1); // + README

        // Progress runs from 0 to total and ends in "finishing".
        assert_eq!(progress.first().unwrap().2, "preparing");
        let last = progress.last().unwrap();
        assert_eq!((last.0, last.2.as_str()), (last.1, "finishing"));

        // The map picture is copied and embedded.
        let assets: Vec<_> = files
            .iter()
            .filter(|p| p.starts_with(summary.path.join("assets")))
            .collect();
        assert_eq!(assets.len(), 1);
        let map_note =
            fs::read_to_string(summary.path.join("Maps/Map of the Shattered Coast.md")).unwrap();
        let asset_name = assets[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(map_note.contains(&format!("![[assets/{asset_name}]]")));
        assert!(map_note.contains("## Pins\n\n- [["));
        assert!(map_note.contains("- [[Port Veyra]]"));

        // Front matter parses; every link points at an existing file name.
        let stems: HashSet<String> = notes
            .iter()
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        let mut total_links = 0;
        for note in &notes {
            let text = fs::read_to_string(note).unwrap();
            if note.file_name().unwrap() == "README.md" {
                continue;
            }
            let (front, body) = front_matter(&text);
            assert!(front["id"].as_str().is_some(), "{note:?}");
            assert!(front["type"].as_str().is_some());
            for link in wikilinks(&body) {
                // `![[assets/...]]` embeds point at copied images, not notes.
                if link.starts_with("assets/") {
                    assert!(summary.path.join(&link).is_file(), "{link}");
                    continue;
                }
                total_links += 1;
                assert!(stems.contains(&link), "dangling [[{link}]] in {note:?}");
            }
        }
        assert!(total_links > 40, "{total_links}");

        // A character note: fields, relationships, events, lore links.
        let isolde =
            fs::read_to_string(summary.path.join("Characters/Queen Isolde Varn.md")).unwrap();
        let (front, body) = front_matter(&isolde);
        assert_eq!(front["type"].as_str(), Some("Character"));
        assert_eq!(front["species"].as_str(), Some("Human"));
        assert_eq!(front["age"].as_i64(), Some(34));
        assert!(body.trim_start().starts_with("# Queen Isolde Varn\n"));
        assert!(body.contains("## Relationships"));
        assert!(body.contains("- child of [[King Aldous Varn]]"));
        assert!(body.contains("## Events"));
        // An event note: dates formatted with the display calendar.
        let events_dir = summary.path.join("Events");
        let some_event = fs::read_dir(&events_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let (front, body) = front_matter(&fs::read_to_string(some_event).unwrap());
        assert!(front["start"].as_str().unwrap().contains("AF"));
        assert!(front["start_day"].is_number());
        assert!(body.contains("**Date:**"));
        assert!(body.contains("## Involved"));
    }

    #[test]
    fn options_leave_out_maps_events_and_assets() {
        let dir = tempfile::tempdir().unwrap();
        let w = sample_world::create(dir.path()).unwrap();
        let target = dir.path().join("out");
        fs::create_dir(&target).unwrap();
        let options = MarkdownExportOptions {
            include_assets: false,
            include_maps: false,
            include_events: false,
            overwrite: false,
        };
        let summary = run(&w, &target, &options).unwrap();
        assert!(!summary.path.join("Maps").exists());
        assert!(!summary.path.join("Events").exists());
        assert!(!summary.path.join("assets").exists());
        let isolde =
            fs::read_to_string(summary.path.join("Characters/Queen Isolde Varn.md")).unwrap();
        assert!(!isolde.contains("## Events"));
        // References to left-out entities are plain text, never dangling links.
        let (_, body) = front_matter(&isolde);
        let stems: HashSet<String> = all_files(&summary.path)
            .iter()
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        for link in wikilinks(&body) {
            assert!(stems.contains(&link), "{link}");
        }
    }

    #[test]
    fn custom_types_and_every_field_kind_in_front_matter() {
        use crate::models::entity_type::FieldInput;
        use crate::services::entity_types;
        let dir = tempfile::tempdir().unwrap();
        let w = crate::services::world::create_world(&dir.path().join("W"), "W").unwrap();
        let f = |label: &str, kind: FieldKind, options: &[&str]| FieldInput {
            key: None,
            label: label.into(),
            kind,
            options: options.iter().map(|o| (*o).to_owned()).collect(),
        };
        let t = entity_types::create(
            &w.conn,
            "Deity",
            "Deities",
            "sparkles",
            "#aa3355",
            vec![
                f("Domain", FieldKind::Select, &["war", "sea"]),
                f("Symbol", FieldKind::Text, &[]),
                f("Lore", FieldKind::LongText, &[]),
                f("Power", FieldKind::Number, &[]),
                f("Active", FieldKind::Boolean, &[]),
                f("Aspects", FieldKind::MultiSelect, &["a", "b"]),
                f("Born", FieldKind::Date, &[]),
                f("Site", FieldKind::Url, &[]),
                f("Id", FieldKind::Text, &[]),
                f(
                    "Worshippers",
                    FieldKind::EntityRef {
                        allowed_types: vec!["faction".into()],
                        multiple: true,
                    },
                    &[],
                ),
                f(
                    "Rival",
                    FieldKind::EntityRef {
                        allowed_types: vec![],
                        multiple: false,
                    },
                    &[],
                ),
            ],
        )
        .unwrap();
        let mk = |t: &str, n: &str| {
            entities::create(
                &w.conn,
                NewEntity {
                    entity_type: t.into(),
                    name: n.into(),
                },
            )
            .unwrap()
        };
        let cult = mk("faction", "Cult: of \"Aer\"");
        let aer = mk(&t.key, "Aer");
        let rival = mk(&t.key, "Brin");
        entities::update(
            &w.conn,
            &aer.id,
            EntityPatch {
                summary: Some("# Sky\nGod".into()),
                properties: Some(json!({
                    "domain": "sea", "symbol": "wave: \"~\"", "lore": "line1\nline2",
                    "power": 9.5, "active": true, "aspects": ["a", "b"], "born": 365,
                    "site": "https://example.com", "id": "clash",
                    "worshippers": [cult.id, "deleted-id"], "rival": rival.id,
                })),
                ..Default::default()
            },
        )
        .unwrap();
        let target = dir.path().join("out");
        fs::create_dir(&target).unwrap();
        let summary = run(&w, &target, &opts()).unwrap();
        let note = fs::read_to_string(summary.path.join("Deities/Aer.md")).unwrap();
        let (front, body) = front_matter(&note);
        assert_eq!(front["type"].as_str(), Some("Deity"));
        assert_eq!(front["type_key"].as_str(), Some("custom-deity"));
        assert_eq!(front["domain"].as_str(), Some("sea"));
        assert_eq!(front["symbol"].as_str(), Some("wave: \"~\""));
        assert_eq!(front["lore"].as_str(), Some("line1\nline2"));
        assert_eq!(front["power"].as_f64(), Some(9.5));
        assert_eq!(front["active"].as_bool(), Some(true));
        assert_eq!(front["aspects"][1].as_str(), Some("b"));
        assert!(front["born"].as_str().unwrap().contains("AF"));
        assert_eq!(front["born_day"].as_i64(), Some(365));
        assert_eq!(front["site"].as_str(), Some("https://example.com"));
        // A field named `id` does not overwrite the entity id.
        assert_eq!(front["field_id"].as_str(), Some("clash"));
        assert_eq!(front["id"].as_str(), Some(aer.id.as_str()));
        // Entity references become quoted wikilinks; dead ids are dropped.
        let cult_link = front["worshippers"][0].as_str().unwrap();
        assert_eq!(cult_link, "[[Cult- of -Aer-|Cult: of \"Aer\"]]");
        assert_eq!(front["worshippers"].as_sequence().unwrap().len(), 1);
        assert_eq!(front["rival"].as_str(), Some("[[Brin]]"));
        assert!(body.contains("\\# Sky\nGod"), "{body}");
        assert!(summary.path.join("Factions/Cult- of -Aer-.md").is_file());
    }

    #[test]
    fn refuses_existing_folder_and_overwrites_only_earlier_exports() {
        let dir = tempfile::tempdir().unwrap();
        let w = sample_world::create(dir.path()).unwrap();
        let target = dir.path().join("out");
        fs::create_dir(&target).unwrap();
        let first = run(&w, &target, &opts()).unwrap();
        fs::create_dir(first.path.join(".obsidian")).unwrap();
        fs::write(first.path.join(".obsidian/app.json"), "{}").unwrap();
        fs::write(first.path.join("stale.md"), "old").unwrap();

        // Second export without overwrite is refused and changes nothing.
        let again = run(&w, &target, &opts());
        assert!(matches!(again, Err(AppError::Validation(_))));
        assert!(first.path.join("stale.md").exists());

        // With overwrite the folder is replaced; obsidian settings survive.
        let replaced = run(
            &w,
            &target,
            &MarkdownExportOptions {
                overwrite: true,
                ..opts()
            },
        )
        .unwrap();
        assert!(!replaced.path.join("stale.md").exists());
        assert!(replaced.path.join(".obsidian/app.json").exists());
        assert!(replaced.path.join("README.md").exists());
        let leftovers = fs::read_dir(&target).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary folders are left behind");

        // A folder that is not an export is never replaced.
        let foreign = dir.path().join("out2");
        fs::create_dir_all(foreign.join("The Shattered Coast")).unwrap();
        fs::write(foreign.join("The Shattered Coast/precious.txt"), "keep").unwrap();
        let r = run(
            &w,
            &foreign,
            &MarkdownExportOptions {
                overwrite: true,
                ..opts()
            },
        );
        assert!(matches!(r, Err(AppError::Validation(_))));
        assert!(foreign.join("The Shattered Coast/precious.txt").exists());
        assert_eq!(fs::read_dir(&foreign).unwrap().count(), 1);
    }

    #[test]
    fn a_failing_export_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out");
        fs::create_dir(&target).unwrap();
        // No world.db in this folder: the export fails while opening it.
        let empty_world = dir.path().join("nothing");
        fs::create_dir(&empty_world).unwrap();
        let r = export_markdown(&empty_world, "Broken", &target, &opts(), &mut |_, _, _| {});
        assert!(r.is_err());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        // A missing target folder is a validation error.
        let r = export_markdown(
            &empty_world,
            "X",
            &dir.path().join("missing"),
            &opts(),
            &mut |_, _, _| {},
        );
        assert!(matches!(r, Err(AppError::Validation(_))));
    }
}
