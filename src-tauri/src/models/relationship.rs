//! Typed relationships between entities (Phase 9).
//!
//! A relationship is one row in `links` whose `kind` is a key from the
//! catalog below (or `custom`, with the user's own text in `links.label`).
//! The catalog is the single source of truth for labels, symmetry,
//! categories and type constraints; the frontend asks for it through the
//! `list_relationship_kinds` command instead of hard-coding anything.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Kind key for user-written relationships; the text lives in `links.label`.
pub const KIND_CUSTOM: &str = "custom";

/// Used to group and filter kinds in the UI.
///
/// An `enum` is a type that is exactly one of a fixed set of variants.
/// `match` on it must handle every variant, so the compiler tells us when
/// we forget one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RelationshipCategory {
    Family,
    Social,
    Political,
    Place,
    Ownership,
    Other,
}

/// One entry of the relationship catalog, as sent to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipKindDef {
    /// Stored in `links.kind`, e.g. `parent_of`.
    pub key: String,
    /// Reads "source <forward_label> target", e.g. "parent of".
    pub forward_label: String,
    /// Reads "target <inverse_label> source", e.g. "child of".
    pub inverse_label: String,
    /// Symmetric kinds (spouse, ally...) mean the same in both directions.
    pub symmetric: bool,
    pub category: RelationshipCategory,
    /// Allowed entity types for the source; empty means any type.
    pub source_types: Vec<String>,
    /// Allowed entity types for the target; empty means any type.
    pub target_types: Vec<String>,
    /// True only for `custom`: the user supplies the label text.
    pub needs_label: bool,
}

/// Compact constructor for the catalog table below. `&[&str]` is a borrowed
/// slice of string literals, converted to owned `String`s once here.
fn def(
    key: &str,
    labels: (&str, &str),
    symmetric: bool,
    category: RelationshipCategory,
    source_types: &[&str],
    target_types: &[&str],
) -> RelationshipKindDef {
    let owned = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect();
    RelationshipKindDef {
        key: key.into(),
        forward_label: labels.0.into(),
        inverse_label: labels.1.into(),
        symmetric,
        category,
        source_types: owned(source_types),
        target_types: owned(target_types),
        needs_label: key == KIND_CUSTOM,
    }
}

/// The built-in relationship kinds, in the order the UI should list them.
pub fn all() -> Vec<RelationshipKindDef> {
    use RelationshipCategory::*;
    const C: &str = "character";
    const F: &str = "faction";
    const L: &str = "location";
    const I: &str = "item";
    vec![
        def(
            "parent_of",
            ("parent of", "child of"),
            false,
            Family,
            &[C],
            &[C],
        ),
        def(
            "spouse_of",
            ("spouse of", "spouse of"),
            true,
            Family,
            &[C],
            &[C],
        ),
        def(
            "sibling_of",
            ("sibling of", "sibling of"),
            true,
            Family,
            &[C],
            &[C],
        ),
        def(
            "friend_of",
            ("friend of", "friend of"),
            true,
            Social,
            &[C],
            &[C],
        ),
        def(
            "mentor_of",
            ("mentor of", "student of"),
            false,
            Social,
            &[C],
            &[C],
        ),
        def(
            "member_of",
            ("member of", "has member"),
            false,
            Political,
            &[C],
            &[F],
        ),
        def(
            "leader_of",
            ("leader of", "led by"),
            false,
            Political,
            &[C],
            &[F],
        ),
        def(
            "ally_of",
            ("ally of", "ally of"),
            true,
            Political,
            &[C, F],
            &[C, F],
        ),
        def(
            "enemy_of",
            ("enemy of", "enemy of"),
            true,
            Political,
            &[C, F],
            &[C, F],
        ),
        def(
            "rules",
            ("rules", "ruled by"),
            false,
            Political,
            &[C, F],
            &[L, F],
        ),
        def(
            "located_in",
            ("located in", "contains"),
            false,
            Place,
            &[L, C, F, I],
            &[L],
        ),
        def(
            "owns",
            ("owns", "owned by"),
            false,
            Ownership,
            &[C, F],
            &[I],
        ),
        def(
            "related_to",
            ("related to", "related to"),
            true,
            Other,
            &[],
            &[],
        ),
        // The label text is supplied by the user, so these labels are only
        // placeholders the UI can show in a kind picker.
        def(KIND_CUSTOM, ("custom", "custom"), false, Other, &[], &[]),
    ]
}

/// Looks a kind up by key.
pub fn find(key: &str) -> Option<RelationshipKindDef> {
    all().into_iter().find(|k| k.key == key)
}

/// The kind keys that make up a family tree.
pub const FAMILY_KINDS: [&str; 3] = ["parent_of", "spouse_of", "sibling_of"];

/// A stored relationship row, as returned by create/update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipRecord {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    /// Only set for `custom` relationships.
    pub label: Option<String>,
}

/// A relationship phrased from the point of view of one entity
/// ("Ada is *child of* Bran").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipView {
    pub link_id: String,
    pub kind: String,
    /// What the viewed entity is to the other one: "child of", "parent of",
    /// "spouse of", or the custom text.
    pub label: String,
    pub other_id: String,
    pub other_name: String,
    #[serde(rename = "otherType")]
    pub other_type: String,
    /// True when the viewed entity is the stored `source`.
    pub outgoing: bool,
    pub symmetric: bool,
    pub category: RelationshipCategory,
}

/// Which links `get_entity_graph` follows and which nodes it keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphFilter {
    /// Relationship kind keys to follow. `None` = every relationship kind.
    /// May also contain `mention`, `involved_in` and `field_ref` (excluded by default).
    pub kinds: Option<Vec<String>>,
    /// Entity types to keep. `None` = all. The focus node is always kept.
    pub entity_types: Option<Vec<String>>,
    /// Node cap: default 200, at most 500.
    pub max_nodes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    /// Distance from the focus node (0 = the focus itself).
    pub degree: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    /// The `links` row id.
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
    /// Text to draw on the edge, read as "source <label> target".
    pub label: String,
    pub symmetric: bool,
    pub category: RelationshipCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityGraph {
    pub nodes: Vec<GraphNode>,
    /// Only edges whose both ends are in `nodes`.
    pub edges: Vec<GraphEdge>,
    /// True when `max_nodes` stopped the search before it was complete.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FamilyNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    /// 0 = the focus's generation, negative = ancestors, positive = descendants.
    pub generation: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FamilyTree {
    pub nodes: Vec<FamilyNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn catalog_keys_are_unique_and_reserved_kinds_absent() {
        let kinds = all();
        let mut keys: Vec<&str> = kinds.iter().map(|k| k.key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), kinds.len());
        assert!(find("mention").is_none());
        assert!(find("involved_in").is_none());
        assert!(find(KIND_CUSTOM).unwrap().needs_label);
    }

    #[test]
    fn symmetric_kinds_read_the_same_both_ways() {
        for k in all().iter().filter(|k| k.symmetric) {
            assert_eq!(k.forward_label, k.inverse_label, "{}", k.key);
        }
    }
}
