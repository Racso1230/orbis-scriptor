//! The optional sample world, "The Shattered Coast".
//!
//! It is built through the normal services (entities, documents, maps,
//! events), never with raw SQL, so the search index, mention links and
//! involved-in links are exactly what the app itself would have produced.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::entity::{EntityPatch, NewEntity};
use crate::models::map::{Asset, MapGeometry, NewMapObject};
use crate::services::world::{self, ASSETS_DIR};
use crate::services::{assets, calendar, documents, entities, events, maps, relationships};
use crate::state::WorldHandle;

/// Name of the world and of the folder created inside the chosen parent.
pub const SAMPLE_NAME: &str = "The Shattered Coast";

/// Size of the bundled map picture, in pixels.
const MAP_SIZE: (u32, u32) = (2000, 1333);

/// The map picture, embedded in the program so it works in dev and release.
/// (`include_bytes!` reads the file at compile time into a `&'static [u8]`.)
/// Made by `node scripts/make-fantasy-map.mjs 2000 1333 map.png 7`.
const MAP_IMAGE: &[u8] = include_bytes!("../../assets/shattered-coast-map.png");

/// (type, name, summary, properties)
type EntitySeed = (&'static str, &'static str, &'static str, Value);

fn entity_seeds() -> Vec<EntitySeed> {
    vec![
        (
            "character",
            "Queen Isolde Varn",
            "Young ruler of the Shattered Coast, crowned after the Tide War.",
            json!({ "species": "Human", "age": 34, "status": "alive" }),
        ),
        (
            "character",
            "Captain Joren Hale",
            "Veteran commander of the Tidewardens' fleet and the Queen's oldest ally.",
            json!({ "species": "Human", "age": 41, "status": "alive" }),
        ),
        (
            "character",
            "Archmage Teodric Vale",
            "Reclusive mage of Highmarch who wants the broken crown for himself.",
            json!({ "species": "Half-elf", "age": 212, "status": "alive" }),
        ),
        (
            "character",
            "King Aldous Varn",
            "Isolde's father, who fell in the last battle of the Tide War.",
            json!({ "species": "Human", "age": 63, "status": "dead" }),
        ),
        (
            "character",
            "Lady Seren Varn",
            "Isolde's younger sister, a quiet scholar at court.",
            json!({ "species": "Human", "age": 29, "status": "alive" }),
        ),
        (
            "character",
            "Lucan Dorne",
            "The Queen's husband, a diplomat from the Northern Reach.",
            json!({ "species": "Human", "age": 36, "status": "alive" }),
        ),
        (
            "character",
            "Prince Corwin Varn",
            "Isolde and Lucan's young son and heir.",
            json!({ "species": "Human", "age": 8, "status": "alive" }),
        ),
        (
            "faction",
            "House Varn",
            "The royal house that has ruled the coast for nine generations.",
            json!({ "kind": "house", "leader": "Queen Isolde Varn" }),
        ),
        (
            "faction",
            "The Tidewardens",
            "A guild of sailors and harbour watchmen who keep the sea lanes open.",
            json!({ "kind": "guild", "leader": "Captain Joren Hale" }),
        ),
        (
            "item",
            "The Shattered Crown",
            "The royal crown, broken into five shards when the old king fell.",
            json!({ "kind": "artifact", "owner": "House Varn" }),
        ),
        (
            "location",
            "Port Veyra",
            "The capital: a crowded harbour city on the inland sea.",
            json!({ "kind": "city", "region": "Veyran Bay" }),
        ),
        (
            "location",
            "Highmarch Citadel",
            "A mage-fortress built into the foothills above the bay.",
            json!({ "kind": "landmark", "region": "The Highmarch" }),
        ),
        (
            "location",
            "Stormwatch Keep",
            "A windswept northern fort that watches the open sea.",
            json!({ "kind": "landmark", "region": "The Northern Reach" }),
        ),
        (
            "location",
            "The Ember Spine",
            "A jagged mountain range, home to old forges and older rumours.",
            json!({ "kind": "region", "region": "The Western Marches" }),
        ),
        (
            "location",
            "Whisperwood",
            "A dark forest where travellers hear their own names spoken.",
            json!({ "kind": "region", "region": "The Western Marches" }),
        ),
        (
            "note",
            "Campaign hooks",
            "Loose ideas for a first adventure on the coast.",
            json!({}),
        ),
    ]
}

/// (source name, kind, target name, custom label). Read each row as
/// "source <kind> target", e.g. Aldous *parent of* Isolde.
type RelationshipSeed = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
);

fn relationship_seeds() -> Vec<RelationshipSeed> {
    vec![
        // The royal family.
        ("King Aldous Varn", "parent_of", "Queen Isolde Varn", None),
        ("King Aldous Varn", "parent_of", "Lady Seren Varn", None),
        ("Queen Isolde Varn", "sibling_of", "Lady Seren Varn", None),
        ("Lucan Dorne", "spouse_of", "Queen Isolde Varn", None),
        ("Queen Isolde Varn", "parent_of", "Prince Corwin Varn", None),
        ("Lucan Dorne", "parent_of", "Prince Corwin Varn", None),
        (
            "Captain Joren Hale",
            "mentor_of",
            "Prince Corwin Varn",
            None,
        ),
        // Factions.
        ("Queen Isolde Varn", "leader_of", "House Varn", None),
        ("King Aldous Varn", "member_of", "House Varn", None),
        ("Lady Seren Varn", "member_of", "House Varn", None),
        ("Captain Joren Hale", "leader_of", "The Tidewardens", None),
        ("House Varn", "ally_of", "The Tidewardens", None),
        ("Queen Isolde Varn", "ally_of", "Captain Joren Hale", None),
        (
            "Archmage Teodric Vale",
            "enemy_of",
            "Queen Isolde Varn",
            None,
        ),
        // Places.
        ("Queen Isolde Varn", "rules", "Port Veyra", None),
        ("Archmage Teodric Vale", "rules", "Highmarch Citadel", None),
        ("Whisperwood", "located_in", "The Ember Spine", None),
        ("The Tidewardens", "located_in", "Port Veyra", None),
        // Items.
        ("House Varn", "owns", "The Shattered Crown", None),
        ("The Shattered Crown", "located_in", "Port Veyra", None),
        (
            "Archmage Teodric Vale",
            "custom",
            "The Shattered Crown",
            Some("covets"),
        ),
    ]
}

/// Lore texts by entity name. A `[[Name]]` becomes a real mention node;
/// blank lines split paragraphs. Names may refer to entities or events.
fn lore_seeds() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "Queen Isolde Varn",
            "Isolde of [[House Varn]] was crowned at [[Port Veyra]] in 1192, five years after the end of [[The Tide War]]. She wears no crown at all, because [[The Shattered Crown]] has never been mended.\n\nHer closest ally is [[Captain Joren Hale]] of [[The Tidewardens]]. Her greatest worry is the ambition of [[Archmage Teodric Vale]].",
        ),
        (
            "Captain Joren Hale",
            "Joren Hale rose from deckhand to captain of [[The Tidewardens]] during [[The Tide War]]. He still spends each winter at [[Stormwatch Keep]], counting ships.\n\nHe swore his oath to [[Queen Isolde Varn]] in the harbour hall of [[Port Veyra]].",
        ),
        (
            "Archmage Teodric Vale",
            "Teodric has lived in [[Highmarch Citadel]] for two centuries. He lent his magic to the defence of the coast in [[The Tide War]] and has never forgiven [[House Varn]] for thanking him with a medal.\n\nHe is certain that the shards of [[The Shattered Crown]] lie somewhere in [[The Ember Spine]].",
        ),
        (
            "House Varn",
            "The Varns united the coast under one banner nine generations ago. Their seat is [[Port Veyra]], and their sworn guard is [[The Tidewardens]]. The current head of the house is [[Queen Isolde Varn]].",
        ),
        (
            "The Tidewardens",
            "Founded to keep pirates out of the bay, the Tidewardens now police every harbour on the coast. They answer to [[Captain Joren Hale]] and, through him, to [[House Varn]].",
        ),
        (
            "The Shattered Crown",
            "Once worn by every ruler of [[House Varn]], the crown broke into five shards when the old king fell. Three shards are lost. [[Archmage Teodric Vale]] claims to have found one in [[The Ember Spine]].",
        ),
        (
            "Port Veyra",
            "The capital of the coast, built around a deep harbour. The palace of [[House Varn]] overlooks the docks, and the fleet of [[The Tidewardens]] anchors below it.\n\nTravellers say the road west to [[Whisperwood]] is faster by night, though nobody explains why.",
        ),
        (
            "Highmarch Citadel",
            "A fortress of pale stone whose towers lean toward the bay. It is the home of [[Archmage Teodric Vale]]. [[Port Veyra]] is two days away by road.",
        ),
        (
            "Stormwatch Keep",
            "The northernmost fort of the realm. [[Captain Joren Hale]] keeps a small garrison of [[The Tidewardens]] here to watch for sails.",
        ),
        (
            "The Ember Spine",
            "A range of black mountains with abandoned forges. Rumour says a shard of [[The Shattered Crown]] was carried here. [[Whisperwood]] lies at its foot.",
        ),
        (
            "Whisperwood",
            "The trees here seem to murmur. Hunters from [[Port Veyra]] avoid it after dark, and the miners of [[The Ember Spine]] pay a toll in salt to pass through.",
        ),
        (
            "Campaign hooks",
            "1. A courier from [[Stormwatch Keep]] arrives at [[Port Veyra]] with a sealed warning and no memory of who sent it.\n\n2. [[Archmage Teodric Vale]] offers the party a fortune to fetch something from [[Whisperwood]].\n\n3. [[Queen Isolde Varn]] secretly wants to know whether [[The Shattered Crown]] can be repaired.",
        ),
        (
            "The Tide War",
            "Five years of fighting between the sea-lords and the crown. It ended with the siege of [[Highmarch Citadel]] and the fall of the old king, whose [[The Shattered Crown]] broke in the last battle.",
        ),
        (
            "Coronation of Queen Isolde",
            "A grey, rainy day in [[Port Veyra]]. [[Captain Joren Hale]] held the empty crown cushion because [[The Shattered Crown]] could not be worn.",
        ),
    ]
}

/// Pins on the map: (location name, x, y) in map pixels. Each point sits on
/// land in the bundled image.
const PINS: [(&str, f64, f64); 5] = [
    ("Port Veyra", 1260.0, 720.0),
    ("Highmarch Citadel", 790.0, 900.0),
    ("Stormwatch Keep", 900.0, 120.0),
    ("The Ember Spine", 430.0, 600.0),
    ("Whisperwood", 280.0, 650.0),
];

/// (year, month, day) in the default calendar.
type Ymd = (i64, u32, u32);

struct EventSeed {
    name: &'static str,
    summary: &'static str,
    start: Ymd,
    end: Option<Ymd>,
    importance: i32,
    involved: &'static [&'static str],
}

fn event_seeds() -> Vec<EventSeed> {
    vec![
        EventSeed {
            name: "Founding of the Tidewardens",
            summary: "Harbour watchmen band together against the pirates of the bay.",
            start: (1122, 4, 3),
            end: None,
            importance: 3,
            involved: &["The Tidewardens", "Port Veyra"],
        },
        EventSeed {
            name: "The Tide War",
            summary: "Five years of war between the sea-lords and the crown.",
            start: (1183, 2, 1),
            end: Some((1187, 9, 20)),
            importance: 5,
            involved: &[
                "Captain Joren Hale",
                "Archmage Teodric Vale",
                "House Varn",
                "The Tidewardens",
            ],
        },
        EventSeed {
            name: "Siege of Highmarch",
            summary: "The last stronghold of the sea-lords holds out for three months.",
            start: (1187, 4, 1),
            end: Some((1187, 7, 15)),
            importance: 3,
            involved: &["Archmage Teodric Vale", "Highmarch Citadel"],
        },
        EventSeed {
            name: "Breaking of the Crown",
            summary: "The old king falls and the royal crown shatters.",
            start: (1187, 9, 20),
            end: None,
            importance: 4,
            involved: &["The Shattered Crown", "House Varn"],
        },
        EventSeed {
            name: "Coronation of Queen Isolde",
            summary: "Isolde Varn takes the throne in a quiet ceremony.",
            start: (1192, 5, 10),
            end: None,
            importance: 4,
            involved: &["Queen Isolde Varn", "Captain Joren Hale", "Port Veyra"],
        },
    ]
}

/// Builds a TipTap document from text with `[[Name]]` mentions.
/// Returns (TipTap JSON string, plain text for search).
fn build_document(text: &str, ids: &HashMap<String, String>) -> AppResult<(String, String)> {
    let mut paragraphs = Vec::new();
    let mut plain = Vec::new();
    for block in text.split("\n\n") {
        let mut nodes = Vec::new();
        let mut plain_block = String::new();
        let mut rest = block;
        while let Some(open) = rest.find("[[") {
            let (before, after) = rest.split_at(open);
            let close = after
                .find("]]")
                .ok_or_else(|| AppError::Internal(format!("unclosed mention in: {block}")))?;
            let name = &after[2..close];
            let id = ids.get(name).ok_or_else(|| {
                AppError::Internal(format!("sample lore mentions unknown {name}"))
            })?;
            if !before.is_empty() {
                nodes.push(json!({ "type": "text", "text": before }));
                plain_block.push_str(before);
            }
            // The same node the editor's Mention extension produces.
            nodes.push(json!({ "type": "mention", "attrs": { "id": id } }));
            plain_block.push_str(name);
            rest = &after[close + 2..];
        }
        if !rest.is_empty() {
            nodes.push(json!({ "type": "text", "text": rest }));
            plain_block.push_str(rest);
        }
        paragraphs.push(json!({ "type": "paragraph", "content": nodes }));
        plain.push(plain_block);
    }
    let doc = json!({ "type": "doc", "content": paragraphs });
    Ok((doc.to_string(), plain.join("\n\n")))
}

/// Writes the embedded map image into `assets/` and registers it.
fn import_map_image(conn: &Connection, assets_dir: &Path) -> AppResult<Asset> {
    fs::create_dir_all(assets_dir)?;
    let asset = assets::with_paths(
        Asset {
            id: Uuid::now_v7().to_string(),
            file_name: "shattered-coast-map.png".into(),
            mime: "image/png".into(),
            width: Some(MAP_SIZE.0),
            height: Some(MAP_SIZE.1),
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            path: String::new(),
            thumb_path: None,
        },
        assets_dir,
    );
    fs::write(&asset.path, MAP_IMAGE)?;
    maps::register_asset(conn, &asset)?;
    // A missing thumbnail is cosmetic, so a failure is only logged.
    if let Err(e) = assets::make_thumbnail(assets_dir, &asset.id, &asset.mime) {
        eprintln!("sample map thumbnail failed: {e}");
    }
    Ok(asset)
}

/// Fills an empty, freshly created world with the sample content.
pub fn populate(conn: &Connection, assets_dir: &Path) -> AppResult<()> {
    // name -> id, so lore and events can refer to things by name.
    let mut ids: HashMap<String, String> = HashMap::new();

    for (entity_type, name, summary, properties) in entity_seeds() {
        let entity = entities::create(
            conn,
            NewEntity {
                entity_type: entity_type.into(),
                name: name.into(),
            },
        )?;
        entities::update(
            conn,
            &entity.id,
            EntityPatch {
                summary: Some(summary.into()),
                properties: Some(properties),
                ..Default::default()
            },
        )?;
        ids.insert(name.into(), entity.id);
    }

    // Creating a `map` entity also creates its `maps` row and default layer.
    let map = entities::create(
        conn,
        NewEntity {
            entity_type: "map".into(),
            name: "Map of the Shattered Coast".into(),
        },
    )?;
    entities::update(
        conn,
        &map.id,
        EntityPatch {
            summary: Some("The lands around the inland sea.".into()),
            ..Default::default()
        },
    )?;
    let asset = import_map_image(conn, assets_dir)?;
    maps::set_background(conn, assets_dir, &map.id, &asset.id)?;
    let data = maps::get_map(conn, assets_dir, &map.id)?;
    let layer_id = data
        .layers
        .first()
        .map(|l| l.id.clone())
        .ok_or_else(|| AppError::Internal("sample map has no layer".into()))?;
    for (name, x, y) in PINS {
        maps::create_object(
            conn,
            NewMapObject {
                layer_id: layer_id.clone(),
                geometry: MapGeometry::Pin { x, y },
                style: None,
                entity_id: ids.get(name).cloned(),
            },
        )?;
    }

    // Relationships between the entities above (by name).
    for (source, kind, target, label) in relationship_seeds() {
        let find = |name: &str| {
            ids.get(name)
                .ok_or_else(|| AppError::Internal(format!("unknown relationship entity {name}")))
        };
        relationships::create(
            conn,
            find(source)?,
            find(target)?,
            kind,
            label.map(Into::into),
        )?;
    }

    // Events: dates become day numbers through the default calendar.
    let cal = calendar::get_default(conn)?.definition;
    let day = |(year, month, d): Ymd| calendar::from_date(&cal, 0, year, month, d);
    for seed in event_seeds() {
        let start = day(seed.start)?;
        let end = seed.end.map(day).transpose()?;
        let event = entities::create_event(
            conn,
            NewEntity {
                entity_type: "event".into(),
                name: seed.name.into(),
            },
            start,
            end,
        )?;
        events::update_dates(conn, &event.id, start, end, Some(seed.importance))?;
        entities::update(
            conn,
            &event.id,
            EntityPatch {
                summary: Some(seed.summary.into()),
                ..Default::default()
            },
        )?;
        for who in seed.involved {
            let who_id = ids
                .get(*who)
                .ok_or_else(|| AppError::Internal(format!("unknown involved entity {who}")))?;
            events::add_involved(conn, &event.id, who_id)?;
        }
        ids.insert(seed.name.into(), event.id);
    }

    // Lore last, when every name resolves to an id. `documents::save` also
    // records the mention links and updates the search index.
    for (name, text) in lore_seeds() {
        let id = ids
            .get(name)
            .ok_or_else(|| AppError::Internal(format!("lore for unknown entity {name}")))?;
        let (content, plain_text) = build_document(text, &ids)?;
        documents::save(conn, id, content, plain_text)?;
    }
    Ok(())
}

/// Creates `<parent>/The Shattered Coast` as a normal world and fills it.
pub fn create(parent: &Path) -> AppResult<WorldHandle> {
    let dir = parent.join(SAMPLE_NAME);
    if dir.exists() {
        return Err(AppError::Validation(format!(
            "A folder called \"{SAMPLE_NAME}\" already exists there. Open it, or choose another location."
        )));
    }
    let handle = world::create_world(&dir, SAMPLE_NAME)?;
    populate(&handle.conn, &dir.join(ASSETS_DIR))?;
    Ok(handle)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::{graph, links, search};

    fn sample() -> (tempfile::TempDir, WorldHandle) {
        let dir = tempfile::tempdir().unwrap();
        let handle = create(dir.path()).unwrap();
        (dir, handle)
    }

    fn id_of(conn: &Connection, name: &str) -> String {
        entities::list(conn, None)
            .unwrap()
            .into_iter()
            .find(|e| e.name == name)
            .unwrap()
            .id
    }

    #[test]
    fn creates_expected_content() {
        let (dir, w) = sample();
        assert_eq!(w.manifest.name, SAMPLE_NAME);
        assert!(dir.path().join(SAMPLE_NAME).join("world.json").exists());

        let count = |t: &str| entities::list(&w.conn, Some(t)).unwrap().len();
        assert_eq!(count("character"), 7);
        assert_eq!(count("location"), 5);
        assert_eq!(count("faction"), 2);
        assert_eq!(count("item"), 1);
        assert_eq!(count("note"), 1);
        assert_eq!(count("map"), 1);
        assert_eq!(count("event"), 5);
    }

    #[test]
    fn mentions_create_backlinks() {
        let (_dir, w) = sample();
        let port = id_of(&w.conn, "Port Veyra");
        let back = links::backlinks(&w.conn, &port, "Port Veyra").unwrap();
        let names: Vec<&str> = back.iter().map(|b| b.source_name.as_str()).collect();
        assert!(names.contains(&"Queen Isolde Varn"));
        assert!(names.contains(&"House Varn"));
        assert!(back.len() >= 4);
    }

    #[test]
    fn map_has_background_and_pins_on_locations() {
        let (dir, w) = sample();
        let assets_dir = dir.path().join(SAMPLE_NAME).join(ASSETS_DIR);
        let map_id = id_of(&w.conn, "Map of the Shattered Coast");
        let data = maps::get_map(&w.conn, &assets_dir, &map_id).unwrap();
        assert_eq!((data.map.width, data.map.height), MAP_SIZE);
        let bg = data.map.background.unwrap();
        assert!(Path::new(&bg.path).exists());
        assert_eq!(data.objects.len(), PINS.len());
        assert!(
            data.objects
                .iter()
                .all(|o| o.entity_type.as_deref() == Some("location"))
        );
    }

    #[test]
    fn events_have_ranges_points_and_people() {
        let (_dir, w) = sample();
        let all = events::list(&w.conn, -1_000_000, 1_000_000).unwrap();
        assert_eq!(all.len(), 5);
        let war = all.iter().find(|e| e.name == "The Tide War").unwrap();
        assert!(war.end_day.is_some());
        assert_eq!(war.importance, 5);
        let coronation = all
            .iter()
            .find(|e| e.name == "Coronation of Queen Isolde")
            .unwrap();
        assert!(coronation.end_day.is_none());
        let who = events::list_involved(&w.conn, &war.entity_id).unwrap();
        assert!(who.iter().any(|e| e.name == "Captain Joren Hale"));
    }

    #[test]
    fn has_relationships_for_the_graph() {
        let (_dir, w) = sample();
        let isolde = id_of(&w.conn, "Queen Isolde Varn");
        let rels = relationships::list_for_entity(&w.conn, &isolde).unwrap();
        let has = |label: &str, other: &str| {
            rels.iter()
                .any(|r| r.label == label && r.other_name == other)
        };
        assert!(has("child of", "King Aldous Varn"));
        assert!(has("spouse of", "Lucan Dorne"));
        assert!(has("parent of", "Prince Corwin Varn"));
        assert!(has("leader of", "House Varn"));
        assert!(has("enemy of", "Archmage Teodric Vale"));

        // The graph and the family tree see them too.
        let g = graph::get_entity_graph(&w.conn, &isolde, 2, &Default::default()).unwrap();
        assert!(g.nodes.len() >= 8 && !g.truncated);
        let tree = graph::get_family_tree(&w.conn, &isolde, None, None).unwrap();
        let gen_of = |n: &str| tree.nodes.iter().find(|x| x.name == n).unwrap().generation;
        assert_eq!(gen_of("King Aldous Varn"), -1);
        assert_eq!(gen_of("Lady Seren Varn"), 0);
        assert_eq!(gen_of("Lucan Dorne"), 0);
        assert_eq!(gen_of("Prince Corwin Varn"), 1);
        // Mentions from the lore are still there next to the relationships.
        let port = id_of(&w.conn, "Port Veyra");
        assert!(
            links::backlinks(&w.conn, &port, "Port Veyra")
                .unwrap()
                .len()
                >= 4
        );
    }

    #[test]
    fn search_finds_seeded_names_and_lore() {
        let (_dir, w) = sample();
        let hits = search::search(&w.conn, "Veyra", 10).unwrap();
        assert!(hits.iter().any(|h| h.name == "Port Veyra"));
        // Words that only appear in lore text are indexed too.
        assert!(!search::search(&w.conn, "murmur", 10).unwrap().is_empty());
    }

    #[test]
    fn refuses_to_overwrite_an_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(SAMPLE_NAME)).unwrap();
        assert!(matches!(create(dir.path()), Err(AppError::Validation(_))));
    }
}
