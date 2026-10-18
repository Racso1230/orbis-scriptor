//! Tags: free-form labels on entities.
//!
//! Rules:
//! - A tag name is trimmed and has 1 to 40 characters: letters, digits, `-`,
//!   `_`, spaces and `/`. `/` nests tags the way Obsidian does
//!   (`places/cities`); a nested name must not have an empty part, so
//!   `a//b`, `/a` and `a/` are refused. Runs of spaces become one space and
//!   spaces around a `/` are removed.
//! - Names are unique ignoring case. Putting a tag on an entity with a name
//!   that already exists in another case uses the existing tag (and its
//!   spelling).
//! - An entity can have at most [`MAX_TAGS_PER_ENTITY`] tags.
//! - Filtering by a tag also finds the entities of its nested tags:
//!   `places` matches `places` and `places/cities`.
//! - Renaming a tag to a name another tag already has MERGES the two: the
//!   entities move to the existing tag (keeping its colour) and the renamed
//!   tag is deleted. Only the tag itself is renamed, not nested children.
//! - Changing the tags of an entity counts as an edit (`updated_at`), and
//!   the search index is kept in step.

use std::collections::HashMap;

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::entity::EntitySummary;
use crate::models::tag::Tag;
use crate::services::search;

/// Longest tag name, in characters.
pub const MAX_TAG_CHARS: usize = 40;
/// Most tags one entity may carry.
pub const MAX_TAGS_PER_ENTITY: usize = 50;

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn allowed_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '/')
}

/// Validates and tidies a tag name (see the module docs for the rules).
pub fn normalize_name(raw: &str) -> AppResult<String> {
    let bad = |why: &str| AppError::Validation(format!("Tag \"{}\": {why}", raw.trim()));
    if let Some(c) = raw
        .chars()
        .find(|c| !allowed_char(*c) && !c.is_whitespace())
    {
        return Err(bad(&format!(
            "the character \"{c}\" is not allowed (use letters, digits, - _ space and /)"
        )));
    }
    // `split_whitespace().join(" ")` makes every run of whitespace one space.
    let mut parts: Vec<String> = Vec::new();
    for part in raw.split('/') {
        let tidy = part.split_whitespace().collect::<Vec<_>>().join(" ");
        if tidy.is_empty() {
            return Err(if raw.trim().is_empty() {
                AppError::Validation("A tag name cannot be empty".into())
            } else {
                bad("a nested tag cannot have an empty part")
            });
        }
        parts.push(tidy);
    }
    let name = parts.join("/");
    if name.chars().count() > MAX_TAG_CHARS {
        return Err(bad(&format!(
            "a tag can have at most {MAX_TAG_CHARS} characters"
        )));
    }
    Ok(name)
}

/// `#rrggbb` (any case), returned lower-case.
fn check_color(color: &str) -> AppResult<String> {
    let c = color.trim();
    if c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(c.to_ascii_lowercase())
    } else {
        Err(AppError::Validation("Colour must look like #RRGGBB".into()))
    }
}

pub fn list(conn: &Connection) -> AppResult<Vec<Tag>> {
    db::tags::list(conn)
}

fn require(conn: &Connection, id: &str) -> AppResult<Tag> {
    db::tags::get(conn, id)?.ok_or_else(|| AppError::NotFound(format!("Tag {id} does not exist")))
}

/// Lower-cased name -> tag id, for case-insensitive lookups. (SQLite's own
/// NOCASE only folds ASCII, so we compare lower-cased Unicode text here.)
pub fn load_cache(conn: &Connection) -> AppResult<HashMap<String, String>> {
    Ok(db::tags::list(conn)?
        .into_iter()
        .map(|t| (t.name.to_lowercase(), t.id))
        .collect())
}

/// Sets the tags of an entity to exactly `names` WITHOUT its own
/// transaction, search update or `updated_at` change; the caller does those
/// (the import writes thousands of entities in one transaction). `names`
/// must already be normalised. Missing tags are created; `cache` (see
/// [`load_cache`]) is kept up to date.
pub fn apply_entity_tags(
    conn: &Connection,
    entity_id: &str,
    names: &[String],
    cache: &mut HashMap<String, String>,
) -> AppResult<()> {
    let mut ids: Vec<String> = Vec::new();
    for name in names {
        let key = name.to_lowercase();
        let id = match cache.get(&key) {
            Some(id) => id.clone(),
            None => {
                let id = Uuid::now_v7().to_string();
                db::tags::insert(conn, &id, name, &now())?;
                cache.insert(key, id.clone());
                id
            }
        };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    db::tags::replace_for_entity(conn, entity_id, &ids)
}

/// Validates a list of names, dropping duplicates (ignoring case).
pub fn normalize_all(names: &[String]) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for raw in names {
        let name = normalize_name(raw)?;
        if !out.iter().any(|n| n.to_lowercase() == name.to_lowercase()) {
            out.push(name);
        }
    }
    if out.len() > MAX_TAGS_PER_ENTITY {
        return Err(AppError::Validation(format!(
            "An entity can have at most {MAX_TAGS_PER_ENTITY} tags"
        )));
    }
    Ok(out)
}

/// Replaces the tags of an entity with `names` (creating tags that do not
/// exist yet) in one transaction. Returns the entity's tag names, sorted.
pub fn set_entity_tags(
    conn: &Connection,
    entity_id: &str,
    names: &[String],
) -> AppResult<Vec<String>> {
    if db::entities::get(conn, entity_id)?.is_none() {
        return Err(AppError::NotFound(format!(
            "Entity {entity_id} does not exist"
        )));
    }
    let names = normalize_all(names)?;
    let tx = conn.unchecked_transaction()?;
    let mut cache = load_cache(&tx)?;
    apply_entity_tags(&tx, entity_id, &names, &mut cache)?;
    db::entities::touch(&tx, entity_id, &now())?;
    search::reindex_entity(&tx, entity_id)?;
    let result = db::tags::names_for_entity(&tx, entity_id)?;
    tx.commit()?;
    Ok(result)
}

/// Re-indexes every entity that carries `tag_id` (after a rename, merge or
/// delete changed what their `tags` search column should say).
fn reindex_tag_users(conn: &Connection, entity_ids: &[String]) -> AppResult<()> {
    for id in entity_ids {
        search::reindex_entity(conn, id)?;
    }
    Ok(())
}

/// Renames a tag, or MERGES it into the existing tag that already has the
/// new name (ignoring case). Returns the surviving tag.
pub fn rename(conn: &Connection, id: &str, new_name: &str) -> AppResult<Tag> {
    require(conn, id)?;
    let name = normalize_name(new_name)?;
    let other = db::tags::list(conn)?
        .into_iter()
        .find(|t| t.id != id && t.name.to_lowercase() == name.to_lowercase());
    let tx = conn.unchecked_transaction()?;
    let affected = db::tags::entity_ids_for_tag(&tx, id)?;
    let survivor = match other {
        Some(target) => {
            db::tags::merge(&tx, id, &target.id)?;
            target.id
        }
        None => {
            db::tags::rename(&tx, id, &name)?;
            id.to_owned()
        }
    };
    reindex_tag_users(&tx, &affected)?;
    let tag = require(&tx, &survivor)?;
    tx.commit()?;
    Ok(tag)
}

/// Deletes a tag; its entities simply lose it.
pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    require(conn, id)?;
    let tx = conn.unchecked_transaction()?;
    let affected = db::tags::entity_ids_for_tag(&tx, id)?;
    db::tags::delete(&tx, id)?;
    reindex_tag_users(&tx, &affected)?;
    tx.commit()?;
    Ok(())
}

/// Sets or clears (`None`) the colour of a tag.
pub fn set_color(conn: &Connection, id: &str, color: Option<&str>) -> AppResult<Tag> {
    require(conn, id)?;
    let color = color.map(check_color).transpose()?;
    db::tags::set_color(conn, id, color.as_deref())?;
    require(conn, id)
}

/// The ids of the tag called `name` (ignoring case, a leading `#` is
/// allowed) AND of its nested tags (`name/...`). Empty when no tag matches.
pub fn tag_group(all: &[Tag], name: &str) -> Vec<String> {
    let wanted = name
        .trim()
        .trim_start_matches('#')
        .trim_end_matches('/')
        .split('/')
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase();
    if wanted.is_empty() {
        return Vec::new();
    }
    let nested_prefix = format!("{wanted}/");
    all.iter()
        .filter(|t| {
            let lower = t.name.to_lowercase();
            lower == wanted || lower.starts_with(&nested_prefix)
        })
        .map(|t| t.id.clone())
        .collect()
}

/// Entities that carry the given tags (nested tags included), sorted by
/// type then name. `match_all` = must have every tag, otherwise any one.
/// Names that match no tag are ignored in "any" mode and make the result
/// empty in "all" mode. No names = no entities.
pub fn list_entities_by_tag(
    conn: &Connection,
    names: &[String],
    match_all: bool,
) -> AppResult<Vec<EntitySummary>> {
    let all = db::tags::list(conn)?;
    let mut result: Option<Vec<String>> = None;
    for name in names {
        let group = tag_group(&all, name);
        let ids = db::tags::entity_ids_for_tags(conn, &group)?;
        result = Some(match result {
            None => ids,
            Some(prev) if match_all => prev.into_iter().filter(|i| ids.contains(i)).collect(),
            Some(mut prev) => {
                for id in ids {
                    if !prev.contains(&id) {
                        prev.push(id);
                    }
                }
                prev
            }
        });
    }
    let ids = result.unwrap_or_default();
    let mut found = db::entities::summaries_by_ids(conn, &ids)?;
    found.sort_by_cached_key(|e| (e.entity_type.clone(), e.name.to_lowercase()));
    Ok(found)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::{EntityPatch, NewEntity};
    use crate::services::entities;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    fn make(c: &Connection, name: &str) -> String {
        entities::create(
            c,
            NewEntity {
                entity_type: "note".into(),
                name: name.into(),
            },
        )
        .unwrap()
        .id
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn tag_named(c: &Connection, name: &str) -> Tag {
        list(c)
            .unwrap()
            .into_iter()
            .find(|t| t.name == name)
            .unwrap()
    }

    #[test]
    fn names_are_trimmed_and_tidied() {
        assert_eq!(normalize_name("  royalty ").unwrap(), "royalty");
        assert_eq!(normalize_name("dark   age").unwrap(), "dark age");
        assert_eq!(normalize_name("places / cities").unwrap(), "places/cities");
        assert_eq!(normalize_name("Ünï-cøde_1").unwrap(), "Ünï-cøde_1");
    }

    #[test]
    fn bad_names_are_refused() {
        for bad in [
            "",
            "   ",
            "a//b",
            "/a",
            "a/",
            "a#b",
            "#tag",
            "a.b",
            "a,b",
            "emoji🐉",
            "a:b",
        ] {
            assert!(
                matches!(normalize_name(bad), Err(AppError::Validation(_))),
                "{bad:?} should be refused"
            );
        }
        assert!(normalize_name(&"x".repeat(40)).is_ok());
        assert!(normalize_name(&"x".repeat(41)).is_err());
    }

    #[test]
    fn set_entity_tags_creates_and_replaces() {
        let c = conn();
        let a = make(&c, "A");
        let b = make(&c, "B");
        let got = set_entity_tags(&c, &a, &names(&["royalty", "places/cities"])).unwrap();
        assert_eq!(got, vec!["places/cities", "royalty"]);
        set_entity_tags(&c, &b, &names(&["Royalty"])).unwrap();
        // Same tag (case-insensitive), the first spelling wins.
        assert_eq!(list(&c).unwrap().len(), 2);
        assert_eq!(tag_named(&c, "royalty").count, 2);

        // Replace: "royalty" goes away from A, "coast" is added.
        let got = set_entity_tags(&c, &a, &names(&["coast", "COAST", "Places/Cities"])).unwrap();
        assert_eq!(got, vec!["coast", "places/cities"]);
        assert_eq!(tag_named(&c, "royalty").count, 1);
        assert_eq!(entities::get(&c, &a).unwrap().tags, got);

        // Clearing leaves the tag rows in place (unused tags stay listed).
        set_entity_tags(&c, &a, &[]).unwrap();
        assert!(entities::get(&c, &a).unwrap().tags.is_empty());
        assert_eq!(tag_named(&c, "coast").count, 0);
    }

    #[test]
    fn invalid_name_changes_nothing() {
        let c = conn();
        let a = make(&c, "A");
        set_entity_tags(&c, &a, &names(&["ok"])).unwrap();
        assert!(set_entity_tags(&c, &a, &names(&["fine", "not/ok/"])).is_err());
        assert_eq!(entities::get(&c, &a).unwrap().tags, vec!["ok"]);
        assert_eq!(list(&c).unwrap().len(), 1);
        assert!(matches!(
            set_entity_tags(&c, "missing", &names(&["x"])),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn too_many_tags_are_refused() {
        let c = conn();
        let a = make(&c, "A");
        let many: Vec<String> = (0..=MAX_TAGS_PER_ENTITY).map(|i| format!("t{i}")).collect();
        assert!(matches!(
            set_entity_tags(&c, &a, &many),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn rename_changes_the_name_and_merge_moves_entities() {
        let c = conn();
        let a = make(&c, "A");
        let b = make(&c, "B");
        set_entity_tags(&c, &a, &names(&["old", "keep"])).unwrap();
        set_entity_tags(&c, &b, &names(&["new", "old"])).unwrap();
        let old = tag_named(&c, "old");
        set_color(&c, &old.id, Some("#AABBCC")).unwrap();

        // Plain rename (also just changing the case of the same tag).
        let keep = tag_named(&c, "keep");
        assert_eq!(rename(&c, &keep.id, "Kept").unwrap().name, "Kept");
        assert_eq!(rename(&c, &keep.id, "kept").unwrap().name, "kept");

        // Renaming "old" to "NEW" merges it into the existing "new".
        let merged = rename(&c, &old.id, "NEW").unwrap();
        assert_eq!(merged.name, "new");
        assert_eq!(merged.count, 2);
        assert_eq!(merged.color, None); // the surviving tag keeps ITS colour
        assert!(list(&c).unwrap().iter().all(|t| t.name != "old"));
        assert_eq!(entities::get(&c, &a).unwrap().tags, vec!["kept", "new"]);
        assert_eq!(entities::get(&c, &b).unwrap().tags, vec!["new"]);
        assert!(matches!(
            rename(&c, "missing", "x"),
            Err(AppError::NotFound(_))
        ));
        assert!(rename(&c, &merged.id, "bad!").is_err());
    }

    #[test]
    fn delete_untags_entities_and_colour_is_validated() {
        let c = conn();
        let a = make(&c, "A");
        set_entity_tags(&c, &a, &names(&["x", "y"])).unwrap();
        let x = tag_named(&c, "x");
        assert_eq!(
            set_color(&c, &x.id, Some("#ABCDEF"))
                .unwrap()
                .color
                .as_deref(),
            Some("#abcdef")
        );
        assert!(set_color(&c, &x.id, Some("red")).is_err());
        assert_eq!(set_color(&c, &x.id, None).unwrap().color, None);
        delete(&c, &x.id).unwrap();
        assert_eq!(entities::get(&c, &a).unwrap().tags, vec!["y"]);
        assert!(matches!(delete(&c, &x.id), Err(AppError::NotFound(_))));
    }

    #[test]
    fn deleting_an_entity_removes_its_tag_links() {
        let c = conn();
        let a = make(&c, "A");
        set_entity_tags(&c, &a, &names(&["x"])).unwrap();
        entities::delete(&c, &a).unwrap();
        assert_eq!(tag_named(&c, "x").count, 0);
    }

    #[test]
    fn filter_by_tag_includes_nested_tags_and_supports_all_and_any() {
        let c = conn();
        let a = make(&c, "Alpha");
        let b = make(&c, "Beta");
        let g = make(&c, "Gamma");
        set_entity_tags(&c, &a, &names(&["places/cities", "royal"])).unwrap();
        set_entity_tags(&c, &b, &names(&["places", "dark"])).unwrap();
        set_entity_tags(&c, &g, &names(&["placesmith"])).unwrap();

        let ids = |found: Vec<EntitySummary>| -> Vec<String> {
            found.into_iter().map(|e| e.name).collect()
        };
        // "places" matches itself and "places/cities", but not "placesmith".
        assert_eq!(
            ids(list_entities_by_tag(&c, &names(&["Places"]), false).unwrap()),
            vec!["Alpha", "Beta"]
        );
        assert_eq!(
            ids(list_entities_by_tag(&c, &names(&["places/cities"]), false).unwrap()),
            vec!["Alpha"]
        );
        assert_eq!(
            ids(list_entities_by_tag(&c, &names(&["places", "royal"]), true).unwrap()),
            vec!["Alpha"]
        );
        assert_eq!(
            ids(list_entities_by_tag(&c, &names(&["royal", "dark"]), false).unwrap()),
            vec!["Alpha", "Beta"]
        );
        assert!(
            list_entities_by_tag(&c, &names(&["royal", "nope"]), true)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            list_entities_by_tag(&c, &names(&["royal", "nope"]), false)
                .unwrap()
                .len(),
            1
        );
        assert!(list_entities_by_tag(&c, &[], true).unwrap().is_empty());
        // Summaries carry their tags.
        let found = list_entities_by_tag(&c, &names(&["royal"]), true).unwrap();
        assert_eq!(found[0].tags, vec!["places/cities", "royal"]);
    }

    #[test]
    fn editing_other_fields_keeps_the_tags() {
        let c = conn();
        let a = make(&c, "A");
        set_entity_tags(&c, &a, &names(&["x"])).unwrap();
        entities::update(
            &c,
            &a,
            EntityPatch {
                summary: Some("hello".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(entities::get(&c, &a).unwrap().tags, vec!["x"]);
    }
}
