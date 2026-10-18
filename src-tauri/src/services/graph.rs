//! Graph queries over `links`: the entity graph around one entity and the
//! family tree of a character.
//!
//! Both are a breadth-first search (BFS): look at the focus entity, then all
//! of its neighbours, then their neighbours, one "level" at a time. Each level
//! costs a single SQL query (`WHERE source_id IN (...) OR target_id IN (...)`,
//! split into chunks only if the frontier is very large), instead of one
//! query per node. A recursive CTE could do the same in SQL, but the type
//! filter, node cap and generation arithmetic are much clearer in Rust.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::db;
use crate::db::links::LinkRow;
use crate::error::{AppError, AppResult};
use crate::models::entity::EntitySummary;
use crate::models::link::{KIND_FIELD_REF, KIND_INVOLVED_IN, KIND_MENTION};
use crate::models::relationship::{
    EntityGraph, FAMILY_KINDS, FamilyNode, FamilyTree, GraphEdge, GraphFilter, GraphNode,
};
use crate::services::{entities, entity_types, relationships};

pub const DEFAULT_MAX_NODES: u32 = 200;
pub const MAX_NODES_CAP: u32 = 500;
pub const MAX_DEPTH: u32 = 4;
pub const DEFAULT_GENERATIONS: u32 = 3;
pub const MAX_GENERATIONS: u32 = 10;
/// Node cap for family trees (they are layered, so keep them readable).
pub const FAMILY_MAX_NODES: usize = 300;

/// Turns the requested kind filter into the list of kinds to follow.
/// `None` means "all relationship kinds" (mentions, involved_in and
/// field_ref are opt-in). Unknown kinds are an error rather than silently ignored.
fn effective_kinds(requested: Option<&Vec<String>>) -> AppResult<Vec<String>> {
    let Some(list) = requested else {
        return Ok(relationships::relationship_kind_keys());
    };
    let known = relationships::relationship_kind_keys();
    let mut out: Vec<String> = Vec::new();
    for kind in list {
        let ok = known.contains(kind)
            || kind == KIND_MENTION
            || kind == KIND_INVOLVED_IN
            || kind == KIND_FIELD_REF;
        if !ok {
            return Err(AppError::Validation(format!(
                "Unknown relationship kind \"{kind}\""
            )));
        }
        if !out.contains(kind) {
            out.push(kind.clone());
        }
    }
    Ok(out)
}

fn to_edge(row: &LinkRow) -> GraphEdge {
    let (label, symmetric, category) = relationships::edge_info(&row.kind, row.label.as_deref());
    GraphEdge {
        id: row.id.clone(),
        source: row.source_id.clone(),
        target: row.target_id.clone(),
        kind: row.kind.clone(),
        label,
        symmetric,
        category,
    }
}

/// Every link of `kinds` whose two ends are both in `ids`, without duplicates.
fn edges_among(
    conn: &Connection,
    ids: &HashSet<String>,
    kinds: &[String],
) -> AppResult<Vec<GraphEdge>> {
    if kinds.is_empty() || ids.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<String> = ids.iter().cloned().collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut edges = Vec::new();
    for row in db::links::edges_touching(conn, &list, kinds)? {
        if ids.contains(&row.source_id)
            && ids.contains(&row.target_id)
            && seen.insert(row.id.clone())
        {
            edges.push(to_edge(&row));
        }
    }
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(edges)
}

/// Sorts candidate neighbours so results do not depend on SQLite row order:
/// by name (case-insensitive), then id.
fn sort_summaries(list: &mut [EntitySummary]) {
    list.sort_by_cached_key(|e| (e.name.to_lowercase(), e.id.clone()));
}

/// The graph around `focus_id`, up to `depth` links away.
///
/// - `depth` is clamped to 1..=4 and `max_nodes` to 1..=500 (default 200).
/// - Links are followed in both directions.
/// - `filter.kinds` picks which link kinds to follow; `filter.entity_types`
///   which neighbours to keep. A neighbour of a filtered-out type is not
///   included and is not walked through. The focus is always included.
/// - Nodes are added level by level; when the cap is reached the search
///   stops and `truncated` is set.
pub fn get_entity_graph(
    conn: &Connection,
    focus_id: &str,
    depth: u32,
    filter: &GraphFilter,
) -> AppResult<EntityGraph> {
    let depth = depth.clamp(1, MAX_DEPTH);
    let max_nodes = filter
        .max_nodes
        .unwrap_or(DEFAULT_MAX_NODES)
        .clamp(1, MAX_NODES_CAP) as usize;
    let kinds = effective_kinds(filter.kinds.as_ref())?;
    // `Option<&Vec<String>>` -> `Option<HashSet<&str>>` for fast lookups.
    if let Some(types) = &filter.entity_types {
        for t in types {
            if !entity_types::is_known(conn, t)? {
                return Err(AppError::Validation(format!("Unknown entity type \"{t}\"")));
            }
        }
    }
    let type_allowed = |t: &str| {
        filter
            .entity_types
            .as_ref()
            .is_none_or(|types| types.iter().any(|x| x == t))
    };

    let focus = entities::get(conn, focus_id)?;
    let mut nodes = vec![GraphNode {
        id: focus.id.clone(),
        name: focus.name,
        entity_type: focus.entity_type,
        degree: 0,
    }];
    let mut known: HashSet<String> = HashSet::from([focus.id.clone()]);
    let mut frontier = vec![focus.id];
    let mut truncated = false;

    if !kinds.is_empty() {
        for level in 1..=depth {
            if frontier.is_empty() {
                break;
            }
            // One query for the whole level.
            let rows = db::links::edges_touching(conn, &frontier, &kinds)?;

            // Neighbours we have not seen yet, each once.
            let mut candidates: Vec<String> = Vec::new();
            let mut queued: HashSet<&str> = HashSet::new();
            for row in &rows {
                for end in [&row.source_id, &row.target_id] {
                    if !known.contains(end) && queued.insert(end.as_str()) {
                        candidates.push(end.clone());
                    }
                }
            }
            let mut found = db::entities::summaries_by_ids(conn, &candidates)?;
            found.retain(|e| type_allowed(&e.entity_type));
            sort_summaries(&mut found);

            let mut next = Vec::new();
            for e in found {
                if nodes.len() >= max_nodes {
                    truncated = true;
                    break;
                }
                known.insert(e.id.clone());
                next.push(e.id.clone());
                nodes.push(GraphNode {
                    id: e.id,
                    name: e.name,
                    entity_type: e.entity_type,
                    degree: level,
                });
            }
            frontier = next;
            if truncated {
                break;
            }
        }
    }

    let edges = edges_among(conn, &known, &kinds)?;
    Ok(EntityGraph {
        nodes,
        edges,
        truncated,
    })
}

/// The family tree around a character, as nodes with generation numbers.
///
/// Only `parent_of`, `spouse_of` and `sibling_of` links are followed. The
/// focus is generation 0. Going from a parent to a child adds one, from a
/// child to a parent subtracts one, and spouses and siblings keep the same
/// number. So grandparents are -2, aunts and uncles -1, cousins and siblings
/// 0, and children are +1. Anyone outside the range
/// `-generations_up..=generations_down` is left out. If the data contradicts
/// itself (someone reachable at two generations), the first one found wins.
/// At most 300 nodes are returned (`truncated` tells).
pub fn get_family_tree(
    conn: &Connection,
    focus_id: &str,
    generations_up: Option<u32>,
    generations_down: Option<u32>,
) -> AppResult<FamilyTree> {
    let up = generations_up
        .unwrap_or(DEFAULT_GENERATIONS)
        .min(MAX_GENERATIONS) as i32;
    let down = generations_down
        .unwrap_or(DEFAULT_GENERATIONS)
        .min(MAX_GENERATIONS) as i32;
    let kinds: Vec<String> = FAMILY_KINDS.iter().map(|k| (*k).to_owned()).collect();

    let focus = entities::get(conn, focus_id)?;
    let mut generation: HashMap<String, i32> = HashMap::from([(focus.id.clone(), 0)]);
    let mut nodes = vec![FamilyNode {
        id: focus.id.clone(),
        name: focus.name,
        entity_type: focus.entity_type,
        generation: 0,
    }];
    let mut frontier = vec![focus.id];
    let mut truncated = false;

    while !frontier.is_empty() && !truncated {
        let rows = db::links::edges_touching(conn, &frontier, &kinds)?;

        // Work out the generation of each new person from a known neighbour.
        let mut candidates: Vec<(String, i32)> = Vec::new();
        let mut queued: HashSet<String> = HashSet::new();
        for row in &rows {
            // `parent_of`: source is the parent, target the child.
            let step = if row.kind == "parent_of" { 1 } else { 0 };
            let known_source = generation.get(&row.source_id).copied();
            let known_target = generation.get(&row.target_id).copied();
            let new = match (known_source, known_target) {
                (Some(g), None) => Some((row.target_id.clone(), g + step)),
                (None, Some(g)) => Some((row.source_id.clone(), g - step)),
                _ => None,
            };
            if let Some((id, g)) = new
                && g >= -up
                && g <= down
                && queued.insert(id.clone())
            {
                candidates.push((id, g));
            }
        }

        let ids: Vec<String> = candidates.iter().map(|(id, _)| id.clone()).collect();
        let mut found = db::entities::summaries_by_ids(conn, &ids)?;
        sort_summaries(&mut found);
        let gens: HashMap<String, i32> = candidates.into_iter().collect();

        let mut next = Vec::new();
        for e in found {
            if nodes.len() >= FAMILY_MAX_NODES {
                truncated = true;
                break;
            }
            let g = gens.get(&e.id).copied().unwrap_or(0);
            generation.insert(e.id.clone(), g);
            next.push(e.id.clone());
            nodes.push(FamilyNode {
                id: e.id,
                name: e.name,
                entity_type: e.entity_type,
                generation: g,
            });
        }
        frontier = next;
    }

    let ids: HashSet<String> = generation.keys().cloned().collect();
    let edges = edges_among(conn, &ids, &kinds)?;
    Ok(FamilyTree {
        nodes,
        edges,
        truncated,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::relationships::{
        create,
        tests::{conn, mk},
    };

    fn ids(g: &EntityGraph) -> Vec<&str> {
        g.nodes.iter().map(|n| n.name.as_str()).collect()
    }

    /// Chain A - B - C - D - E using friend_of (symmetric).
    fn chain(c: &Connection) -> Vec<String> {
        let people: Vec<String> = ["A", "B", "C", "D", "E"]
            .iter()
            .map(|n| mk(c, "character", n))
            .collect();
        for w in people.windows(2) {
            create(c, &w[0], &w[1], "friend_of", None).unwrap();
        }
        people
    }

    #[test]
    fn depth_limits_the_search_and_records_degrees() {
        let c = conn();
        let p = chain(&c);
        let g = get_entity_graph(&c, &p[0], 2, &GraphFilter::default()).unwrap();
        assert_eq!(ids(&g), vec!["A", "B", "C"]);
        let degrees: Vec<u32> = g.nodes.iter().map(|n| n.degree).collect();
        assert_eq!(degrees, vec![0, 1, 2]);
        assert_eq!(g.edges.len(), 2);
        assert!(!g.truncated);

        // Depth 0 is clamped up to 1; depth 99 down to 4.
        assert_eq!(
            get_entity_graph(&c, &p[0], 0, &GraphFilter::default())
                .unwrap()
                .nodes
                .len(),
            2
        );
        assert_eq!(
            get_entity_graph(&c, &p[0], 99, &GraphFilter::default())
                .unwrap()
                .nodes
                .len(),
            5
        );
        // Links are followed backwards too: from the end of the chain.
        let g = get_entity_graph(&c, &p[4], 1, &GraphFilter::default()).unwrap();
        assert_eq!(ids(&g), vec!["E", "D"]);
    }

    #[test]
    fn edges_between_nodes_at_the_last_level_are_included() {
        let c = conn();
        let a = mk(&c, "character", "A");
        let b = mk(&c, "character", "B");
        let d = mk(&c, "character", "D");
        create(&c, &a, &b, "friend_of", None).unwrap();
        create(&c, &a, &d, "friend_of", None).unwrap();
        create(&c, &b, &d, "enemy_of", None).unwrap();
        let g = get_entity_graph(&c, &a, 1, &GraphFilter::default()).unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 3);
    }

    #[test]
    fn kind_filter_and_optional_mention_edges() {
        let c = conn();
        let a = mk(&c, "character", "A");
        let b = mk(&c, "character", "B");
        let town = mk(&c, "location", "Town");
        create(&c, &a, &b, "friend_of", None).unwrap();
        create(&c, &a, &town, "custom", Some("haunts".into())).unwrap();
        db::links::insert_link(&c, &a, &town, KIND_MENTION).unwrap();

        // Default: relationships only, mentions excluded.
        let g = get_entity_graph(&c, &a, 1, &GraphFilter::default()).unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
        assert!(g.edges.iter().all(|e| e.kind != KIND_MENTION));
        let custom = g.edges.iter().find(|e| e.kind == "custom").unwrap();
        assert_eq!(custom.label, "haunts");

        // Only friend_of.
        let f = GraphFilter {
            kinds: Some(vec!["friend_of".into()]),
            ..Default::default()
        };
        let g = get_entity_graph(&c, &a, 1, &f).unwrap();
        assert_eq!(ids(&g), vec!["A", "B"]);
        assert_eq!(g.edges[0].label, "friend of");
        assert!(g.edges[0].symmetric);

        // Mentions only, explicitly requested.
        let f = GraphFilter {
            kinds: Some(vec![KIND_MENTION.into()]),
            ..Default::default()
        };
        let g = get_entity_graph(&c, &a, 1, &f).unwrap();
        assert_eq!(ids(&g), vec!["A", "Town"]);
        assert_eq!(g.edges[0].label, "mentions");

        // Empty kind list: just the focus.
        let f = GraphFilter {
            kinds: Some(vec![]),
            ..Default::default()
        };
        let g = get_entity_graph(&c, &a, 3, &f).unwrap();
        assert_eq!((g.nodes.len(), g.edges.len()), (1, 0));

        let bad = GraphFilter {
            kinds: Some(vec!["bogus".into()]),
            ..Default::default()
        };
        assert!(get_entity_graph(&c, &a, 1, &bad).is_err());
    }

    #[test]
    fn type_filter_keeps_the_focus_and_does_not_walk_through_hidden_types() {
        let c = conn();
        let a = mk(&c, "character", "A");
        let guild = mk(&c, "faction", "Guild");
        let b = mk(&c, "character", "B");
        let town = mk(&c, "location", "Town");
        create(&c, &a, &guild, "member_of", None).unwrap();
        create(&c, &b, &guild, "member_of", None).unwrap();
        create(&c, &a, &town, "custom", Some("born in".into())).unwrap();

        // Characters only: B is only reachable through the faction -> hidden.
        let f = GraphFilter {
            entity_types: Some(vec!["character".into()]),
            ..Default::default()
        };
        let g = get_entity_graph(&c, &a, 3, &f).unwrap();
        assert_eq!(ids(&g), vec!["A"]);

        // Focus type is not in the filter, but it is still included.
        let f = GraphFilter {
            entity_types: Some(vec!["faction".into()]),
            ..Default::default()
        };
        let g = get_entity_graph(&c, &a, 3, &f).unwrap();
        assert_eq!(ids(&g), vec!["A", "Guild"]);

        let bad = GraphFilter {
            entity_types: Some(vec!["dragon".into()]),
            ..Default::default()
        };
        assert!(get_entity_graph(&c, &a, 1, &bad).is_err());
    }

    #[test]
    fn max_nodes_truncates_and_edges_stay_inside_the_node_set() {
        let c = conn();
        let hub = mk(&c, "character", "Hub");
        for i in 0..10 {
            let n = mk(&c, "character", &format!("N{i:02}"));
            create(&c, &hub, &n, "friend_of", None).unwrap();
        }
        let f = GraphFilter {
            max_nodes: Some(4),
            ..Default::default()
        };
        let g = get_entity_graph(&c, &hub, 2, &f).unwrap();
        assert_eq!(g.nodes.len(), 4);
        assert!(g.truncated);
        // Deterministic: neighbours are admitted in name order.
        assert_eq!(ids(&g), vec!["Hub", "N00", "N01", "N02"]);
        let node_ids: HashSet<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(g.edges.len(), 3);
        assert!(
            g.edges
                .iter()
                .all(|e| node_ids.contains(e.source.as_str())
                    && node_ids.contains(e.target.as_str()))
        );

        // Exactly fitting is not truncated; the cap itself is capped at 500.
        let f = GraphFilter {
            max_nodes: Some(11),
            ..Default::default()
        };
        assert!(!get_entity_graph(&c, &hub, 1, &f).unwrap().truncated);
        let f = GraphFilter {
            max_nodes: Some(100_000),
            ..Default::default()
        };
        assert!(!get_entity_graph(&c, &hub, 1, &f).unwrap().truncated);
        assert!(matches!(
            get_entity_graph(&c, "missing", 1, &GraphFilter::default()),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn large_frontier_is_chunked() {
        // More neighbours than IN_CHUNK, to exercise the chunking path.
        let c = conn();
        let hub = mk(&c, "character", "Hub");
        let n = db::entities::IN_CHUNK + 20;
        for i in 0..n {
            let leaf = mk(&c, "character", &format!("L{i:04}"));
            create(&c, &hub, &leaf, "friend_of", None).unwrap();
        }
        let f = GraphFilter {
            max_nodes: Some(500),
            ..Default::default()
        };
        let g = get_entity_graph(&c, &hub, 2, &f).unwrap();
        assert_eq!(g.nodes.len(), n + 1);
        assert_eq!(g.edges.len(), n);
    }

    fn gen_of(t: &FamilyTree, name: &str) -> i32 {
        t.nodes.iter().find(|n| n.name == name).unwrap().generation
    }

    #[test]
    fn family_tree_generations() {
        let c = conn();
        let grand = mk(&c, "character", "Grandma");
        let parent = mk(&c, "character", "Parent");
        let aunt = mk(&c, "character", "Aunt");
        let focus = mk(&c, "character", "Focus");
        let sib = mk(&c, "character", "Sibling");
        let spouse = mk(&c, "character", "Spouse");
        let child = mk(&c, "character", "Child");
        let grandchild = mk(&c, "character", "Grandchild");
        let friend = mk(&c, "character", "Friend");
        create(&c, &grand, &parent, "parent_of", None).unwrap();
        create(&c, &grand, &aunt, "parent_of", None).unwrap();
        create(&c, &parent, &focus, "parent_of", None).unwrap();
        create(&c, &focus, &sib, "sibling_of", None).unwrap();
        create(&c, &spouse, &focus, "spouse_of", None).unwrap();
        create(&c, &focus, &child, "parent_of", None).unwrap();
        create(&c, &child, &grandchild, "parent_of", None).unwrap();
        create(&c, &focus, &friend, "friend_of", None).unwrap();

        let t = get_family_tree(&c, &focus, None, None).unwrap();
        assert_eq!(gen_of(&t, "Grandma"), -2);
        assert_eq!(gen_of(&t, "Parent"), -1);
        assert_eq!(gen_of(&t, "Aunt"), -1);
        assert_eq!(gen_of(&t, "Focus"), 0);
        assert_eq!(gen_of(&t, "Sibling"), 0);
        assert_eq!(gen_of(&t, "Spouse"), 0);
        assert_eq!(gen_of(&t, "Child"), 1);
        assert_eq!(gen_of(&t, "Grandchild"), 2);
        // Non-family links are not followed.
        assert!(t.nodes.iter().all(|n| n.name != "Friend"));
        assert_eq!(t.nodes.len(), 8);
        assert_eq!(t.edges.len(), 7);
        assert!(!t.truncated);

        // Limits: no ancestors, one generation down.
        let t = get_family_tree(&c, &focus, Some(0), Some(1)).unwrap();
        let names: HashSet<&str> = t.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            HashSet::from(["Focus", "Sibling", "Spouse", "Child"])
        );

        // Starting from the grandchild, ancestors are negative.
        let t = get_family_tree(&c, &grandchild, Some(2), Some(0)).unwrap();
        assert_eq!(gen_of(&t, "Grandchild"), 0);
        assert_eq!(gen_of(&t, "Child"), -1);
        assert_eq!(gen_of(&t, "Focus"), -2);
        assert!(t.nodes.iter().all(|n| n.name != "Parent"));
    }
}
