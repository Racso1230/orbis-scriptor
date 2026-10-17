use serde::{Deserialize, Serialize};
use specta::Type;

/// Marker placed before each matched term inside [`SearchResult::snippet`].
/// It is the ASCII control character STX (U+0002), which never appears in
/// real text, so the frontend can split on it and render `<mark>` elements
/// itself without needing `dangerouslySetInnerHTML`.
pub const HIGHLIGHT_START: char = '\u{2}';
/// Marker placed after each matched term (ASCII ETX, U+0003).
pub const HIGHLIGHT_END: char = '\u{3}';

/// One hit from the full-text search, best match first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub entity_id: String,
    pub entity_type: String,
    pub name: String,
    /// A short excerpt of the best-matching text. Matched terms are wrapped
    /// in U+0002 (start) and U+0003 (end); an ellipsis (U+2026) marks
    /// trimmed text.
    pub snippet: String,
}
