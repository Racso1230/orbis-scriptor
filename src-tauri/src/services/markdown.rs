//! TipTap JSON to Markdown (Obsidian flavour). A pure function: no database,
//! no file system, so it is easy to test.
//!
//! TipTap stores a document as a tree of nodes, each a JSON object like
//! `{ "type": "paragraph", "content": [ ...child nodes... ] }`; text lives in
//! `{ "type": "text", "text": "...", "marks": [ { "type": "bold" } ] }`.
//! Converting a tree is naturally *recursive*: a function renders one node
//! by rendering its children first and then wrapping the result, e.g. a
//! blockquote renders its children and puts `> ` in front of every line.
//!
//! Supported nodes: paragraph, heading, bulletList, orderedList, taskList,
//! listItem, taskItem, blockquote, codeBlock, horizontalRule, hardBreak,
//! table (GFM), image, mention (turned into a `[[wikilink]]` by the caller's
//! resolver). Marks: bold, italic, strike, code, underline, highlight, link.
//! Anything else is rendered as the text it contains.

use serde_json::Value;

/// Turns an entity id (from a `mention` node) into the text to write, normally
/// a wikilink such as `[[Queen Isolde Varn]]`. `None` = the entity no longer
/// exists (written as plain text).
///
/// `&dyn Fn(&str) -> Option<String>` is a borrowed "any function with this
/// signature" (a trait object), so callers can pass a closure.
pub type Resolver<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Text written for a mention whose entity was deleted.
pub const MISSING_MENTION: &str = "(deleted entity)";

/// Converts a TipTap document to Markdown. The result has no trailing newline.
pub fn tiptap_to_markdown(doc: &Value, resolve: Resolver) -> String {
    let ctx = Ctx {
        resolve,
        in_table: false,
    };
    blocks(children(doc), &ctx)
}

/// Settings that travel down the recursion.
struct Ctx<'a> {
    resolve: Resolver<'a>,
    /// Inside a table cell: no line breaks, `|` must be escaped.
    in_table: bool,
}

fn children(node: &Value) -> &[Value] {
    node.get("content")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn kind(node: &Value) -> &str {
    node.get("type").and_then(Value::as_str).unwrap_or("")
}

fn attr<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("attrs").and_then(|a| a.get(name))
}

fn is_list(node: &Value) -> bool {
    matches!(kind(node), "bulletList" | "orderedList" | "taskList")
}

/// Inline nodes are rendered inside a paragraph-like block.
fn is_inline(node: &Value) -> bool {
    matches!(kind(node), "text" | "hardBreak" | "mention") || node.get("text").is_some()
}

/// Renders sibling block nodes separated by blank lines. Empty blocks (an
/// empty paragraph) are skipped.
fn blocks(nodes: &[Value], ctx: &Ctx) -> String {
    let mut parts: Vec<String> = Vec::new();
    // Consecutive inline nodes without a wrapping block (unknown containers)
    // are collected into one paragraph.
    let mut loose: Vec<&Value> = Vec::new();
    let flush = |loose: &mut Vec<&Value>, parts: &mut Vec<String>| {
        if !loose.is_empty() {
            let text = inline_nodes(loose.iter().copied(), ctx);
            if !text.trim().is_empty() {
                parts.push(escape_line_starts(&text));
            }
            loose.clear();
        }
    };
    for node in nodes {
        if is_inline(node) {
            loose.push(node);
            continue;
        }
        flush(&mut loose, &mut parts);
        let text = block(node, ctx);
        if !text.trim().is_empty() {
            parts.push(text);
        }
    }
    flush(&mut loose, &mut parts);
    parts.join("\n\n")
}

fn block(node: &Value, ctx: &Ctx) -> String {
    match kind(node) {
        "paragraph" => escape_line_starts(&inline_nodes(children(node).iter(), ctx)),
        "heading" => {
            let level = attr(node, "level")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 6) as usize;
            let text = inline_nodes(children(node).iter(), ctx).replace('\n', " ");
            format!("{} {}", "#".repeat(level), text.trim())
        }
        "bulletList" => list(node, ctx, |_| "- ".to_owned()),
        "orderedList" => {
            let start = attr(node, "start").and_then(Value::as_u64).unwrap_or(1);
            list(node, ctx, |i| format!("{}. ", start + i as u64))
        }
        "taskList" => list(node, ctx, |_| String::new()),
        "blockquote" => {
            let inner = blocks(children(node), ctx);
            inner
                .lines()
                .map(|l| {
                    if l.is_empty() {
                        ">".to_owned()
                    } else {
                        format!("> {l}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "codeBlock" => code_block(node),
        "horizontalRule" => "---".to_owned(),
        "table" => table(node, ctx),
        "image" => image(node),
        // Unknown container: render what is inside it.
        _ => blocks(children(node), ctx),
    }
}

// ---- lists ---------------------------------------------------------------

/// Renders a list. `marker(i)` gives the prefix of item `i`; task items add
/// their own `- [ ] ` / `- [x] ` marker (so their list marker is empty).
fn list(node: &Value, ctx: &Ctx, marker: impl Fn(usize) -> String) -> String {
    let mut out: Vec<String> = Vec::new();
    for (i, item) in children(node).iter().enumerate() {
        let mut prefix = marker(i);
        if kind(item) == "taskItem" {
            let done = attr(item, "checked")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            prefix = if done { "- [x] " } else { "- [ ] " }.to_owned();
        }
        let body = list_item_body(item, ctx);
        // The first line gets the marker; later lines are indented to line up
        // with the text after the marker (empty lines stay empty).
        let pad = " ".repeat(if kind(item) == "taskItem" {
            2
        } else {
            prefix.chars().count()
        });
        let mut lines = body.lines();
        let first = lines.next().unwrap_or("");
        let mut text = format!("{prefix}{first}");
        for line in lines {
            text.push('\n');
            if !line.is_empty() {
                text.push_str(&pad);
                text.push_str(line);
            }
        }
        out.push(text);
    }
    out.join("\n")
}

/// The blocks of one list item. A nested list directly follows the item's
/// text on the next line (a "tight" list); other blocks are separated by a
/// blank line.
fn list_item_body(item: &Value, ctx: &Ctx) -> String {
    let mut out = String::new();
    for child in children(item) {
        let text = block(child, ctx);
        if text.trim().is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str(if is_list(child) { "\n" } else { "\n\n" });
        }
        out.push_str(&text);
    }
    out
}

// ---- code, images, tables ------------------------------------------------

fn plain_text(node: &Value) -> String {
    if let Some(t) = node.get("text").and_then(Value::as_str) {
        return t.to_owned();
    }
    children(node).iter().map(plain_text).collect()
}

/// The shortest run of backticks (at least `min`) that does not occur in `text`.
fn fence_for(text: &str, min: usize) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(min.max(longest + 1))
}

fn code_block(node: &Value) -> String {
    let code = plain_text(node);
    let fence = fence_for(&code, 3);
    let lang = attr(node, "language").and_then(Value::as_str).unwrap_or("");
    format!("{fence}{lang}\n{}\n{fence}", code.trim_end_matches('\n'))
}

fn image(node: &Value) -> String {
    let src = attr(node, "src").and_then(Value::as_str).unwrap_or("");
    if src.is_empty() {
        return String::new();
    }
    let alt = attr(node, "alt").and_then(Value::as_str).unwrap_or("");
    format!("![{}]({})", escape_text(alt), link_target(src))
}

/// A GFM table. The first row becomes the header row (GFM requires one).
fn table(node: &Value, ctx: &Ctx) -> String {
    let cell_ctx = Ctx {
        resolve: ctx.resolve,
        in_table: true,
    };
    let rows: Vec<Vec<String>> = children(node)
        .iter()
        .filter(|r| kind(r) == "tableRow")
        .map(|row| {
            children(row)
                .iter()
                .map(|cell| {
                    // Paragraphs inside a cell are joined with <br>.
                    let parts: Vec<String> = children(cell)
                        .iter()
                        .map(|c| block(c, &cell_ctx).replace('\n', " "))
                        .filter(|t| !t.trim().is_empty())
                        .collect();
                    parts.join("<br>")
                })
                .collect()
        })
        .collect();
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return String::new();
    }
    let line = |cells: &[String]| {
        let mut padded: Vec<&str> = cells.iter().map(String::as_str).collect();
        padded.resize(columns, "");
        format!("| {} |", padded.join(" | "))
    };
    let mut out: Vec<String> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        out.push(line(row));
        if i == 0 {
            out.push(format!("|{}|", vec![" --- "; columns].join("|")));
        }
    }
    out.join("\n")
}

// ---- inline content --------------------------------------------------------

fn inline_nodes<'a>(nodes: impl Iterator<Item = &'a Value>, ctx: &Ctx) -> String {
    let mut out = String::new();
    for node in nodes {
        match kind(node) {
            "hardBreak" => out.push_str(if ctx.in_table { "<br>" } else { "  \n" }),
            "mention" => {
                let id = attr(node, "id").and_then(Value::as_str).unwrap_or("");
                let text = (ctx.resolve)(id).unwrap_or_else(|| MISSING_MENTION.to_owned());
                out.push_str(&if ctx.in_table {
                    escape_pipes(&text)
                } else {
                    text
                });
            }
            "image" => out.push_str(&image(node)),
            _ => {
                if let Some(text) = node.get("text").and_then(Value::as_str) {
                    out.push_str(&text_with_marks(text, node, ctx));
                } else {
                    // Unknown inline container: keep the text inside it.
                    out.push_str(&inline_nodes(children(node).iter(), ctx));
                }
            }
        }
    }
    out
}

fn has_mark(node: &Value, name: &str) -> bool {
    mark(node, name).is_some()
}

fn mark<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("marks")
        .and_then(Value::as_array)?
        .iter()
        .find(|m| kind(m) == name)
}

/// Wraps `text` in `open`/`close`, keeping surrounding spaces outside, since
/// `** bold **` is not bold in Markdown.
fn wrap(text: &str, open: &str, close: &str) -> String {
    let core = text.trim();
    if core.is_empty() {
        return text.to_owned();
    }
    let start = text.len() - text.trim_start().len();
    let end = text.trim_end().len();
    format!("{}{open}{core}{close}{}", &text[..start], &text[end..])
}

fn text_with_marks(text: &str, node: &Value, ctx: &Ctx) -> String {
    // Code is innermost and not escaped.
    let mut out = if has_mark(node, "code") {
        let fence = fence_for(text, 1);
        // A space pads text that starts or ends with a backtick.
        let pad = if text.starts_with('`') || text.ends_with('`') {
            " "
        } else {
            ""
        };
        format!("{fence}{pad}{text}{pad}{fence}")
    } else {
        escape_text(text)
    };
    if has_mark(node, "bold") {
        out = wrap(&out, "**", "**");
    }
    if has_mark(node, "italic") {
        out = wrap(&out, "*", "*");
    }
    if has_mark(node, "strike") {
        out = wrap(&out, "~~", "~~");
    }
    if has_mark(node, "highlight") {
        out = wrap(&out, "==", "==");
    }
    if has_mark(node, "underline") {
        out = wrap(&out, "<u>", "</u>");
    }
    if let Some(link) = mark(node, "link")
        && let Some(href) = link.pointer("/attrs/href").and_then(Value::as_str)
        && !href.is_empty()
    {
        out = format!("[{out}]({})", link_target(href));
    }
    if ctx.in_table {
        out = escape_pipes(&out);
    }
    out
}

// ---- escaping --------------------------------------------------------------

/// Escapes characters that would be read as Markdown or Obsidian syntax.
/// (A backslash before any ASCII punctuation is a legal Markdown escape.)
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let next = chars.get(i + 1).copied();
        let escape = match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '~' | '|' | '$' => true,
            // `==highlight==` and `%%comment%%` are Obsidian syntax.
            '=' => next == Some('=') || (i > 0 && chars[i - 1] == '='),
            '%' => next == Some('%') || (i > 0 && chars[i - 1] == '%'),
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Protects the start of every line of a paragraph: `# Title`, `- item`,
/// `1. item` and `---` written as plain text must not turn into headings,
/// lists or rules.
fn escape_line_starts(text: &str) -> String {
    text.split('\n')
        .map(escape_line_start)
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_line_start(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let mut chars = trimmed.chars();
    let first = chars.next();
    let second = chars.next();
    let escaped = match first {
        Some('#') => {
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            let after = trimmed.chars().nth(hashes);
            (hashes <= 6 && matches!(after, None | Some(' '))).then(|| format!("\\{trimmed}"))
        }
        Some('-' | '+') => {
            matches!(second, None | Some(' ') | Some('-')).then(|| format!("\\{trimmed}"))
        }
        Some(d) if d.is_ascii_digit() => {
            let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
            let punct = trimmed.chars().nth(digits);
            let after = trimmed.chars().nth(digits + 1);
            (matches!(punct, Some('.' | ')')) && matches!(after, None | Some(' '))).then(|| {
                let (num, rest) = trimmed.split_at(digits);
                format!("{num}\\{rest}")
            })
        }
        _ => None,
    };
    format!("{indent}{}", escaped.unwrap_or_else(|| trimmed.to_owned()))
}

/// Escapes unescaped `|` for use inside a table cell.
fn escape_pipes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_backslash = false;
    for c in text.chars() {
        if c == '|' && !prev_backslash {
            out.push('\\');
        }
        prev_backslash = c == '\\' && !prev_backslash;
        out.push(c);
    }
    out
}

/// A link/image target. Targets with spaces or parentheses are wrapped in
/// `<...>`, which CommonMark allows.
fn link_target(href: &str) -> String {
    if href
        .chars()
        .any(|c| c.is_whitespace() || c == '(' || c == ')')
    {
        format!("<{}>", href.replace('<', "%3C").replace('>', "%3E"))
    } else {
        href.to_owned()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(content: Value) -> Value {
        json!({ "type": "doc", "content": content })
    }

    fn p(text: &str) -> Value {
        json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
    }

    fn marked(text: &str, marks: &[&str]) -> Value {
        let marks: Vec<Value> = marks.iter().map(|m| json!({ "type": m })).collect();
        json!({ "type": "text", "text": text, "marks": marks })
    }

    fn para(inline: Vec<Value>) -> Value {
        json!({ "type": "paragraph", "content": inline })
    }

    fn resolver(id: &str) -> Option<String> {
        match id {
            "a" => Some("[[Ada]]".into()),
            "b" => Some("[[Bob File|Bob]]".into()),
            _ => None,
        }
    }

    fn md(d: Value) -> String {
        tiptap_to_markdown(&d, &resolver)
    }

    #[test]
    fn empty_documents_and_paragraphs() {
        assert_eq!(md(doc(json!([]))), "");
        assert_eq!(md(json!({ "type": "doc" })), "");
        let empty = json!({ "type": "paragraph" });
        assert_eq!(md(doc(json!([empty, p("A"), empty, p("B")]))), "A\n\nB");
    }

    #[test]
    fn headings_and_paragraphs() {
        let d = doc(json!([
            { "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "Lore" }] },
            p("Hello"),
            { "type": "heading", "attrs": { "level": 9 }, "content": [{ "type": "text", "text": "Deep" }] },
        ]));
        assert_eq!(md(d), "## Lore\n\nHello\n\n###### Deep");
    }

    #[test]
    fn marks_each_and_combined() {
        let d = doc(json!([para(vec![
            marked("b", &["bold"]),
            json!({ "type": "text", "text": " " }),
            marked("i", &["italic"]),
            json!({ "type": "text", "text": " " }),
            marked("s", &["strike"]),
            json!({ "type": "text", "text": " " }),
            marked("c", &["code"]),
            json!({ "type": "text", "text": " " }),
            marked("u", &["underline"]),
            json!({ "type": "text", "text": " " }),
            marked("h", &["highlight"]),
            json!({ "type": "text", "text": " " }),
            marked("both", &["bold", "italic"]),
        ]),]));
        assert_eq!(md(d), "**b** *i* ~~s~~ `c` <u>u</u> ==h== ***both***");
    }

    #[test]
    fn marks_keep_spaces_outside_and_code_is_not_escaped() {
        let d = doc(json!([para(vec![
            marked(" bold ", &["bold"]),
            marked("a*b_c", &["code"]),
            json!({ "type": "text", "text": " " }),
            marked("x`y", &["code"]),
        ])]));
        assert_eq!(md(d), " **bold** `a*b_c` ``x`y``");
    }

    #[test]
    fn links_and_escaping_of_targets() {
        let link = |text: &str, href: &str| {
            json!({ "type": "text", "text": text,
                    "marks": [{ "type": "link", "attrs": { "href": href } }] })
        };
        let d = doc(json!([para(vec![
            link("site", "https://example.com/a"),
            json!({ "type": "text", "text": " " }),
            link("odd", "https://example.com/a b(1)"),
        ])]));
        assert_eq!(
            md(d),
            "[site](https://example.com/a) [odd](<https://example.com/a b(1)>)"
        );
    }

    #[test]
    fn special_characters_are_escaped() {
        let d = doc(json!([
            p("1 * 2 _x_ [a](b) <tag> `c` ~~ a|b $5 \\ ==x== %%y%%"),
            p("# not a heading"),
            p("- not a list"),
            p("+ plus"),
            p("1. not ordered"),
            p("2) neither"),
            p("---"),
            p("#hashtag stays"),
            p("1.5 is fine"),
        ]));
        let out = md(d);
        assert_eq!(
            out,
            "1 \\* 2 \\_x\\_ \\[a\\](b) \\<tag\\> \\`c\\` \\~\\~ a\\|b \\$5 \\\\ \\=\\=x\\=\\= \\%\\%y\\%\\%\n\n\
             \\# not a heading\n\n\\- not a list\n\n\\+ plus\n\n1\\. not ordered\n\n2\\) neither\n\n\\---\n\n\
             #hashtag stays\n\n1.5 is fine"
        );
    }

    #[test]
    fn hard_breaks() {
        let d = doc(json!([para(vec![
            json!({ "type": "text", "text": "one" }),
            json!({ "type": "hardBreak" }),
            json!({ "type": "text", "text": "- two" }),
        ])]));
        assert_eq!(md(d), "one  \n\\- two");
    }

    #[test]
    fn bullet_ordered_and_nested_lists() {
        let item = |content: Value| json!({ "type": "listItem", "content": content });
        let d = doc(json!([
            { "type": "bulletList", "content": [
                item(json!([p("one"), { "type": "orderedList", "attrs": { "start": 3 }, "content": [
                    item(json!([p("three")])),
                    item(json!([p("four")])),
                ]}])),
                item(json!([p("two"), p("second paragraph")])),
            ]},
            { "type": "orderedList", "content": [
                item(json!([p("first")])),
                item(json!([p("second")])),
            ]},
        ]));
        assert_eq!(
            md(d),
            "- one\n  3. three\n  4. four\n- two\n\n  second paragraph\n\n1. first\n2. second"
        );
    }

    #[test]
    fn task_lists() {
        let task = |checked: bool, text: &str| json!({ "type": "taskItem", "attrs": { "checked": checked }, "content": [p(text)] });
        let d = doc(
            json!([{ "type": "taskList", "content": [task(false, "todo"), task(true, "done")] }]),
        );
        assert_eq!(md(d), "- [ ] todo\n- [x] done");
    }

    #[test]
    fn blockquotes_code_and_rules() {
        let d = doc(json!([
            { "type": "blockquote", "content": [p("quoted"), p("again")] },
            { "type": "codeBlock", "attrs": { "language": "rust" },
              "content": [{ "type": "text", "text": "let x = 1;\n\nlet y = 2;\n" }] },
            { "type": "codeBlock", "content": [{ "type": "text", "text": "```\nfenced\n```" }] },
            { "type": "horizontalRule" },
        ]));
        assert_eq!(
            md(d),
            "> quoted\n>\n> again\n\n```rust\nlet x = 1;\n\nlet y = 2;\n```\n\n````\n```\nfenced\n```\n````\n\n---"
        );
    }

    #[test]
    fn tables() {
        let cell = |kind: &str, text: &str| json!({ "type": kind, "content": [p(text)] });
        let d = doc(json!([{ "type": "table", "content": [
            { "type": "tableRow", "content": [cell("tableHeader", "Name"), cell("tableHeader", "Note")] },
            { "type": "tableRow", "content": [cell("tableCell", "a|b"), cell("tableCell", "x")] },
            { "type": "tableRow", "content": [cell("tableCell", "short")] },
        ]}]));
        assert_eq!(
            md(d),
            "| Name | Note |\n| --- | --- |\n| a\\|b | x |\n| short |  |"
        );
    }

    #[test]
    fn table_cells_with_mentions_and_two_paragraphs() {
        let cell = json!({ "type": "tableCell", "content": [
            para(vec![json!({ "type": "mention", "attrs": { "id": "b" } })]),
            p("more"),
        ]});
        let d = doc(json!([{ "type": "table", "content": [
            { "type": "tableRow", "content": [cell] },
        ]}]));
        assert_eq!(md(d), "| [[Bob File\\|Bob]]<br>more |\n| --- |");
    }

    #[test]
    fn mentions_become_links_and_missing_targets_plain_text() {
        let d = doc(json!([para(vec![
            json!({ "type": "text", "text": "See " }),
            json!({ "type": "mention", "attrs": { "id": "a" } }),
            json!({ "type": "text", "text": ", " }),
            json!({ "type": "mention", "attrs": { "id": "b" } }),
            json!({ "type": "text", "text": " and " }),
            json!({ "type": "mention", "attrs": { "id": "gone" } }),
        ])]));
        assert_eq!(md(d), "See [[Ada]], [[Bob File|Bob]] and (deleted entity)");
    }

    #[test]
    fn images() {
        let d = doc(json!([
            { "type": "image", "attrs": { "src": "https://x.test/a.png", "alt": "A [map]" } },
            { "type": "image", "attrs": { "src": "" } },
        ]));
        assert_eq!(md(d), "![A \\[map\\]](https://x.test/a.png)");
    }

    #[test]
    fn unknown_nodes_keep_their_text() {
        let d = doc(json!([
            { "type": "callout", "content": [p("inside"), { "type": "weird", "content": [
                { "type": "text", "text": "bare text" }
            ]}]},
            { "type": "leafWithNothing" },
        ]));
        assert_eq!(md(d), "inside\n\nbare text");
    }

    #[test]
    fn quote_containing_a_list() {
        let d = doc(json!([{ "type": "blockquote", "content": [
            { "type": "bulletList", "content": [
                { "type": "listItem", "content": [p("a")] },
                { "type": "listItem", "content": [p("b")] },
            ]}
        ]}]));
        assert_eq!(md(d), "> - a\n> - b");
    }
}
