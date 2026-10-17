use serde::{Deserialize, Serialize};
use specta::Type;

/// Highest importance value (1 = minor ... 5 = major).
pub const MAX_IMPORTANCE: i32 = 5;

/// An event as shown on the timeline: its entity name plus the dates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EventSummary {
    pub entity_id: String,
    pub name: String,
    /// Day number from the world epoch; may be negative.
    #[specta(type = specta_typescript::Number)]
    pub start_day: i64,
    /// `None` = a single-day event (drawn as a point).
    #[specta(type = Option<specta_typescript::Number>)]
    pub end_day: Option<i64>,
    /// 1 (minor) to 5 (major).
    pub importance: i32,
    pub calendar_id: String,
}
