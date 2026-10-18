//! YAML front matter at the top of a Markdown note:
//!
//! ```text
//! ---
//! tags: [royalty, places/cities]
//! aliases:
//!   - The Queen
//! ---
//! The note text...
//! ```
//!
//! [`split`] separates the YAML from the text and [`parse`] turns the YAML
//! into a `serde_json::Value` object, so the rest of the importer works with
//! one familiar value type. The YAML parser is `yaml-rust2` (see Cargo.toml
//! for why it was chosen over the deprecated `serde_yaml`).

use serde_json::{Map, Value, json};
use yaml_rust2::{Yaml, YamlLoader};

/// A note split into its front matter and the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split<'a> {
    /// The YAML between the `---` lines, if the note starts with one.
    pub yaml: Option<&'a str>,
    /// Everything after the closing `---` (or the whole note).
    pub body: &'a str,
}

/// Splits `text` into YAML and body. Front matter must start on the very
/// first line (a UTF-8 BOM is allowed) with `---` and end with a line that is
/// `---` or `...`. A note without a closing line has no front matter.
pub fn split(text: &str) -> Split<'_> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let no_front = Split {
        yaml: None,
        body: text,
    };
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return no_front;
    };
    if first.trim_end() != "---" {
        return no_front;
    }
    let mut offset = first.len();
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return Split {
                yaml: Some(&text[first.len()..offset]),
                body: &text[offset + line.len()..],
            };
        }
        offset += line.len();
    }
    no_front
}

/// Parses front matter YAML into a JSON object. An empty block is an empty
/// object; anything that is not a mapping (or is invalid YAML) is an error
/// with a short message.
pub fn parse(yaml: &str) -> Result<Map<String, Value>, String> {
    let docs = YamlLoader::load_from_str(yaml).map_err(|e| e.to_string())?;
    match docs.into_iter().next() {
        None | Some(Yaml::Null) => Ok(Map::new()),
        Some(Yaml::Hash(hash)) => {
            let mut out = Map::new();
            for (key, value) in hash {
                if let Some(key) = scalar_key(&key) {
                    out.insert(key, to_json(&value));
                }
            }
            Ok(out)
        }
        Some(_) => Err("the front matter is not a list of key: value lines".into()),
    }
}

/// Keys are written back as text (`1: x` is the key "1").
fn scalar_key(key: &Yaml) -> Option<String> {
    match key {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Real(r) => Some(r.clone()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn to_json(yaml: &Yaml) -> Value {
    match yaml {
        Yaml::Null | Yaml::BadValue | Yaml::Alias(_) => Value::Null,
        Yaml::Boolean(b) => json!(b),
        Yaml::Integer(i) => json!(i),
        // `Real` keeps the original text; `parse` fails only for odd values
        // such as `.inf`, which we keep as text.
        Yaml::Real(r) => r
            .parse::<f64>()
            .ok()
            .filter(|f| f.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map_or_else(|| json!(r), Value::Number),
        Yaml::String(s) => json!(s),
        Yaml::Array(items) => Value::Array(items.iter().map(to_json).collect()),
        Yaml::Hash(hash) => {
            let mut map = Map::new();
            for (k, v) in hash {
                if let Some(key) = scalar_key(k) {
                    map.insert(key, to_json(v));
                }
            }
            Value::Object(map)
        }
    }
}

/// A value as a list of non-empty strings: a YAML list, or a single string
/// (for `tags` Obsidian also accepts `a, b` and `a b`-style text, which we
/// split on commas only).
pub fn string_list(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.trim().to_owned()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect(),
        Value::String(s) => s
            .split(',')
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// A scalar as text (`None` for null, lists and mappings).
pub fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn splits_front_matter_from_the_body() {
        let s = split("---\ntags: [a]\n---\nHello\n\nWorld");
        assert_eq!(s.yaml, Some("tags: [a]\n"));
        assert_eq!(s.body, "Hello\n\nWorld");
        // CRLF line endings and a BOM.
        let s = split("\u{feff}---\r\ntitle: x\r\n---\r\nBody");
        assert_eq!(s.yaml, Some("title: x\r\n"));
        assert_eq!(s.body, "Body");
        // `...` also closes; an empty block is allowed.
        assert_eq!(split("---\n...\nx").yaml, Some(""));
    }

    #[test]
    fn notes_without_complete_front_matter_are_all_body() {
        for text in [
            "",
            "Hello",
            "---\nno end",
            " ---\na: 1\n---\nx",
            "text\n---\na: 1\n---",
        ] {
            let s = split(text);
            assert_eq!(s.yaml, None, "{text:?}");
            assert_eq!(s.body, text);
        }
        // A horizontal rule later in the note is not front matter.
        assert!(split("# Title\n\n---\n\nmore").yaml.is_none());
    }

    #[test]
    fn parses_scalars_lists_and_nested_values() {
        let map = parse(
            "id: \"abc\"\nage: 31\nratio: 2.5\nalive: true\nnothing:\ntags:\n  - a\n  - b/c\ninline: [x, y]\nnested:\n  k: v\ndate: 2024-05-17\n1: one\n",
        )
        .unwrap();
        assert_eq!(map["id"], "abc");
        assert_eq!(map["age"], 31);
        assert_eq!(map["ratio"], 2.5);
        assert_eq!(map["alive"], true);
        assert_eq!(map["nothing"], Value::Null);
        assert_eq!(map["tags"], json!(["a", "b/c"]));
        assert_eq!(map["inline"], json!(["x", "y"]));
        assert_eq!(map["nested"], json!({ "k": "v" }));
        // Dates stay text.
        assert_eq!(map["date"], "2024-05-17");
        assert_eq!(map["1"], "one");
    }

    #[test]
    fn json_style_strings_from_the_exporter_parse_back() {
        let map =
            parse("summary: \"Line \\\"quoted\\\" \\u00e9 end\"\nname: \"a: b # c\"\n").unwrap();
        assert_eq!(map["summary"], "Line \"quoted\" \u{e9} end");
        assert_eq!(map["name"], "a: b # c");
    }

    #[test]
    fn invalid_yaml_and_non_mappings_are_errors() {
        assert!(parse("a: [unclosed").is_err());
        assert!(parse("- just\n- a list").is_err());
        assert!(parse("plain text").is_err());
        assert!(parse("").unwrap().is_empty());
        assert!(parse("# only a comment\n").unwrap().is_empty());
    }

    #[test]
    fn list_helpers() {
        assert_eq!(
            string_list(&json!(["a", " b ", 3, true, null, ""])),
            vec!["a", "b", "3", "true"]
        );
        assert_eq!(string_list(&json!("x, y,,z")), vec!["x", "y", "z"]);
        assert!(string_list(&json!({ "a": 1 })).is_empty());
        assert_eq!(scalar_text(&json!(2)), Some("2".into()));
        assert_eq!(scalar_text(&json!([1])), None);
    }
}
