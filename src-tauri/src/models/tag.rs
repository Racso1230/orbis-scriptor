use serde::{Deserialize, Serialize};
use specta::Type;

/// A tag that can be put on any number of entities. Names are unique
/// ignoring case and may be nested with `/` (`places/cities`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    /// `#rrggbb`, or `None` for the default look.
    pub color: Option<String>,
    /// How many entities carry this exact tag (nested tags are counted
    /// under their own name only).
    pub count: u32,
}
