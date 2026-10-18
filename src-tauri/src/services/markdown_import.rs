//! Markdown (Obsidian flavour) to TipTap JSON: the inverse of
//! `services::markdown`. A pure function: no database, no file system.
//!
//! How it works: `pulldown-cmark` turns the text into a flat stream of
//! events (`Start(Paragraph)`, `Text("hi")`, `End(Paragraph)`, ...). We walk
//! that stream with a small recursive-descent parser: `blocks` builds block
//! nodes, `inlines` builds the text inside them, and each `Start` event
//! recurses until its matching `End`.
//!
//! Supported (see the mapping in each function):
//! - headings, paragraphs, hard and soft line breaks (a soft break becomes a
//!   hard break, because Obsidian shows single line breaks);
//! - bold, italic, strike, inline code, links, `<u>` (underline) and
//!   `==highlight==`;
//! - bullet, ordered and task lists (nested), blockquotes, `> [!callouts]`,
//!   fenced code, horizontal rules, GFM tables, images;
//! - Obsidian: `[[Target]]`, `[[Target|Alias]]`, `[[Target#Heading]]`
//!   (a link to Target), `![[image.png]]` embeds, `#tags` and `%%comments%%`.
//!
//! Anything else (raw HTML blocks, footnotes, math) is dropped or kept as the
//! text it contains.
//!
//! Links to other notes are resolved by the caller through [`Hooks`], so the
//! two-pass import (create every entity first, then convert) stays outside
//! this module.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};
use serde_json::{Map, Value, json};

use crate::services::tags;

/// What to do with an image (or `![[embed]]`) found in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageAction {
    /// Keep an `image` node with the original `src`.
    Keep,
    /// Remove it from the text (the importer moves it to the gallery).
    Drop,
    /// Replace it with this plain text.
    Text(String),
}

/// What the converter needs from its caller.
///
/// A `trait` is a set of methods a type promises to have (like an
/// interface in PHP). `&mut dyn Hooks` is "any value that implements it".
pub trait Hooks {
    /// The entity id for a note name (`[[Target]]`, already stripped of any
    /// `#heading` and folder), or `None` when no note matches.
    fn resolve_note(&mut self, target: &str) -> Option<String>;
    /// Decides what happens to an image. `src` is the path or URL exactly as
    /// written (percent-decoded); `alt` the caption / alt text.
    fn image(&mut self, src: &str, alt: &str) -> ImageAction;
}

/// The result of converting one note.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    /// A TipTap document: `{ "type": "doc", "content": [...] }`.
    pub doc: Value,
    /// Inline `#tags` found in the text, without the `#`, unique, in order.
    /// The `#tag` text itself stays in the document.
    pub tags: Vec<String>,
}

/// File extensions that count as images in `![[embeds]]`.
const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"];

/// Stands in for an unescaped `==` while a paragraph is collected, so the
/// highlight pass can tell it from an escaped `\=\=`.
const HIGHLIGHT_MARK: char = '\u{E000}';

/// True when `path` ends in a known image extension.
pub fn is_image_path(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Converts a Markdown note body (front matter already removed).
pub fn convert(markdown: &str, hooks: &mut dyn Hooks) -> Converted {
    let cleaned = strip_comments(markdown);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_WIKILINKS);
    let events: Vec<(Event, Range<usize>)> = Parser::new_ext(&cleaned, options)
        .into_offset_iter()
        .collect();
    let mut conv = Conv {
        events,
        pos: 0,
        src: &cleaned,
        hooks,
        tags: Vec::new(),
    };
    let mut content = conv.blocks();
    // A stray unmatched `End` event would stop `blocks` early; skip it and
    // carry on so nothing after it is lost.
    while conv.pos < conv.events.len() {
        conv.pos += 1;
        content.extend(conv.blocks());
    }
    Converted {
        doc: json!({ "type": "doc", "content": content }),
        tags: conv.tags,
    }
}

// ---- %%comments%% ---------------------------------------------------------

/// Removes Obsidian `%%comments%%` (inline and multi-line), leaving fenced
/// code, inline code and escaped `\%` alone.
pub fn strip_comments(src: &str) -> String {
    if !src.contains("%%") {
        return src.to_owned();
    }
    let mut out = String::with_capacity(src.len());
    let mut in_comment = false;
    let mut fence: Option<(char, usize)> = None;
    for line in src.split_inclusive('\n') {
        let trimmed = line.trim_start();
        // Fenced code blocks: a line of 3+ backticks or tildes opens, the
        // same character (at least as many) closes.
        if !in_comment {
            let fence_char = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            if let Some(c) = fence_char {
                let run = trimmed.chars().take_while(|x| *x == c).count();
                match fence {
                    None if run >= 3 => fence = Some((c, run)),
                    Some((fc, n)) if fc == c && run >= n && trimmed[run..].trim().is_empty() => {
                        fence = None;
                    }
                    _ => {}
                }
                out.push_str(line);
                continue;
            }
            if fence.is_some() {
                out.push_str(line);
                continue;
            }
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        let mut in_code = false;
        while i < chars.len() {
            let c = chars[i];
            if in_comment {
                if c == '%' && chars.get(i + 1) == Some(&'%') {
                    in_comment = false;
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            if c == '\\' && i + 1 < chars.len() {
                out.push(c);
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '`' {
                in_code = !in_code;
            }
            if !in_code && c == '%' && chars.get(i + 1) == Some(&'%') {
                in_comment = true;
                i += 2;
                continue;
            }
            out.push(c);
            i += 1;
        }
        // A comment that ends on this line must not swallow the newline of
        // a line that only held the comment.
        if in_comment && !out.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

// ---- inline pieces ---------------------------------------------------------

/// A piece of inline content before it becomes JSON.
#[derive(Debug, Clone)]
enum Inl {
    Text { text: String, marks: Vec<Value> },
    Node(Value),
}

fn mark(name: &str) -> Value {
    json!({ "type": name })
}

fn link_mark(href: &str) -> Value {
    json!({ "type": "link", "attrs": { "href": href } })
}

fn is_mark(m: &Value, name: &str) -> bool {
    m.get("type").and_then(Value::as_str) == Some(name)
}

/// Splits a `==highlight==` run into text pieces carrying the highlight
/// mark. `HIGHLIGHT_MARK` characters toggle it; an unpaired last one is
/// written back as a literal `==`.
fn apply_highlights(items: Vec<Inl>) -> Vec<Inl> {
    let total: usize = items
        .iter()
        .map(|i| match i {
            Inl::Text { text, .. } => text.matches(HIGHLIGHT_MARK).count(),
            Inl::Node(_) => 0,
        })
        .sum();
    if total == 0 {
        return items;
    }
    let mut seen = 0usize;
    let mut open = false;
    let mut out: Vec<Inl> = Vec::new();
    for item in items {
        let Inl::Text { text, marks } = item else {
            out.push(item);
            continue;
        };
        let mut piece = String::new();
        let flush = |piece: &mut String, open: bool, out: &mut Vec<Inl>| {
            if piece.is_empty() {
                return;
            }
            let mut m = marks.clone();
            if open && !m.iter().any(|x| is_mark(x, "highlight")) {
                m.push(mark("highlight"));
            }
            out.push(Inl::Text {
                text: std::mem::take(piece),
                marks: m,
            });
        };
        for c in text.chars() {
            if c != HIGHLIGHT_MARK {
                piece.push(c);
                continue;
            }
            seen += 1;
            if seen == total && total % 2 == 1 {
                piece.push_str("=="); // unpaired: literal
            } else {
                flush(&mut piece, open, &mut out);
                open = !open;
            }
        }
        flush(&mut piece, open, &mut out);
    }
    out
}

/// Joins neighbouring text pieces that carry exactly the same marks.
fn merge_text(items: Vec<Inl>) -> Vec<Inl> {
    let mut out: Vec<Inl> = Vec::new();
    for item in items {
        if let (
            Inl::Text { text, marks },
            Some(Inl::Text {
                text: prev,
                marks: prev_marks,
            }),
        ) = (&item, out.last_mut())
            && marks == prev_marks
        {
            prev.push_str(text);
            continue;
        }
        out.push(item);
    }
    out
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// Finds Obsidian `#tags` in plain text. A tag starts after whitespace (or
/// at the start), is made of letters, digits, `_`, `-` and `/`, and has at
/// least one non-digit (`#1` is not a tag). Names that are not valid Orbis
/// tags (too long, empty nested parts) are skipped.
pub fn find_tags(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found: Vec<String> = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        if c != '#' || (i > 0 && !chars[i - 1].is_whitespace() && !matches!(chars[i - 1], '(')) {
            continue;
        }
        let name: String = chars[i + 1..]
            .iter()
            .take_while(|c| is_tag_char(**c))
            .collect();
        let name = name.trim_end_matches('/').to_owned();
        if name.is_empty() || name.chars().all(|c| c.is_ascii_digit() || c == '/') {
            continue;
        }
        if let Ok(valid) = tags::normalize_name(&name)
            && !found.iter().any(|f| f.eq_ignore_ascii_case(&valid))
        {
            found.push(valid);
        }
    }
    found
}

fn inline_to_json(item: Inl) -> Value {
    match item {
        Inl::Node(v) => v,
        Inl::Text { text, marks } => {
            let mut node = Map::new();
            node.insert("type".into(), json!("text"));
            node.insert("text".into(), json!(text));
            if !marks.is_empty() {
                node.insert("marks".into(), Value::Array(marks));
            }
            Value::Object(node)
        }
    }
}

// ---- percent-decoding ------------------------------------------------------

/// Decodes `%20`-style escapes (used in Markdown link and image targets).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// The note name a link points at: no `#heading` / `^block`, no folders, no
/// `.md`. `[[folder/My Note#Intro]]` becomes `My Note`.
pub fn note_name_of(target: &str) -> String {
    let no_anchor = target.split(['#', '^']).next().unwrap_or("");
    let last = no_anchor.rsplit('/').next().unwrap_or("").trim();
    last.strip_suffix(".md").unwrap_or(last).trim().to_owned()
}

// ---- the parser --------------------------------------------------------------

struct Conv<'a> {
    events: Vec<(Event<'a>, Range<usize>)>,
    pos: usize,
    src: &'a str,
    hooks: &'a mut dyn Hooks,
    tags: Vec<String>,
}

fn is_inline_start(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Emphasis
            | Tag::Strong
            | Tag::Strikethrough
            | Tag::Superscript
            | Tag::Subscript
            | Tag::Link { .. }
            | Tag::Image { .. }
    )
}

impl<'a> Conv<'a> {
    fn peek(&self) -> Option<&Event<'a>> {
        self.events.get(self.pos).map(|(e, _)| e)
    }

    /// True when the next event belongs to a paragraph's text.
    fn at_inline(&self) -> bool {
        match self.peek() {
            Some(Event::Start(tag)) => is_inline_start(tag),
            Some(
                Event::Text(_)
                | Event::Code(_)
                | Event::SoftBreak
                | Event::HardBreak
                | Event::InlineHtml(_)
                | Event::InlineMath(_)
                | Event::DisplayMath(_)
                | Event::FootnoteReference(_),
            ) => true,
            _ => false,
        }
    }

    // ---- blocks ----

    /// Builds sibling blocks until an `End` event (not consumed) or the end
    /// of the stream.
    fn blocks(&mut self) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        while let Some(event) = self.peek().cloned() {
            if matches!(event, Event::End(_)) {
                break;
            }
            if self.at_inline() {
                // Loose text (a tight list item has no Paragraph event).
                let items = self.inlines(&[]);
                out.extend(self.paragraph(items));
                continue;
            }
            self.pos += 1;
            match event {
                Event::Start(Tag::Paragraph) => {
                    let items = self.inlines(&[]);
                    self.pos += 1; // End(Paragraph)
                    out.extend(self.paragraph(items));
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let items = self.inlines(&[]);
                    self.pos += 1;
                    let content = self.finish_inlines(items);
                    if !content.is_empty() {
                        out.push(json!({
                            "type": "heading",
                            "attrs": { "level": level as usize },
                            "content": content,
                        }));
                    }
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    let mut inner = self.blocks();
                    self.pos += 1;
                    if inner.is_empty() {
                        inner.push(json!({ "type": "paragraph" }));
                    }
                    out.push(callout(json!({ "type": "blockquote", "content": inner })));
                }
                Event::Start(Tag::CodeBlock(kind)) => out.push(self.code_block(&kind)),
                Event::Start(Tag::List(first)) => out.push(self.list(first)),
                Event::Start(Tag::Table(_)) => out.push(self.table()),
                Event::Rule => out.push(json!({ "type": "horizontalRule" })),
                Event::Start(Tag::HtmlBlock) => {
                    // Raw HTML is dropped.
                    while let Some(e) = self.peek() {
                        let done = matches!(e, Event::End(TagEnd::HtmlBlock));
                        self.pos += 1;
                        if done {
                            break;
                        }
                    }
                }
                Event::Start(_) => {
                    // A container we do not model: keep what is inside it.
                    out.extend(self.blocks());
                    self.pos += 1;
                }
                // Html, TaskListMarker outside a list, ...: nothing to keep.
                _ => {}
            }
        }
        out
    }

    /// A paragraph node, or the images it consists of (as block images), or
    /// nothing when it is empty.
    fn paragraph(&mut self, items: Vec<Inl>) -> Vec<Value> {
        let content = self.finish_inlines(items);
        if content.is_empty() {
            return Vec::new();
        }
        let only_images = content
            .iter()
            .all(|n| n.get("type").and_then(Value::as_str) == Some("image"));
        if only_images {
            return content;
        }
        vec![json!({ "type": "paragraph", "content": content })]
    }

    fn code_block(&mut self, kind: &CodeBlockKind) -> Value {
        let mut code = String::new();
        while let Some(event) = self.peek().cloned() {
            self.pos += 1;
            match event {
                Event::Text(t) => code.push_str(&t),
                Event::End(TagEnd::CodeBlock) => break,
                _ => {}
            }
        }
        let code = code.strip_suffix('\n').unwrap_or(&code).to_owned();
        let mut node = Map::new();
        node.insert("type".into(), json!("codeBlock"));
        if let CodeBlockKind::Fenced(info) = kind
            && let Some(lang) = info.split_whitespace().next()
        {
            node.insert("attrs".into(), json!({ "language": lang }));
        }
        if !code.is_empty() {
            node.insert("content".into(), json!([{ "type": "text", "text": code }]));
        }
        Value::Object(node)
    }

    fn list(&mut self, first: Option<u64>) -> Value {
        let mut items: Vec<(Option<bool>, Vec<Value>)> = Vec::new();
        while matches!(self.peek(), Some(Event::Start(Tag::Item))) {
            self.pos += 1;
            let checked = match self.peek() {
                Some(Event::TaskListMarker(c)) => {
                    let c = *c;
                    self.pos += 1;
                    Some(c)
                }
                _ => None,
            };
            let mut content = self.blocks();
            self.pos += 1; // End(Item)
            if content.is_empty() {
                content.push(json!({ "type": "paragraph" }));
            }
            items.push((checked, content));
        }
        self.pos += 1; // End(List)
        let is_task = items.iter().any(|(c, _)| c.is_some());
        if is_task {
            let content: Vec<Value> = items
                .into_iter()
                .map(|(c, content)| {
                    json!({
                        "type": "taskItem",
                        "attrs": { "checked": c.unwrap_or(false) },
                        "content": content,
                    })
                })
                .collect();
            return json!({ "type": "taskList", "content": content });
        }
        let content: Vec<Value> = items
            .into_iter()
            .map(|(_, content)| json!({ "type": "listItem", "content": content }))
            .collect();
        match first {
            Some(start) => {
                let mut node = json!({ "type": "orderedList", "content": content });
                if start != 1 {
                    node["attrs"] = json!({ "start": start });
                }
                node
            }
            None => json!({ "type": "bulletList", "content": content }),
        }
    }

    fn table(&mut self) -> Value {
        let mut rows: Vec<Value> = Vec::new();
        while let Some(event) = self.peek().cloned() {
            self.pos += 1;
            match event {
                Event::Start(Tag::TableHead) => rows.push(self.table_row("tableHeader", true)),
                Event::Start(Tag::TableRow) => rows.push(self.table_row("tableCell", false)),
                Event::End(TagEnd::Table) => break,
                _ => {}
            }
        }
        json!({ "type": "table", "content": rows })
    }

    /// Cells until the end of the head / row. (The table head holds its
    /// cells directly; body rows are wrapped in a `TableRow`.)
    fn table_row(&mut self, cell_type: &str, _is_head: bool) -> Value {
        let mut cells: Vec<Value> = Vec::new();
        while let Some(event) = self.peek().cloned() {
            self.pos += 1;
            match event {
                Event::Start(Tag::TableCell) => {
                    let items = self.inlines(&[]);
                    self.pos += 1; // End(TableCell)
                    let content = self.finish_inlines(items);
                    let paragraph = if content.is_empty() {
                        json!({ "type": "paragraph" })
                    } else {
                        json!({ "type": "paragraph", "content": content })
                    };
                    cells.push(json!({ "type": cell_type, "content": [paragraph] }));
                }
                Event::End(TagEnd::TableHead | TagEnd::TableRow) => break,
                _ => {}
            }
        }
        json!({ "type": "tableRow", "content": cells })
    }

    // ---- inlines ----

    /// Collects inline pieces until an `End` event (not consumed) or the
    /// start of a block. `marks` are the marks of the surrounding emphasis,
    /// links, ... and are added to every text piece.
    fn inlines(&mut self, marks: &[Value]) -> Vec<Inl> {
        let mut out: Vec<Inl> = Vec::new();
        // `<u>` ... `</u>` arrive as two separate InlineHtml events.
        let mut underline = false;
        while let Some((event, range)) = self.events.get(self.pos).cloned() {
            let current = |underline: bool| -> Vec<Value> {
                let mut m = marks.to_vec();
                if underline {
                    m.push(mark("underline"));
                }
                m
            };
            match event {
                Event::End(_) => break,
                Event::Start(ref tag) if !is_inline_start(tag) => break,
                Event::Text(text) => {
                    self.pos += 1;
                    // An escaped character (`\=`) is a literal; only an
                    // unescaped `==` can open or close a highlight.
                    let escaped = text.len() == 1
                        && range.len() == 2
                        && self.src.as_bytes().get(range.start) == Some(&b'\\');
                    let text = if escaped {
                        text.to_string()
                    } else {
                        text.replace("==", &HIGHLIGHT_MARK.to_string())
                    };
                    out.push(Inl::Text {
                        text,
                        marks: current(underline),
                    });
                }
                Event::Code(code) => {
                    self.pos += 1;
                    let mut m = current(underline);
                    m.push(mark("code"));
                    out.push(Inl::Text {
                        text: code.to_string(),
                        marks: m,
                    });
                }
                Event::SoftBreak | Event::HardBreak => {
                    self.pos += 1;
                    out.push(Inl::Node(json!({ "type": "hardBreak" })));
                }
                Event::InlineHtml(html) | Event::Html(html) => {
                    self.pos += 1;
                    let tag = html.trim().to_ascii_lowercase();
                    match tag.as_str() {
                        "<br>" | "<br/>" | "<br />" => {
                            out.push(Inl::Node(json!({ "type": "hardBreak" })));
                        }
                        "<u>" => underline = true,
                        "</u>" => underline = false,
                        _ => {} // other tags are dropped, their text stays
                    }
                }
                Event::InlineMath(t) | Event::DisplayMath(t) => {
                    self.pos += 1;
                    out.push(Inl::Text {
                        text: format!("${t}$"),
                        marks: current(underline),
                    });
                }
                Event::FootnoteReference(_) | Event::TaskListMarker(_) | Event::Rule => {
                    self.pos += 1;
                }
                Event::Start(Tag::Emphasis | Tag::Superscript | Tag::Subscript) => {
                    self.pos += 1;
                    let mut m = current(underline);
                    m.push(mark("italic"));
                    out.extend(self.inlines(&m));
                    self.pos += 1;
                }
                Event::Start(Tag::Strong) => {
                    self.pos += 1;
                    let mut m = current(underline);
                    m.push(mark("bold"));
                    out.extend(self.inlines(&m));
                    self.pos += 1;
                }
                Event::Start(Tag::Strikethrough) => {
                    self.pos += 1;
                    let mut m = current(underline);
                    m.push(mark("strike"));
                    out.extend(self.inlines(&m));
                    self.pos += 1;
                }
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    ..
                }) => {
                    self.pos += 1;
                    let m = current(underline);
                    out.extend(self.link(link_type, &dest_url, &m));
                }
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    ..
                }) => {
                    self.pos += 1;
                    out.extend(self.image(link_type, &dest_url, &m_for(marks, underline)));
                }
                Event::Start(_) => {
                    self.pos += 1;
                }
            }
        }
        out
    }

    /// The pieces for a link whose `Start` was just consumed (consumes
    /// through its `End`).
    fn link(&mut self, kind: LinkType, dest: &str, marks: &[Value]) -> Vec<Inl> {
        let is_wiki = matches!(kind, LinkType::WikiLink { .. });
        let children = self.inlines(marks);
        self.pos += 1; // End(Link)
        let decoded = if is_wiki {
            // In a table cell `[[A\|B]]` leaves the escape's backslash on the target.
            dest.trim_end_matches('\\').to_owned()
        } else {
            percent_decode(dest)
        };

        // A link to another note: a wikilink, or a relative `.md` link.
        let note_like = is_wiki
            || (!has_scheme(&decoded)
                && !decoded.starts_with('#')
                && decoded.split(['#', '?']).next().is_some_and(|p| {
                    std::path::Path::new(p)
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
                }));
        if note_like {
            let name = note_name_of(&decoded);
            if !name.is_empty()
                && let Some(id) = self.hooks.resolve_note(&name)
            {
                return vec![Inl::Node(
                    json!({ "type": "mention", "attrs": { "id": id } }),
                )];
            }
            // Unresolved: just the visible text.
            return children;
        }
        let safe = ["http://", "https://", "mailto:"]
            .iter()
            .any(|p| decoded.to_ascii_lowercase().starts_with(p));
        let href = if kind == LinkType::Email && !decoded.starts_with("mailto:") {
            Some(format!("mailto:{decoded}"))
        } else if safe {
            Some(decoded)
        } else {
            None
        };
        match href {
            Some(href) => children
                .into_iter()
                .map(|c| match c {
                    Inl::Text { text, mut marks } => {
                        marks.push(link_mark(&href));
                        Inl::Text { text, marks }
                    }
                    other => other,
                })
                .collect(),
            None => children,
        }
    }

    /// An image or `![[embed]]` whose `Start` was just consumed.
    fn image(&mut self, kind: LinkType, dest: &str, marks: &[Value]) -> Vec<Inl> {
        let is_wiki = matches!(kind, LinkType::WikiLink { .. });
        // The alt text is the plain text inside.
        let mut alt = String::new();
        let mut depth = 0usize;
        while let Some(event) = self.peek().cloned() {
            self.pos += 1;
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) if depth == 0 => break,
                Event::End(_) => depth -= 1,
                Event::Text(t) | Event::Code(t) => alt.push_str(&t),
                Event::SoftBreak | Event::HardBreak => alt.push(' '),
                _ => {}
            }
        }
        let dest = if is_wiki {
            dest.trim_end_matches('\\')
        } else {
            dest
        };
        let src = if is_wiki {
            // `![[pic.png|300]]` and `![[pic.png#anchor]]`: keep the file.
            dest.split('#').next().unwrap_or("").trim().to_owned()
        } else {
            percent_decode(dest)
        };
        if is_wiki && !is_image_path(&src) {
            // `![[Some Note]]` embeds a note: treat it as a link to it.
            let name = note_name_of(dest);
            if let Some(id) = self.hooks.resolve_note(&name) {
                return vec![Inl::Node(
                    json!({ "type": "mention", "attrs": { "id": id } }),
                )];
            }
            return vec![Inl::Text {
                text: if alt.is_empty() || alt == dest {
                    name
                } else {
                    alt
                },
                marks: marks.to_vec(),
            }];
        }
        // For a wikilink embed the "alt" is the size or caption after `|`;
        // a pure number is a width, not a caption.
        let is_size = alt.chars().all(|c| c.is_ascii_digit() || c == 'x');
        let alt = if (is_wiki && alt == dest) || is_size {
            String::new()
        } else {
            alt
        };
        match self.hooks.image(&src, &alt) {
            ImageAction::Keep => {
                let mut attrs = Map::new();
                attrs.insert("src".into(), json!(src));
                if !alt.is_empty() {
                    attrs.insert("alt".into(), json!(alt));
                }
                vec![Inl::Node(json!({ "type": "image", "attrs": attrs }))]
            }
            ImageAction::Drop => Vec::new(),
            ImageAction::Text(text) => vec![Inl::Text {
                text,
                marks: marks.to_vec(),
            }],
        }
    }

    /// Final clean-up of one block's inline pieces: highlights, merged text,
    /// `#tags`, no hard break at the start or end.
    fn finish_inlines(&mut self, items: Vec<Inl>) -> Vec<Value> {
        let mut items = merge_text(apply_highlights(items));
        // Any highlight marker left over (no pairs) is a literal `==`.
        for item in &mut items {
            if let Inl::Text { text, .. } = item
                && text.contains(HIGHLIGHT_MARK)
            {
                *text = text.replace(HIGHLIGHT_MARK, "==");
            }
        }
        let is_break = |i: &Inl| matches!(i, Inl::Node(v) if v.get("type").and_then(Value::as_str) == Some("hardBreak"));
        while items.first().is_some_and(is_break) {
            items.remove(0);
        }
        while items.last().is_some_and(is_break) {
            items.pop();
        }
        for item in &items {
            if let Inl::Text { text, marks } = item
                && !marks
                    .iter()
                    .any(|m| is_mark(m, "code") || is_mark(m, "link"))
            {
                for tag in find_tags(text) {
                    if !self.tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                        self.tags.push(tag);
                    }
                }
            }
        }
        items
            .into_iter()
            .filter(|i| !matches!(i, Inl::Text { text, .. } if text.is_empty()))
            .map(inline_to_json)
            .collect()
    }
}

fn m_for(marks: &[Value], underline: bool) -> Vec<Value> {
    let mut m = marks.to_vec();
    if underline {
        m.push(mark("underline"));
    }
    m
}

/// `scheme:` at the start (`http:`, `mailto:`, `C:` ...).
fn has_scheme(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

// ---- callouts ------------------------------------------------------------------

/// Turns an Obsidian callout (`> [!note] Title` + lines) into a plain
/// blockquote whose first paragraph is the title in bold. Other
/// blockquotes are returned unchanged.
fn callout(mut quote: Value) -> Value {
    let Some(nodes) = quote
        .pointer("/content/0")
        .filter(|p| p.get("type").and_then(Value::as_str) == Some("paragraph"))
        .and_then(|p| p.get("content"))
        .and_then(Value::as_array)
        .cloned()
    else {
        return quote;
    };
    let Some(first_text) = nodes
        .first()
        .and_then(|n| n.get("text"))
        .and_then(Value::as_str)
    else {
        return quote;
    };
    let Some((kind, after)) = first_text
        .strip_prefix("[!")
        .and_then(|rest| rest.split_once(']'))
    else {
        return quote;
    };
    let kind_ok = !kind.is_empty()
        && kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !kind_ok {
        return quote;
    }
    // The title line ends at the first hard break; the rest is body text.
    let split = nodes
        .iter()
        .position(|n| n.get("type").and_then(Value::as_str) == Some("hardBreak"));
    let (head, tail) = match split {
        Some(i) => (&nodes[..i], nodes[i + 1..].to_vec()),
        None => (&nodes[..], Vec::new()),
    };
    // Optional fold marker (`+` / `-`) right after the bracket.
    let after = after.strip_prefix(['+', '-']).unwrap_or(after).trim();
    let mut title: Vec<Value> = Vec::new();
    if !after.is_empty() {
        title.push(json!({ "type": "text", "text": after }));
    }
    title.extend(head.iter().skip(1).cloned());
    if title.is_empty() {
        // No title given: use the callout type ("note" -> "Note").
        let mut name = kind.to_lowercase();
        if let Some(first) = name.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        title.push(json!({ "type": "text", "text": name }));
    }
    for node in &mut title {
        if node.get("text").is_some() {
            let marks = node
                .as_object_mut()
                .map(|o| o.entry("marks").or_insert_with(|| json!([])));
            if let Some(Value::Array(list)) = marks
                && !list.iter().any(|m| is_mark(m, "bold"))
            {
                list.push(mark("bold"));
            }
        }
    }
    let mut content: Vec<Value> = vec![json!({ "type": "paragraph", "content": title })];
    if !tail.is_empty() {
        content.push(json!({ "type": "paragraph", "content": tail }));
    }
    if let Some(old) = quote.get("content").and_then(Value::as_array) {
        content.extend(old.iter().skip(1).cloned());
    }
    quote["content"] = Value::Array(content);
    quote
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::markdown::tiptap_to_markdown;
    use std::collections::HashMap;

    /// Resolves note names (ignoring case) and records what was asked.
    #[derive(Default)]
    struct TestHooks {
        notes: HashMap<String, String>,
        images: Vec<(String, String)>,
        action: Option<ImageAction>,
    }

    impl TestHooks {
        fn with_notes(pairs: &[(&str, &str)]) -> Self {
            TestHooks {
                notes: pairs
                    .iter()
                    .map(|(n, id)| (n.to_lowercase(), (*id).to_owned()))
                    .collect(),
                ..Default::default()
            }
        }
    }

    impl Hooks for TestHooks {
        fn resolve_note(&mut self, target: &str) -> Option<String> {
            self.notes.get(&target.to_lowercase()).cloned()
        }
        fn image(&mut self, src: &str, alt: &str) -> ImageAction {
            self.images.push((src.to_owned(), alt.to_owned()));
            self.action.clone().unwrap_or(ImageAction::Keep)
        }
    }

    fn conv(md: &str) -> Converted {
        convert(md, &mut TestHooks::default())
    }

    fn blocks(md: &str) -> Vec<Value> {
        conv(md).doc["content"].as_array().unwrap().clone()
    }

    fn text(t: &str) -> Value {
        json!({ "type": "text", "text": t })
    }

    fn marked(t: &str, marks: &[&str]) -> Value {
        let marks: Vec<Value> = marks.iter().map(|m| json!({ "type": m })).collect();
        json!({ "type": "text", "text": t, "marks": marks })
    }

    fn para(content: Vec<Value>) -> Value {
        json!({ "type": "paragraph", "content": content })
    }

    #[test]
    fn headings_paragraphs_marks_and_breaks() {
        let got = blocks(
            "# Title\n\nSome **bold**, *italic*, ~~gone~~ and `code` with a [link](https://example.com).\n\nline one  \nline two\nline three",
        );
        assert_eq!(got[0]["type"], "heading");
        assert_eq!(got[0]["attrs"]["level"], 1);
        assert_eq!(got[0]["content"], json!([text("Title")]));
        let p = got[1]["content"].as_array().unwrap();
        assert_eq!(p[0], text("Some "));
        assert_eq!(p[1], marked("bold", &["bold"]));
        assert_eq!(p[3], marked("italic", &["italic"]));
        assert_eq!(p[5], marked("gone", &["strike"]));
        assert_eq!(p[7], marked("code", &["code"]));
        assert_eq!(
            p[9],
            json!({ "type": "text", "text": "link",
                    "marks": [{ "type": "link", "attrs": { "href": "https://example.com" } }] })
        );
        // Hard and soft line breaks both become hard breaks.
        let lines = got[2]["content"].as_array().unwrap();
        let breaks = lines.iter().filter(|n| n["type"] == "hardBreak").count();
        assert_eq!(breaks, 2);
    }

    #[test]
    fn lists_quotes_code_rules_and_tables() {
        let got = blocks(
            "- one\n  - nested\n- two\n\n3. third\n4. fourth\n\n- [ ] todo\n- [x] done\n\n> quoted\n>\n> more\n\n```rust\nlet x = 1;\n```\n\n---\n\n| A | B |\n| --- | --- |\n| 1 | 2 |",
        );
        let types: Vec<&str> = got.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(
            types,
            vec![
                "bulletList",
                "orderedList",
                "taskList",
                "blockquote",
                "codeBlock",
                "horizontalRule",
                "table"
            ]
        );
        // Nested list lives inside the first item, after its paragraph.
        let first = &got[0]["content"][0];
        assert_eq!(first["content"][0]["type"], "paragraph");
        assert_eq!(first["content"][1]["type"], "bulletList");
        assert_eq!(got[1]["attrs"]["start"], 3);
        assert_eq!(got[2]["content"][0]["attrs"]["checked"], false);
        assert_eq!(got[2]["content"][1]["attrs"]["checked"], true);
        assert_eq!(got[3]["content"].as_array().unwrap().len(), 2);
        assert_eq!(got[4]["attrs"]["language"], "rust");
        assert_eq!(got[4]["content"][0]["text"], "let x = 1;");
        let rows = got[6]["content"].as_array().unwrap();
        assert_eq!(rows[0]["content"][0]["type"], "tableHeader");
        assert_eq!(rows[1]["content"][1]["type"], "tableCell");
        assert_eq!(
            rows[1]["content"][1]["content"][0]["content"][0]["text"],
            "2"
        );
    }

    #[test]
    fn wikilinks_become_mentions_when_they_resolve() {
        let mut hooks = TestHooks::with_notes(&[("Queen Isolde", "id-q"), ("Port Veyra", "id-p")]);
        let md = "See [[Queen Isolde]], [[queen isolde|the Queen]], [[Port Veyra#History]], [[places/Port Veyra]], [[Nobody]] and [[Gone|someone]].";
        let c = convert(md, &mut hooks);
        let nodes = c.doc["content"][0]["content"].as_array().unwrap().clone();
        let ids: Vec<&str> = nodes
            .iter()
            .filter(|n| n["type"] == "mention")
            .map(|n| n["attrs"]["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["id-q", "id-q", "id-p", "id-p"]);
        let plain: String = nodes
            .iter()
            .filter_map(|n| n["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        // Unresolved links keep their visible text (alias, else target).
        assert!(plain.contains("Nobody"), "{plain}");
        assert!(plain.contains("someone"), "{plain}");
        assert!(!plain.contains("[["), "{plain}");
    }

    #[test]
    fn relative_markdown_links_to_notes_resolve_too() {
        let mut hooks = TestHooks::with_notes(&[("My Note", "id-n")]);
        let c = convert(
            "[go](My%20Note.md) and [out](https://x.org) and [bad](javascript:alert(1)) and [here](#top)",
            &mut hooks,
        );
        let nodes = c.doc["content"][0]["content"].as_array().unwrap().clone();
        assert_eq!(nodes[0]["type"], "mention");
        assert_eq!(nodes[0]["attrs"]["id"], "id-n");
        let link = nodes.iter().find(|n| n["text"] == "out").unwrap();
        assert_eq!(link["marks"][0]["attrs"]["href"], "https://x.org");
        // Unsafe and in-page links are plain text.
        assert!(tiptap_text(&c.doc).contains("bad"));
        assert_eq!(nodes.iter().filter(|n| n.get("marks").is_some()).count(), 1);
    }

    #[test]
    fn image_embeds_go_through_the_hook() {
        let mut hooks = TestHooks::with_notes(&[("Some Note", "id-s")]);
        let c = convert(
            "![[map one.png]]\n\ntext ![[portrait.jpg|300]] and ![[Some Note]] and ![fig](dir/a%20b.png)",
            &mut hooks,
        );
        assert_eq!(
            hooks.images,
            vec![
                ("map one.png".to_owned(), String::new()),
                ("portrait.jpg".to_owned(), String::new()),
                ("dir/a b.png".to_owned(), "fig".to_owned()),
            ]
        );
        // A paragraph that is only an image becomes a block image.
        assert_eq!(c.doc["content"][0]["type"], "image");
        assert_eq!(c.doc["content"][0]["attrs"]["src"], "map one.png");
        let nodes = c.doc["content"][1]["content"].as_array().unwrap();
        assert!(nodes.iter().any(|n| n["type"] == "mention"));

        // Drop removes the image (and an empty paragraph); Text replaces it.
        let mut hooks = TestHooks {
            action: Some(ImageAction::Drop),
            ..Default::default()
        };
        let c = convert("![[a.png]]\n\nafter", &mut hooks);
        assert_eq!(c.doc["content"].as_array().unwrap().len(), 1);
        let mut hooks = TestHooks {
            action: Some(ImageAction::Text("[missing.png]".into())),
            ..Default::default()
        };
        let c = convert("x ![[missing.png]]", &mut hooks);
        assert_eq!(c.doc["content"][0]["content"][0]["text"], "x [missing.png]");
    }

    #[test]
    fn inline_tags_are_collected_and_stay_in_the_text() {
        let c = conv("A #royal and #places/cities note, not #1 or a#b or `#code`.\n\n#second");
        assert_eq!(c.tags, vec!["royal", "places/cities", "second"]);
        let nodes = c.doc["content"][0]["content"].as_array().unwrap();
        assert!(nodes[0]["text"].as_str().unwrap().contains("#royal"));
        assert_eq!(
            find_tags("(#paren) #a_b-c #x/ #dup #DUP"),
            vec!["paren", "a_b-c", "x", "dup"]
        );
        assert!(find_tags("no tags # here, 5#6").is_empty());
        assert!(find_tags(&format!("#{}", "x".repeat(41))).is_empty());
    }

    #[test]
    fn comments_are_dropped_but_not_inside_code() {
        let md = "keep %%hidden%% this\n\n%%\nmulti\nline\n%%\n\nafter\n\n```\n%%in code%%\n```\n\n`%%x%%` and \\%\\%literal\\%\\%";
        let got = blocks(md);
        let plain = |b: &Value| tiptap_text(b);
        assert_eq!(plain(&got[0]), "keep  this");
        assert_eq!(plain(&got[1]), "after");
        assert_eq!(got[2]["content"][0]["text"], "%%in code%%");
        assert!(plain(&got[3]).contains("%%x%%"));
        assert!(plain(&got[3]).contains("%%literal%%"));
    }

    fn tiptap_text(node: &Value) -> String {
        if let Some(t) = node.get("text").and_then(Value::as_str) {
            return t.to_owned();
        }
        node.get("content")
            .and_then(Value::as_array)
            .map(|c| c.iter().map(tiptap_text).collect::<String>())
            .unwrap_or_default()
    }

    #[test]
    fn callouts_become_blockquotes_with_a_bold_title() {
        let got = blocks(
            "> [!note] Remember\n> the milk\n> and eggs\n\n> [!warning]\n> careful\n\n> plain quote",
        );
        assert_eq!(got[0]["type"], "blockquote");
        let title = &got[0]["content"][0]["content"][0];
        assert_eq!(title["text"], "Remember");
        assert_eq!(title["marks"][0]["type"], "bold");
        assert!(tiptap_text(&got[0]["content"][1]).contains("the milk"));
        // No title: the type, capitalised.
        assert_eq!(got[1]["content"][0]["content"][0]["text"], "Warning");
        assert_eq!(tiptap_text(&got[1]["content"][1]), "careful");
        // An ordinary quote is untouched.
        assert_eq!(tiptap_text(&got[2]), "plain quote");
        assert_eq!(got[2]["content"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn highlight_underline_and_br() {
        let got = blocks("a ==marked== b <u>under</u> c<br>d ==open and \\=\\=lit\\=\\=");
        let nodes = got[0]["content"].as_array().unwrap();
        assert!(nodes.contains(&marked("marked", &["highlight"])));
        assert!(nodes.contains(&marked("under", &["underline"])));
        assert!(nodes.iter().any(|n| n["type"] == "hardBreak"));
        // The unpaired `==` and the escaped ones are literal text.
        let tail = tiptap_text(&got[0]);
        assert!(tail.contains("==open"), "{tail}");
        assert!(tail.contains("==lit=="), "{tail}");
    }

    #[test]
    fn wikilinks_inside_table_cells_and_lists() {
        let mut hooks = TestHooks::with_notes(&[("Ada", "id-a")]);
        let c = convert(
            "| Who | Where |\n| --- | --- |\n| [[Ada]] | [[Ada\\|the Countess]] |\n\n- see [[Ada]]",
            &mut hooks,
        );
        let table = &c.doc["content"][0];
        let cell = &table["content"][1]["content"][0]["content"][0]["content"][0];
        assert_eq!(cell["type"], "mention");
        let cell2 = &table["content"][1]["content"][1]["content"][0]["content"][0];
        assert_eq!(cell2["type"], "mention");
        let item = &c.doc["content"][1]["content"][0]["content"][0]["content"];
        assert!(
            item.as_array()
                .unwrap()
                .iter()
                .any(|n| n["type"] == "mention")
        );
    }

    #[test]
    fn empty_and_odd_input_never_panics() {
        for md in [
            "",
            "\n\n",
            "   ",
            "[[",
            "]]",
            "[[]]",
            "![[]]",
            "![]()",
            "|",
            "| a |",
            "```",
            "~~~~\n",
            "- ",
            "1.",
            "> ",
            "#",
            "%%",
            "%% unclosed",
            "==",
            "<",
            "<u>",
            "\u{0}",
            "[[a|]]",
            "[[|a]]",
            "- [ ]",
            "***",
            "<div>raw</div>\n\ntext after",
        ] {
            let c = conv(md);
            assert_eq!(c.doc["type"], "doc", "{md:?}");
        }
        // Raw HTML blocks are dropped, the text after them stays.
        assert_eq!(
            tiptap_text(&conv("<div>raw</div>\n\ntext after").doc),
            "text after"
        );
    }

    // ---- round trip ------------------------------------------------------

    /// Sorts marks and merges neighbouring equal text pieces so two
    /// documents that mean the same thing compare equal.
    fn normalise(node: &Value) -> Value {
        let mut out = node.clone();
        normalise_in_place(&mut out);
        out
    }

    fn normalise_in_place(node: &mut Value) {
        if let Some(marks) = node.get_mut("marks").and_then(Value::as_array_mut) {
            marks.sort_by_key(|m| m["type"].as_str().unwrap_or("").to_owned());
        }
        if let Some(content) = node.get_mut("content").and_then(Value::as_array_mut) {
            for child in content.iter_mut() {
                normalise_in_place(child);
            }
            let mut merged: Vec<Value> = Vec::new();
            for child in content.drain(..) {
                if let Some(last) = merged.last_mut()
                    && last.get("text").is_some()
                    && child.get("text").is_some()
                    && last.get("marks") == child.get("marks")
                {
                    let joined = format!(
                        "{}{}",
                        last["text"].as_str().unwrap(),
                        child["text"].as_str().unwrap()
                    );
                    last["text"] = json!(joined);
                    continue;
                }
                merged.push(child);
            }
            *content = merged;
        }
    }

    fn round_trip(doc: &Value) -> Value {
        let names: HashMap<&str, &str> = [("id-queen", "Queen Isolde"), ("id-port", "Port Veyra")]
            .into_iter()
            .collect();
        let resolver = |id: &str| names.get(id).map(|n| format!("[[{n}]]"));
        let md = tiptap_to_markdown(doc, &resolver);
        let mut hooks =
            TestHooks::with_notes(&[("Queen Isolde", "id-queen"), ("Port Veyra", "id-port")]);
        let back = convert(&md, &mut hooks);
        assert!(back.tags.is_empty() || md.contains('#'));
        back.doc
    }

    fn assert_round_trips(doc: Value) {
        let back = round_trip(&doc);
        assert_eq!(normalise(&back), normalise(&doc), "markdown was:\n{}", {
            let names: HashMap<&str, &str> = HashMap::new();
            let resolver = |id: &str| names.get(id).map(|n: &&str| format!("[[{n}]]"));
            tiptap_to_markdown(&doc, &resolver)
        });
    }

    #[test]
    fn round_trip_marks_and_mentions() {
        assert_round_trips(json!({ "type": "doc", "content": [
            { "type": "heading", "attrs": { "level": 2 }, "content": [
                text("A "), marked("Title", &["italic"]) ] },
            para(vec![
                text("Plain, "), marked("bold", &["bold"]), text(", "),
                marked("both", &["bold", "italic"]), text(", "),
                marked("gone", &["strike"]), text(", "), marked("code", &["code"]),
                text(", "), marked("under", &["underline"]), text(", "),
                marked("lit", &["highlight"]), text(" and "),
                json!({ "type": "text", "text": "a link",
                        "marks": [{ "type": "link", "attrs": { "href": "https://example.com/a?b=1" } }] }),
                text(". Then "),
                json!({ "type": "mention", "attrs": { "id": "id-queen" } }),
                text(" met "),
                json!({ "type": "mention", "attrs": { "id": "id-port" } }),
                text("."),
            ]),
            para(vec![text("one"), json!({ "type": "hardBreak" }), text("two")]),
        ]}));
    }

    #[test]
    fn round_trip_escaped_text() {
        assert_round_trips(json!({ "type": "doc", "content": [
            para(vec![text("snake_case *not italic* [not a link] <tag> a|b ~tilde~ $5 `tick`")]),
            para(vec![text("# not a heading")]),
            para(vec![text("- not a list")]),
            para(vec![text("1. not ordered")]),
            para(vec![text("==not highlighted== and %%not a comment%%")]),
            para(vec![text("back\\slash and [[not a mention]]")]),
        ]}));
    }

    #[test]
    fn round_trip_blocks() {
        let item = |t: &str| json!({ "type": "listItem", "content": [para(vec![text(t)])] });
        assert_round_trips(json!({ "type": "doc", "content": [
            { "type": "bulletList", "content": [
                { "type": "listItem", "content": [
                    para(vec![text("outer")]),
                    { "type": "orderedList", "attrs": { "start": 4 }, "content": [item("inner a"), item("inner b")] },
                ]},
                item("second"),
            ]},
            { "type": "orderedList", "content": [item("first"), item("second")] },
            { "type": "taskList", "content": [
                { "type": "taskItem", "attrs": { "checked": false }, "content": [para(vec![text("todo")])] },
                { "type": "taskItem", "attrs": { "checked": true }, "content": [para(vec![text("done")])] },
            ]},
            { "type": "blockquote", "content": [
                para(vec![text("quoted "), marked("bold", &["bold"])]),
                para(vec![text("second paragraph")]),
            ]},
            { "type": "codeBlock", "attrs": { "language": "rust" },
              "content": [text("fn main() {\n    println!(\"hi\");\n}")] },
            { "type": "codeBlock", "content": [text("plain ```fence``` inside")] },
            { "type": "horizontalRule" },
            { "type": "table", "content": [
                { "type": "tableRow", "content": [
                    { "type": "tableHeader", "content": [para(vec![text("Name")])] },
                    { "type": "tableHeader", "content": [para(vec![text("Role")])] } ]},
                { "type": "tableRow", "content": [
                    { "type": "tableCell", "content": [para(vec![text("Ada")])] },
                    { "type": "tableCell", "content": [
                        para(vec![marked("Queen", &["bold"]), text(" | ruler")])] } ]},
            ]},
            { "type": "image", "attrs": { "src": "assets/pic one.png", "alt": "A picture" } },
            para(vec![text("The end.")]),
        ]}));
    }
}
