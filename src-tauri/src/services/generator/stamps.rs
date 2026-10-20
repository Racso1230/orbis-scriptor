//! The stamp keys the frontend knows how to draw.
//!
//! The list lives in `src/features/maps/stamp-keys.json` and is shared with
//! the frontend stamp library, so the generator can only emit stamps that
//! exist. `include_str!` copies the file into the binary at compile time.

use std::collections::HashSet;
use std::sync::LazyLock;

const STAMP_KEYS_JSON: &str = include_str!("../../../../src/features/maps/stamp-keys.json");

/// Parsed once, on first use. `LazyLock` runs the closure the first time the
/// value is touched and shares the result between threads afterwards.
static STAMP_KEYS: LazyLock<Vec<String>> = LazyLock::new(|| {
    // The file is part of the source tree and checked by a test, so a parse
    // failure here can only be a build mistake; fall back to "no stamps"
    // rather than panicking.
    serde_json::from_str(STAMP_KEYS_JSON).unwrap_or_default()
});

static STAMP_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| STAMP_KEYS.iter().map(String::as_str).collect());

/// Every stamp key, sorted.
pub fn known_stamp_keys() -> &'static [String] {
    &STAMP_KEYS
}

/// True if the frontend has a stamp with this key.
pub fn is_known_stamp(key: &str) -> bool {
    STAMP_SET.contains(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_list_parses_and_is_sorted_and_valid() {
        let keys = known_stamp_keys();
        assert_eq!(keys.len(), 30);
        let mut sorted = keys.to_vec();
        sorted.sort();
        assert_eq!(sorted, keys);
        for key in keys {
            assert!(
                key.len() <= 64
                    && key
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{key}"
            );
        }
        assert!(is_known_stamp("snowy-mountain"));
        assert!(!is_known_stamp("dragon"));
    }
}
