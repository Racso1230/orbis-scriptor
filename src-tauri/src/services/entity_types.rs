//! Entity types: the built-in catalog merged with the user's own types and
//! extra fields.
//!
//! Storage (see migration 011):
//! - Custom types live in `custom_entity_types`; the type owns ALL of its
//!   fields (every one is `custom: true`).
//! - Built-in types keep their fixed fields in code. Extra user fields for a
//!   built-in type live in `type_templates` and are appended after the
//!   built-in ones (`custom: true`).
//!
//! Safety rules: removing a field or changing its kind never touches stored
//! `properties` (the values simply stop being shown); the key of a type or
//! field never changes after it was created.

use std::collections::HashSet;

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::entity_type::{
    self, DEFAULT_CUSTOM_COLOR, EntityTypeDef, FieldDef, FieldInput, FieldKind,
};
use crate::services::properties;

/// Most custom types a world may have.
pub const MAX_CUSTOM_TYPES: usize = 50;
/// Most user-defined fields on one type.
pub const MAX_FIELDS: usize = 40;
/// Longest label / plural / option text.
pub const MAX_LABEL_CHARS: usize = 40;
const MAX_OPTIONS: usize = 100;
const MAX_OPTION_CHARS: usize = 60;
/// Prefix of every custom type key.
pub const CUSTOM_PREFIX: &str = "custom-";

/// The icons a custom type may use: lucide icon names that the frontend maps
/// to components. The built-in types' icons are included.
const ICONS: &[&str] = &[
    "user",
    "users",
    "crown",
    "skull",
    "ghost",
    "flag",
    "sword",
    "swords",
    "shield",
    "castle",
    "tent",
    "church",
    "landmark",
    "map-pin",
    "map",
    "compass",
    "globe",
    "anchor",
    "ship",
    "mountain",
    "trees",
    "tree-pine",
    "flame",
    "droplets",
    "wind",
    "snowflake",
    "sun",
    "moon",
    "star",
    "sparkles",
    "book-open",
    "scroll",
    "bookmark",
    "sticky-note",
    "calendar-days",
    "gem",
    "coins",
    "key",
    "hammer",
    "flask-conical",
    "package",
    "box",
    "heart",
    "eye",
    "feather",
    "bird",
    "cat",
    "dog",
    "fish",
    "bug",
];

/// The allowed icon names, for the type editor's icon picker.
pub fn type_icons() -> Vec<String> {
    ICONS.iter().map(|s| (*s).to_owned()).collect()
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

// ---- reading -------------------------------------------------------------

/// A built-in type with the user's extra fields appended.
fn with_template(conn: &Connection, mut def: EntityTypeDef) -> AppResult<EntityTypeDef> {
    let extra = db::entity_types::get_template(conn, &def.key)?;
    def.fields.extend(extra.into_iter().map(mark_custom));
    Ok(def)
}

fn mark_custom(mut field: FieldDef) -> FieldDef {
    field.custom = true;
    field
}

/// Every type, in display order: built-ins (with their extra fields), then
/// custom types by their saved order.
pub fn merged(conn: &Connection) -> AppResult<Vec<EntityTypeDef>> {
    let templates = db::entity_types::list_templates(conn)?;
    let mut out = Vec::new();
    for mut def in entity_type::all() {
        if let Some((_, extra)) = templates.iter().find(|(k, _)| *k == def.key) {
            def.fields.extend(extra.iter().cloned().map(mark_custom));
        }
        out.push(def);
    }
    for mut def in db::entity_types::list_custom(conn)? {
        def.fields = def.fields.into_iter().map(mark_custom).collect();
        out.push(def);
    }
    Ok(out)
}

/// One type by key (built-in or custom), `None` when unknown.
pub fn find(conn: &Connection, key: &str) -> AppResult<Option<EntityTypeDef>> {
    if let Some(def) = entity_type::all().into_iter().find(|t| t.key == key) {
        return Ok(Some(with_template(conn, def)?));
    }
    Ok(db::entity_types::get_custom(conn, key)?.map(|mut def| {
        def.fields = def.fields.into_iter().map(mark_custom).collect();
        def
    }))
}

/// True when `key` is a built-in or custom type of this world.
pub fn is_known(conn: &Connection, key: &str) -> AppResult<bool> {
    Ok(entity_type::is_valid(key) || db::entity_types::get_custom(conn, key)?.is_some())
}

/// The fields of a type; empty for an unknown type.
pub fn fields_for(conn: &Connection, key: &str) -> AppResult<Vec<FieldDef>> {
    Ok(find(conn, key)?.map(|t| t.fields).unwrap_or_default())
}

fn require(conn: &Connection, key: &str) -> AppResult<EntityTypeDef> {
    find(conn, key)?.ok_or_else(|| AppError::NotFound(format!("Unknown entity type \"{key}\"")))
}

// ---- validation helpers --------------------------------------------------

fn clean_name(what: &str, text: &str) -> AppResult<String> {
    let t = text.trim();
    let n = t.chars().count();
    if n == 0 || n > MAX_LABEL_CHARS {
        return Err(AppError::Validation(format!(
            "{what} must be 1 to {MAX_LABEL_CHARS} characters"
        )));
    }
    Ok(t.to_owned())
}

fn check_icon(icon: &str) -> AppResult<String> {
    if ICONS.contains(&icon) {
        Ok(icon.to_owned())
    } else {
        Err(AppError::Validation(format!("Unknown icon \"{icon}\"")))
    }
}

/// Accepts `#rrggbb` (any case) and returns it lower-cased.
fn check_color(color: &str) -> AppResult<String> {
    let c = color.trim();
    let ok = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if ok {
        Ok(c.to_ascii_lowercase())
    } else {
        Err(AppError::Validation("Colour must look like #RRGGBB".into()))
    }
}

/// Labels (and plurals) must be unique across all types, ignoring case.
/// `except` is the type being edited, which may keep its own names.
fn check_names_free(
    all: &[EntityTypeDef],
    label: &str,
    plural: &str,
    except: Option<&str>,
) -> AppResult<()> {
    for t in all.iter().filter(|t| Some(t.key.as_str()) != except) {
        if t.label.to_lowercase() == label.to_lowercase() {
            return Err(AppError::Validation(format!(
                "A type called \"{}\" already exists",
                t.label
            )));
        }
        if t.plural.to_lowercase() == plural.to_lowercase() {
            return Err(AppError::Validation(format!(
                "A type with the plural \"{}\" already exists",
                t.plural
            )));
        }
    }
    Ok(())
}

/// Lower-case ASCII slug of `text` using `sep` between words, at most `max`
/// characters. May be empty (e.g. for a label with no Latin letters).
/// `chars()` iterates Unicode characters, not bytes.
fn slugify(text: &str, sep: char, max: usize) -> String {
    let mut out = String::new();
    let mut pending_sep = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push(sep);
            }
            pending_sep = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_sep = true;
        }
    }
    out.truncate(max);
    while out.ends_with(sep) {
        out.pop();
    }
    out
}

/// True for `[a-z][a-z0-9_]{0,39}`.
pub fn valid_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    first_ok
        && key.len() <= 40
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// A free field key derived from `label`: `domain`, then `domain_2`, ...
fn generate_field_key(label: &str, used: &HashSet<String>) -> String {
    let mut base = slugify(label, '_', 36);
    // Keys must start with a letter.
    if !base.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        base = format!("f_{base}");
        base.truncate(36);
    }
    let mut candidate = base.clone();
    let mut n = 2;
    while used.contains(&candidate) {
        candidate = format!("{base}_{n}");
        n += 1;
    }
    candidate
}

/// Validates the user's field list and turns it into stored form.
///
/// - `reserved`: keys that are not allowed (the built-in fields of the type).
/// - `known_types`: keys an entity-reference field may point at.
pub(crate) fn normalize_fields(
    inputs: Vec<FieldInput>,
    reserved: &HashSet<String>,
    known_types: &HashSet<String>,
) -> AppResult<Vec<FieldDef>> {
    if inputs.len() > MAX_FIELDS {
        return Err(AppError::Validation(format!(
            "A type can have at most {MAX_FIELDS} custom fields"
        )));
    }
    // Pass 1: keys the caller chose explicitly.
    let mut used: HashSet<String> = reserved.clone();
    let mut explicit: HashSet<String> = HashSet::new();
    for input in &inputs {
        if let Some(key) = &input.key {
            if !valid_field_key(key) {
                return Err(AppError::Validation(format!(
                    "Invalid field key \"{key}\" (use a-z, 0-9 and _, starting with a letter)"
                )));
            }
            if reserved.contains(key) {
                return Err(AppError::Validation(format!(
                    "The field key \"{key}\" is already used by a built-in field"
                )));
            }
            if !explicit.insert(key.clone()) {
                return Err(AppError::Validation(format!(
                    "The field key \"{key}\" is used twice"
                )));
            }
        }
    }
    used.extend(explicit);

    // Pass 2: build each field, generating keys for new ones.
    let mut out: Vec<FieldDef> = Vec::with_capacity(inputs.len());
    let mut labels: HashSet<String> = HashSet::new();
    for input in inputs {
        let label = clean_name("A field label", &input.label)?;
        if !labels.insert(label.to_lowercase()) {
            return Err(AppError::Validation(format!(
                "Two fields are called \"{label}\""
            )));
        }
        let key = match input.key {
            Some(k) => k,
            None => {
                let k = generate_field_key(&label, &used);
                used.insert(k.clone());
                k
            }
        };
        let (kind, options) = normalize_kind(&label, input.kind, input.options, known_types)?;
        out.push(FieldDef {
            key,
            label,
            kind,
            options,
            custom: true,
        });
    }
    Ok(out)
}

/// Checks the kind-specific settings: options for the select kinds, allowed
/// types for entity references.
fn normalize_kind(
    label: &str,
    kind: FieldKind,
    options: Vec<String>,
    known_types: &HashSet<String>,
) -> AppResult<(FieldKind, Vec<String>)> {
    match kind {
        FieldKind::Select | FieldKind::MultiSelect => {
            if options.is_empty() || options.len() > MAX_OPTIONS {
                return Err(AppError::Validation(format!(
                    "\"{label}\" needs 1 to {MAX_OPTIONS} options"
                )));
            }
            let mut clean: Vec<String> = Vec::new();
            for option in options {
                let o = option.trim().to_owned();
                let n = o.chars().count();
                if n == 0 || n > MAX_OPTION_CHARS {
                    return Err(AppError::Validation(format!(
                        "Options of \"{label}\" must be 1 to {MAX_OPTION_CHARS} characters"
                    )));
                }
                if clean.iter().any(|c| c.to_lowercase() == o.to_lowercase()) {
                    return Err(AppError::Validation(format!(
                        "\"{label}\" has the option \"{o}\" twice"
                    )));
                }
                clean.push(o);
            }
            Ok((kind, clean))
        }
        FieldKind::EntityRef {
            allowed_types,
            multiple,
        } => {
            let mut types: Vec<String> = Vec::new();
            for t in allowed_types {
                if !known_types.contains(&t) {
                    return Err(AppError::Validation(format!(
                        "\"{label}\" refers to the unknown type \"{t}\""
                    )));
                }
                if !types.contains(&t) {
                    types.push(t);
                }
            }
            Ok((
                FieldKind::EntityRef {
                    allowed_types: types,
                    multiple,
                },
                Vec::new(),
            ))
        }
        // Other kinds have no options.
        other => Ok((other, Vec::new())),
    }
}

pub(crate) fn known_type_keys(all: &[EntityTypeDef]) -> HashSet<String> {
    all.iter().map(|t| t.key.clone()).collect()
}

pub(crate) fn builtin_field_keys(type_key: &str) -> HashSet<String> {
    entity_type::all()
        .into_iter()
        .find(|t| t.key == type_key)
        .map(|t| t.fields.into_iter().map(|f| f.key).collect())
        .unwrap_or_default()
}

fn has_entity_ref(fields: &[FieldDef]) -> bool {
    fields
        .iter()
        .any(|f| matches!(f.kind, FieldKind::EntityRef { .. }))
}

// ---- writing -------------------------------------------------------------

/// Re-derives the `field_ref` links of every entity of a type. Needed when a
/// type's fields change, because links follow the field definitions.
fn resync_type(conn: &Connection, type_key: &str) -> AppResult<()> {
    let fields = fields_for(conn, type_key)?;
    for id in db::entity_types::entity_ids(conn, type_key)? {
        if let Some(entity) = db::entities::get(conn, &id)? {
            properties::sync_field_refs(conn, &entity.id, &fields, &entity.properties)?;
        }
    }
    Ok(())
}

/// Creates a custom type. The key is generated from the label
/// (`custom-<slug>`) and never changes afterwards.
pub fn create(
    conn: &Connection,
    label: &str,
    plural: &str,
    icon: &str,
    color: &str,
    fields: Vec<FieldInput>,
) -> AppResult<EntityTypeDef> {
    let def = prepare_new(conn, label, plural, icon, color, fields)?;
    let tx = conn.unchecked_transaction()?;
    let order = db::entity_types::next_sort_order(&tx)?;
    db::entity_types::insert_custom(&tx, &def, order, &now())?;
    tx.commit()?;
    Ok(def)
}

/// Writes an already validated custom type WITHOUT its own transaction
/// (the Markdown import creates types inside its one big transaction).
pub fn insert_prepared(conn: &Connection, def: &EntityTypeDef) -> AppResult<()> {
    let order = db::entity_types::next_sort_order(conn)?;
    db::entity_types::insert_custom(conn, def, order, &now())
}

/// Validates the input of a new custom type and builds it (including its
/// generated key) without writing anything. [`create`] and the import use it.
pub fn prepare_new(
    conn: &Connection,
    label: &str,
    plural: &str,
    icon: &str,
    color: &str,
    fields: Vec<FieldInput>,
) -> AppResult<EntityTypeDef> {
    let label = clean_name("The label", label)?;
    let plural = clean_name("The plural", plural)?;
    let icon = check_icon(icon)?;
    let color = if color.trim().is_empty() {
        DEFAULT_CUSTOM_COLOR.to_owned()
    } else {
        check_color(color)?
    };
    let all = merged(conn)?;
    let custom_count = all.iter().filter(|t| !t.builtin).count();
    if custom_count >= MAX_CUSTOM_TYPES {
        return Err(AppError::Validation(format!(
            "A world can have at most {MAX_CUSTOM_TYPES} custom types"
        )));
    }
    check_names_free(&all, &label, &plural, None)?;
    let fields = normalize_fields(fields, &HashSet::new(), &known_type_keys(&all))?;

    // Key: custom-<slug>, with -2, -3 ... when the slug is taken. A label
    // with no Latin letters falls back to a few characters of a UUID.
    let mut slug = slugify(&label, '-', 30);
    if slug.is_empty() {
        slug = Uuid::now_v7().simple().to_string()[20..28].to_owned();
    }
    let taken: HashSet<String> = known_type_keys(&all);
    let mut key = format!("{CUSTOM_PREFIX}{slug}");
    let mut n = 2;
    while taken.contains(&key) {
        key = format!("{CUSTOM_PREFIX}{slug}-{n}");
        n += 1;
    }

    Ok(EntityTypeDef {
        key,
        label,
        plural,
        icon,
        color,
        builtin: false,
        fields,
    })
}

/// Edits a custom type; only the `Some` arguments change. When `fields` is
/// given it REPLACES the field list: send existing fields with their keys.
/// Removed fields keep their stored values on entities (they just stop being
/// shown); a changed kind keeps the old value too, which may no longer fit
/// the new kind and is then treated as "not set" by the forms.
pub fn update(
    conn: &Connection,
    key: &str,
    label: Option<&str>,
    plural: Option<&str>,
    icon: Option<&str>,
    color: Option<&str>,
    fields: Option<Vec<FieldInput>>,
) -> AppResult<EntityTypeDef> {
    if entity_type::is_valid(key) {
        return Err(AppError::Validation(
            "Built-in types cannot be edited; use set_type_template_fields to add fields".into(),
        ));
    }
    let mut def = db::entity_types::get_custom(conn, key)?
        .ok_or_else(|| AppError::NotFound(format!("Unknown entity type \"{key}\"")))?;
    let all = merged(conn)?;
    if let Some(l) = label {
        def.label = clean_name("The label", l)?;
    }
    if let Some(p) = plural {
        def.plural = clean_name("The plural", p)?;
    }
    if let Some(i) = icon {
        def.icon = check_icon(i)?;
    }
    if let Some(c) = color {
        def.color = check_color(c)?;
    }
    check_names_free(&all, &def.label, &def.plural, Some(key))?;
    let fields_changed = fields.is_some();
    let had_refs = has_entity_ref(&def.fields);
    if let Some(inputs) = fields {
        def.fields = normalize_fields(inputs, &HashSet::new(), &known_type_keys(&all))?;
    }

    let tx = conn.unchecked_transaction()?;
    db::entity_types::update_custom(&tx, &def, &now())?;
    if fields_changed && (had_refs || has_entity_ref(&def.fields)) {
        resync_type(&tx, key)?;
    }
    tx.commit()?;
    Ok(def)
}

/// Sets the extra user fields of a type. For a built-in type they are stored
/// in `type_templates` after the fixed fields; for a custom type this is the
/// same as replacing its field list with [`update`].
pub fn set_template_fields(
    conn: &Connection,
    type_key: &str,
    fields: Vec<FieldInput>,
) -> AppResult<EntityTypeDef> {
    if !entity_type::is_valid(type_key) {
        require(conn, type_key)?;
        return update(conn, type_key, None, None, None, None, Some(fields));
    }
    let all = merged(conn)?;
    let normalized = normalize_fields(
        fields,
        &builtin_field_keys(type_key),
        &known_type_keys(&all),
    )?;
    let had_refs = has_entity_ref(&db::entity_types::get_template(conn, type_key)?);
    let tx = conn.unchecked_transaction()?;
    db::entity_types::set_template(&tx, type_key, &normalized)?;
    if had_refs || has_entity_ref(&normalized) {
        resync_type(&tx, type_key)?;
    }
    tx.commit()?;
    require(conn, type_key)
}

/// Deletes a custom type. If entities of the type exist, `reassign_to` must
/// name another type; they are converted in the same transaction and keep
/// their properties, lore and links. Fields of other types that listed the
/// deleted type in `allowed_types` are updated to the new type (or lose that
/// entry; an emptied list means "any type").
pub fn delete(conn: &Connection, key: &str, reassign_to: Option<&str>) -> AppResult<()> {
    if entity_type::is_valid(key) {
        return Err(AppError::Validation(
            "Built-in types cannot be deleted".into(),
        ));
    }
    db::entity_types::get_custom(conn, key)?
        .ok_or_else(|| AppError::NotFound(format!("Unknown entity type \"{key}\"")))?;
    let count = db::entity_types::count_entities(conn, key)?;

    if let Some(target) = reassign_to {
        if target == key {
            return Err(AppError::Validation(
                "Choose a different type to move the entities to".into(),
            ));
        }
        if !is_known(conn, target)? {
            return Err(AppError::NotFound(format!(
                "Unknown entity type \"{target}\""
            )));
        }
        // Maps and events have extra rows that other types do not have.
        if count > 0 && (target == "map" || target == "event") {
            return Err(AppError::Validation(
                "Entities cannot be converted to maps or events".into(),
            ));
        }
    } else if count > 0 {
        return Err(AppError::Validation(format!(
            "{count} entities use this type. Choose a type to move them to first."
        )));
    }

    let tx = conn.unchecked_transaction()?;
    if let Some(target) = reassign_to {
        db::entity_types::reassign_entities(&tx, key, target)?;
        // The new type may define different fields: re-derive the links.
        resync_type(&tx, target)?;
    }
    scrub_allowed_types(&tx, key, reassign_to)?;
    db::entity_types::delete_custom(&tx, key)?;
    tx.commit()?;
    Ok(())
}

/// Replaces `old` in every entity-reference field's `allowed_types` with
/// `replacement` (or drops it when there is none).
fn scrub_allowed_types(conn: &Connection, old: &str, replacement: Option<&str>) -> AppResult<()> {
    // Returns the fixed list and whether anything changed.
    let fix = |fields: &[FieldDef]| -> (Vec<FieldDef>, bool) {
        let mut changed = false;
        let fixed = fields
            .iter()
            .cloned()
            .map(|mut f| {
                if let FieldKind::EntityRef {
                    allowed_types,
                    multiple,
                } = &f.kind
                    && allowed_types.iter().any(|t| t == old)
                {
                    let mut next: Vec<String> = Vec::new();
                    for t in allowed_types {
                        let t = if t == old {
                            match replacement {
                                Some(r) => r.to_owned(),
                                None => continue,
                            }
                        } else {
                            t.clone()
                        };
                        if !next.contains(&t) {
                            next.push(t);
                        }
                    }
                    f.kind = FieldKind::EntityRef {
                        allowed_types: next,
                        multiple: *multiple,
                    };
                    changed = true;
                }
                f
            })
            .collect();
        (fixed, changed)
    };
    for mut def in db::entity_types::list_custom(conn)? {
        if def.key == old {
            continue;
        }
        let (fixed, changed) = fix(&def.fields);
        if changed {
            def.fields = fixed;
            db::entity_types::update_custom(conn, &def, &now())?;
        }
    }
    for (type_key, fields) in db::entity_types::list_templates(conn)? {
        let (fixed, changed) = fix(&fields);
        if changed {
            db::entity_types::set_template(conn, &type_key, &fixed)?;
        }
    }
    Ok(())
}

/// Sets the display order of the custom types. `keys` must list every
/// custom type exactly once.
pub fn reorder(conn: &Connection, keys: &[String]) -> AppResult<Vec<EntityTypeDef>> {
    let existing: Vec<String> = db::entity_types::list_custom(conn)?
        .into_iter()
        .map(|t| t.key)
        .collect();
    let given: HashSet<&String> = keys.iter().collect();
    let same = keys.len() == existing.len()
        && given.len() == keys.len()
        && existing.iter().all(|k| given.contains(k));
    if !same {
        return Err(AppError::Validation(
            "Reorder needs every custom type exactly once".into(),
        ));
    }
    let tx = conn.unchecked_transaction()?;
    for (i, key) in keys.iter().enumerate() {
        db::entity_types::set_sort_order(&tx, key, i as i64)?;
    }
    tx.commit()?;
    merged(conn)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::{EntityPatch, NewEntity};
    use crate::models::link::KIND_FIELD_REF;
    use crate::services::entities;
    use serde_json::json;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    pub fn input(label: &str, kind: FieldKind) -> FieldInput {
        FieldInput {
            key: None,
            label: label.into(),
            kind,
            options: vec![],
        }
    }

    fn select(label: &str, options: &[&str]) -> FieldInput {
        FieldInput {
            options: options.iter().map(|o| (*o).to_owned()).collect(),
            ..input(label, FieldKind::Select)
        }
    }

    fn deity(c: &Connection) -> EntityTypeDef {
        create(
            c,
            "Deity",
            "Deities",
            "sparkles",
            "#AA3355",
            vec![
                select("Domain", &["war", "sea"]),
                input("Symbol", FieldKind::Text),
                input(
                    "Worshippers",
                    FieldKind::EntityRef {
                        allowed_types: vec!["faction".into()],
                        multiple: true,
                    },
                ),
            ],
        )
        .unwrap()
    }

    fn new_entity(c: &Connection, t: &str, name: &str) -> String {
        entities::create(
            c,
            NewEntity {
                entity_type: t.into(),
                name: name.into(),
            },
        )
        .unwrap()
        .id
    }

    fn is_validation<T>(r: AppResult<T>) -> bool {
        matches!(r, Err(AppError::Validation(_)))
    }

    #[test]
    fn merged_lists_builtins_extras_then_customs() {
        let c = conn();
        let plain = merged(&c).unwrap();
        assert_eq!(plain.len(), 7);
        assert!(plain.iter().all(|t| t.builtin));

        let d = deity(&c);
        assert_eq!(d.key, "custom-deity");
        assert_eq!(d.color, "#aa3355");
        assert_eq!(d.fields[0].key, "domain");
        assert!(d.fields.iter().all(|f| f.custom));

        set_template_fields(&c, "character", vec![input("Birthday", FieldKind::Date)]).unwrap();
        let all = merged(&c).unwrap();
        assert_eq!(all.len(), 8);
        assert_eq!(all[7].key, "custom-deity");
        assert!(!all[7].builtin);
        let ch = &all[0];
        assert_eq!(ch.key, "character");
        let flags: Vec<(&str, bool)> = ch
            .fields
            .iter()
            .map(|f| (f.key.as_str(), f.custom))
            .collect();
        assert_eq!(
            flags,
            vec![
                ("species", false),
                ("age", false),
                ("status", false),
                ("birthday", true)
            ]
        );
        assert_eq!(find(&c, "character").unwrap().unwrap(), *ch);
        assert!(is_known(&c, "custom-deity").unwrap());
        assert!(!is_known(&c, "dragon").unwrap());
    }

    #[test]
    fn create_validates_names_icon_colour_and_limits() {
        let c = conn();
        let ok = |l: &str, p: &str| create(&c, l, p, "star", "#112233", vec![]);
        assert!(is_validation(ok("", "Xs")));
        assert!(is_validation(ok("X", "")));
        assert!(is_validation(ok(&"x".repeat(41), "Xs")));
        // Unique across built-ins and customs, ignoring case.
        assert!(is_validation(ok("character", "Folks")));
        assert!(is_validation(ok("Folk", "characters")));
        ok("Folk", "Folks").unwrap();
        assert!(is_validation(ok("FOLK", "Other")));
        // Icon and colour.
        assert!(is_validation(create(
            &c,
            "A",
            "As",
            "not-an-icon",
            "#112233",
            vec![]
        )));
        for bad in ["red", "#12345", "#12345g", "112233", "#1122334"] {
            assert!(
                is_validation(create(&c, "B", "Bs", "star", bad, vec![])),
                "{bad}"
            );
        }
        // An empty colour falls back to the default.
        let d = create(&c, "C", "Cs", "star", "", vec![]).unwrap();
        assert_eq!(d.color, DEFAULT_CUSTOM_COLOR);

        // At most 50 custom types (3 exist already).
        for i in 0..MAX_CUSTOM_TYPES - 2 {
            ok(&format!("T{i}"), &format!("Ts{i}")).unwrap();
        }
        assert!(is_validation(ok("One too many", "Too many")));
    }

    #[test]
    fn keys_are_generated_unique_and_immutable() {
        let c = conn();
        let a = create(&c, "Sea Folk!", "Sea Folks", "fish", "#112233", vec![]).unwrap();
        assert_eq!(a.key, "custom-sea-folk");
        // A label with no Latin letters still gets a key.
        let b = create(&c, "神", "神々", "star", "#112233", vec![]).unwrap();
        assert!(b.key.starts_with("custom-") && b.key.len() > 8);
        let u = update(
            &c,
            &a.key,
            Some("Marine Folk"),
            Some("Marine Folks"),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            (u.key.as_str(), u.label.as_str()),
            ("custom-sea-folk", "Marine Folk")
        );
        // Same slug as an existing key -> suffix.
        let c2 = create(&c, "sea-folk", "Sea-Folk plural", "fish", "#112233", vec![]).unwrap();
        assert_eq!(c2.key, "custom-sea-folk-2");
    }

    #[test]
    fn field_validation_rules() {
        let c = conn();
        let mk = |fields: Vec<FieldInput>| create(&c, "T", "Ts", "star", "#112233", fields);
        // Select needs options; duplicates and empties are refused.
        assert!(is_validation(mk(vec![input("S", FieldKind::Select)])));
        assert!(is_validation(mk(vec![select("S", &["a", "A"])])));
        assert!(is_validation(mk(vec![select("S", &["  "])])));
        // Duplicate labels, bad explicit keys, duplicate explicit keys.
        assert!(is_validation(mk(vec![
            input("X", FieldKind::Text),
            input("x", FieldKind::Text)
        ])));
        let long_key = "a".repeat(41);
        for key in ["Bad", "1abc", "has-dash", long_key.as_str(), ""] {
            let f = FieldInput {
                key: Some(key.into()),
                ..input("X", FieldKind::Text)
            };
            assert!(is_validation(mk(vec![f])), "{key}");
        }
        let dup = |l: &str| FieldInput {
            key: Some("same".into()),
            ..input(l, FieldKind::Text)
        };
        assert!(is_validation(mk(vec![dup("A"), dup("B")])));
        // Unknown allowed type.
        let r = input(
            "R",
            FieldKind::EntityRef {
                allowed_types: vec!["dragon".into()],
                multiple: false,
            },
        );
        assert!(is_validation(mk(vec![r])));
        // Too many fields.
        let many: Vec<FieldInput> = (0..=MAX_FIELDS)
            .map(|i| input(&format!("F{i}"), FieldKind::Text))
            .collect();
        assert!(is_validation(mk(many)));
        let max: Vec<FieldInput> = (0..MAX_FIELDS)
            .map(|i| input(&format!("F{i}"), FieldKind::Text))
            .collect();
        mk(max).unwrap();
    }

    #[test]
    fn generated_field_keys_avoid_collisions() {
        let c = conn();
        // "Kind" collides with the built-in `kind` of locations.
        let t = set_template_fields(
            &c,
            "location",
            vec![
                input("Kind", FieldKind::Text),
                input("Region!", FieldKind::Text),
                input("1st", FieldKind::Text),
            ],
        )
        .unwrap();
        let keys: Vec<&str> = t.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["kind", "region", "kind_2", "region_2", "f_1st"]);
        // An explicit key that is built in is refused.
        let f = FieldInput {
            key: Some("region".into()),
            ..input("Mine", FieldKind::Text)
        };
        assert!(is_validation(set_template_fields(&c, "location", vec![f])));
    }

    #[test]
    fn existing_field_keys_survive_edits_and_removed_values_are_kept() {
        let c = conn();
        let d = deity(&c);
        let id = new_entity(&c, &d.key, "Aer");
        entities::update(
            &c,
            &id,
            EntityPatch {
                properties: Some(json!({ "domain": "sea", "symbol": "wave" })),
                ..Default::default()
            },
        )
        .unwrap();
        // Rename "Symbol" (keeping its key), drop "Domain", change nothing else.
        let keep = FieldInput {
            key: Some("symbol".into()),
            ..input("Emblem", FieldKind::Text)
        };
        let u = update(&c, &d.key, None, None, None, None, Some(vec![keep])).unwrap();
        assert_eq!(u.fields.len(), 1);
        assert_eq!(
            (u.fields[0].key.as_str(), u.fields[0].label.as_str()),
            ("symbol", "Emblem")
        );
        // Both stored values are still there.
        let e = entities::get(&c, &id).unwrap();
        assert_eq!(e.properties, json!({ "domain": "sea", "symbol": "wave" }));
    }

    #[test]
    fn built_in_types_cannot_be_edited_or_deleted_and_unknown_is_not_found() {
        let c = conn();
        assert!(is_validation(update(
            &c,
            "character",
            Some("Person"),
            None,
            None,
            None,
            None
        )));
        assert!(is_validation(delete(&c, "character", None)));
        assert!(matches!(
            delete(&c, "custom-nope", None),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            set_template_fields(&c, "custom-nope", vec![]),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            update(&c, "custom-nope", None, None, None, None, None),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn delete_needs_a_target_when_entities_exist_and_reassign_works() {
        let c = conn();
        let d = deity(&c);
        let a = new_entity(&c, &d.key, "Aer");
        let f = new_entity(&c, "faction", "Cult");
        entities::update(
            &c,
            &a,
            EntityPatch {
                properties: Some(json!({ "domain": "war", "worshippers": [f] })),
                summary: Some("Sky god".into()),
                ..Default::default()
            },
        )
        .unwrap();
        // Another custom type refers to deities.
        let other = create(
            &c,
            "Shrine",
            "Shrines",
            "church",
            "#112233",
            vec![input(
                "Patron",
                FieldKind::EntityRef {
                    allowed_types: vec![d.key.clone(), "faction".into()],
                    multiple: false,
                },
            )],
        )
        .unwrap();

        assert!(is_validation(delete(&c, &d.key, None)));
        assert!(is_validation(delete(&c, &d.key, Some(&d.key))));
        assert!(is_validation(delete(&c, &d.key, Some("map"))));
        assert!(matches!(
            delete(&c, &d.key, Some("dragon")),
            Err(AppError::NotFound(_))
        ));
        // Nothing changed after the refusals.
        assert!(find(&c, &d.key).unwrap().is_some());

        delete(&c, &d.key, Some("note")).unwrap();
        assert!(find(&c, &d.key).unwrap().is_none());
        let e = entities::get(&c, &a).unwrap();
        assert_eq!(e.entity_type, "note");
        assert_eq!(e.summary, "Sky god");
        // Properties are kept; the note type has no fields so no links remain.
        assert_eq!(e.properties["domain"], "war");
        assert!(
            db::links::targets_for_source(&c, &a, KIND_FIELD_REF)
                .unwrap()
                .is_empty()
        );
        // The other type's allowed list now names the new type.
        let s = find(&c, &other.key).unwrap().unwrap();
        assert_eq!(
            s.fields[0].kind,
            FieldKind::EntityRef {
                allowed_types: vec!["note".into(), "faction".into()],
                multiple: false
            }
        );
    }

    #[test]
    fn deleting_an_empty_type_needs_no_target_and_scrubs_references() {
        let c = conn();
        let d = deity(&c);
        let other = create(
            &c,
            "Shrine",
            "Shrines",
            "church",
            "#112233",
            vec![input(
                "Patron",
                FieldKind::EntityRef {
                    allowed_types: vec![d.key.clone()],
                    multiple: false,
                },
            )],
        )
        .unwrap();
        set_template_fields(
            &c,
            "item",
            vec![input(
                "Made by",
                FieldKind::EntityRef {
                    allowed_types: vec![d.key.clone()],
                    multiple: false,
                },
            )],
        )
        .unwrap();
        delete(&c, &d.key, None).unwrap();
        // An emptied list means "any type".
        for key in [other.key.as_str(), "item"] {
            let t = find(&c, key).unwrap().unwrap();
            let f = t.fields.last().unwrap();
            assert_eq!(
                f.kind,
                FieldKind::EntityRef {
                    allowed_types: vec![],
                    multiple: false
                }
            );
        }
    }

    #[test]
    fn reorder_requires_every_custom_type_once() {
        let c = conn();
        let a = create(&c, "A", "As", "star", "#112233", vec![]).unwrap();
        let b = create(&c, "B", "Bs", "star", "#112233", vec![]).unwrap();
        let out = reorder(&c, &[b.key.clone(), a.key.clone()]).unwrap();
        let keys: Vec<&str> = out.iter().skip(7).map(|t| t.key.as_str()).collect();
        assert_eq!(keys, vec![b.key.as_str(), a.key.as_str()]);
        assert!(is_validation(reorder(&c, std::slice::from_ref(&a.key))));
        assert!(is_validation(reorder(&c, &[a.key.clone(), a.key.clone()])));
        assert!(is_validation(reorder(
            &c,
            &[a.key.clone(), "custom-x".into()]
        )));
    }

    #[test]
    fn icon_list_has_the_builtin_icons() {
        let icons = type_icons();
        for t in entity_type::all() {
            assert!(icons.contains(&t.icon), "{}", t.icon);
        }
        let unique: HashSet<&String> = icons.iter().collect();
        assert_eq!(unique.len(), icons.len());
    }
}
