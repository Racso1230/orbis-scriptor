# Orbis Scriptor — Development Plan

Read `CLAUDE.md` first. Implement **one phase per session**, in order. Each phase ends with working, tested software and a stop for review.

**Version 0.1 (MVP)** = Phases 0–8. Phases 9+ are the roadmap beyond it.

---

## Phase 0 — Project scaffold

**Goal:** an empty but properly structured app that builds and runs on Windows and Linux.

- [x] Create a Tauri 2 app with React + TypeScript + Vite in the repo root (`orbis-scriptor`)
- [x] App identifier `com.racso1230.orbisscriptor`, window title "Orbis Scriptor", sensible default window size (1400×900, min 1000×650)
- [x] Tailwind CSS + shadcn/ui configured, with light and dark themes using CSS variables
- [x] Choose and configure a router — **decision: TanStack Router, code-based routes in `src/router.tsx`, hash history**
- [x] ESLint, Prettier, `typecheck` script; `rustfmt` and `clippy` config
- [x] Folder structure:
  ```
  src/
    features/          # one folder per feature
    components/        # shared UI (incl. shadcn components)
    lib/               # bindings.ts (generated), query client, utils
    stores/            # Zustand stores
  src-tauri/src/
    commands/  services/  db/  models/  error.rs  state.rs  lib.rs
  docs/
  ```
- [x] `AppError` enum with `thiserror`, serialised to `{ kind, message }`
- [x] `tauri-specta` set up; one example command `app_info()` returning name + version, called from React through the generated bindings and shown on screen
- [x] TanStack Query provider set up
- [x] Vitest + Testing Library with one passing test; one Rust unit test
- [x] GitHub Actions workflow: on push/PR run lint, typecheck, tests, clippy, and build on `windows-latest` and `ubuntu-latest` (install WebKitGTK deps on Linux)
- [x] README: what the app is, prerequisites (Rust, Node, Linux system packages), how to run, test and build

**Acceptance:** `npm run tauri dev` opens a themed window showing the app name and version fetched from Rust. CI is green on both platforms.

---

## Phase 1 — Worlds: create, open, migrate

**Goal:** the user can create a new world folder or open an existing one.

- [x] Welcome screen: "New world", "Open world", list of recent worlds
- [x] Use the Tauri dialog plugin for folder picking
- [x] `create_world(path, name)`: creates `world.json`, `world.db`, `assets/`; refuses non-empty folders that aren't worlds
- [x] `open_world(path)`: validates `world.json`, opens the database, runs pending migrations, stores the handle in managed state
- [x] `close_world()`
- [x] Migrations with `rusqlite_migration`; migration 001 creates a `meta` table (key/value) and enables `PRAGMA foreign_keys = ON` and WAL mode on every connection
- [x] Before running migrations on an existing world, copy `world.db` to `world.db.bak`
- [x] Recent worlds list stored in the app config directory (not inside worlds) as `recent-worlds.json`, capped at 10; missing folders shown as unavailable with a "remove" option
- [x] App shell once a world is open: left sidebar, main content area, top bar with world name and a "close world" action
- [x] Tests: create → open → close round trip; opening an invalid folder returns a clear error; migrations are idempotent

**Acceptance:** create a world, close the app, reopen it from the recent list, and land in the (empty) app shell.

---

## Phase 2 — Entities

**Goal:** create, view, edit and delete the core things in a world.

**Schema (migration 002):**

```sql
CREATE TABLE entities (
  id          TEXT PRIMARY KEY,          -- UUID v7
  type        TEXT NOT NULL,             -- 'character' | 'location' | 'faction' | 'item' | 'note'
  name        TEXT NOT NULL,
  summary     TEXT NOT NULL DEFAULT '',
  properties  TEXT NOT NULL DEFAULT '{}', -- JSON, type-specific fields
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
CREATE INDEX idx_entities_type_name ON entities(type, name);
```

- [x] Built-in types: character, location, faction, item, note. Each type has an icon and a small field definition (e.g. character: species, age, status; location: kind, region) defined in **one** shared place (`src-tauri/src/models/entity_type.rs`, served by the `entity_types` command), so adding a type later is easy
- [x] Commands: `list_entities(type?)`, `get_entity(id)`, `create_entity`, `update_entity`, `delete_entity`
- [x] Sidebar: entities grouped by type, collapsible groups, "new" button per type, filter box. Use a virtualised list so thousands of entities stay fast (`@tanstack/react-virtual`)
- [x] Entity page: editable name, type badge, summary, type-specific fields form
- [x] Debounced autosave with a visible "Saved / Saving…" indicator (`use-autosave.ts`, 800 ms, flushes on unmount)
- [x] Delete with confirmation dialog
- [x] Tabs: opening entities opens tabs in the main area (Zustand store); tabs persist per world
- [x] Tests: repository CRUD; properties JSON round trip; service validation (empty name rejected)

**Acceptance:** create a character and a location, edit their fields, restart the app and see the changes preserved.

---

## Phase 3 — Lore editor

**Goal:** every entity has a rich text document.

**Schema (migration 003):**

```sql
CREATE TABLE documents (
  entity_id   TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
  content     TEXT NOT NULL,   -- TipTap JSON
  plain_text  TEXT NOT NULL,   -- extracted for search
  updated_at  TEXT NOT NULL
);
```

- [x] TipTap editor on the entity page below the fields: headings, bold/italic, lists, quotes, horizontal rule, links, tables (`src/features/lore/lore-editor.tsx`; StarterKit + TableKit)
- [x] Slash-command menu for block types (`slash-command.ts` on `@tiptap/suggestion`)
- [x] Commands: `get_document(entity_id)`, `save_document(entity_id, content, plain_text)`
- [x] Debounced autosave (shared with the Phase 2 indicator — the entity page shows the "worst" of the fields and lore states)
- [x] Editor styling that works in light and dark themes (`@tailwindcss/typography` `prose` / `dark:prose-invert`)
- [x] Tests: save/load round trip; cascade delete when the entity is deleted

**Acceptance:** write formatted lore for a character, restart, and see it exactly as written.

---

## Phase 4 — Wiki links and backlinks

**Goal:** lore entries link to each other, and every entity shows what links to it.

**Schema (migration 004):**

```sql
CREATE TABLE links (
  id          TEXT PRIMARY KEY,
  source_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
  target_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,   -- 'mention' (from documents) or a relationship kind
  label       TEXT,
  created_at  TEXT NOT NULL,
  UNIQUE (source_id, target_id, kind)
);
CREATE INDEX idx_links_target ON links(target_id);
```

- [x] Custom TipTap node: typing `[[` opens an entity picker; the inserted node stores the **entity id** and displays the current name (so renames update automatically) — `src/features/lore/mention.tsx`
- [x] Option to create a new entity from the picker if no match exists (one "Create … " row per type)
- [x] Clicking a mention opens that entity in a tab
- [x] On `save_document`, the Rust service extracts mention ids from the content and syncs `kind = 'mention'` rows in `links` inside a transaction (`services/links.rs`)
- [x] Deleted targets render as a "missing entity" style instead of breaking the document
- [x] Backlinks panel on each entity page: lists entities that mention it, with a text snippet
- [x] Tests: mention extraction from TipTap JSON; link sync adds and removes correctly; rename shows in mentions (verified live; names resolve from the entity list at render time)

**Acceptance:** mention a location from a character's lore, open the location and see the character in its backlinks.

---

## Phase 5 — Search

**Goal:** instant search across all names, summaries and lore.

- [x] Migration 005: FTS5 virtual table over entity name, summary and document plain text (use the `unicode61` tokenizer with `remove_diacritics`)
- [x] Keep the index in sync from the service layer within the same transaction as each write (or via triggers; document the choice) — **chosen: service layer, same transaction** (lore plain text arrives from the editor, triggers would be invisible)
- [x] `search(query, limit)` returns entity id, type, name and a highlighted snippet, ranked with `bm25`
- [x] Prefix matching so results appear while typing
- [x] Command palette on `Ctrl+K`: search results plus actions ("New character", "Close world"…)
- [x] `rebuild_search_index()` command for recovery, exposed in a settings/debug menu (gear menu in the header + palette action)
- [x] Tests: indexing on create/update/delete; prefix search; snippet highlighting; special characters don't break queries

**Acceptance:** with 1,000 generated test entities (add a dev-only seed command), search results appear as you type with no visible lag. _(Manual lag check still pending.)_

---

## Phase 6 — Map editor v1

**Goal:** import a map image, pan and zoom, and place pins linked to entities.

**Schema (migration 006):**

```sql
CREATE TABLE assets (
  id TEXT PRIMARY KEY, file_name TEXT NOT NULL, mime TEXT NOT NULL,
  width INTEGER, height INTEGER, created_at TEXT NOT NULL
);
CREATE TABLE maps (
  id TEXT PRIMARY KEY,
  entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE, -- a map is also an entity (type 'map')
  background_asset_id TEXT REFERENCES assets(id),
  width INTEGER NOT NULL, height INTEGER NOT NULL
);
CREATE TABLE map_layers (
  id TEXT PRIMARY KEY, map_id TEXT NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  name TEXT NOT NULL, sort_order INTEGER NOT NULL, visible INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE map_objects (
  id TEXT PRIMARY KEY, layer_id TEXT NOT NULL REFERENCES map_layers(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,          -- 'pin' now; 'path' | 'area' | 'label' in Phase 10
  geometry TEXT NOT NULL,      -- JSON, in map pixel coordinates
  style TEXT NOT NULL DEFAULT '{}',
  entity_id TEXT REFERENCES entities(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
```

- [x] Add `map` as an entity type
- [x] `import_asset(path)`: copies the file into `assets/` (named by id), reads dimensions with the `image` crate, generates a thumbnail on a background thread
- [x] Serve asset files to the webview safely (Tauri asset protocol scoped to the open world's `assets/` folder)
- [x] Map view with react-konva: background image, mouse-wheel zoom around the cursor, drag to pan, zoom limits, "fit to screen"
- [x] Toolbar with Select and Pin tools (tool state in Zustand)
- [x] Pin tool: click to place; choose or create the linked entity; pin icon reflects the entity type
- [x] Select tool: select, drag to move (saved on drop), delete key removes, hover shows entity name
- [x] Double-clicking a pin opens its entity; entity pages list "appears on maps" with a jump-to link
- [x] One default layer per map (multi-layer UI comes in Phase 10)
- [ ] Performance check: 8000×6000 background image with 500 pins stays smooth on Windows **and** Linux (WebKitGTK). Record results in `docs/performance.md`; if Linux is slow, implement tiling or Konva layer caching before moving on
- [x] Tests: asset import; geometry round trip; deleting an entity unlinks (not deletes) its pins

**Acceptance:** import a fantasy map, drop pins for three locations, double-click one to open its lore.

---

## Phase 7 — Timeline v1

**Goal:** record events and see them on a zoomable timeline.

**Schema (migration 007):**

```sql
CREATE TABLE calendars (
  id TEXT PRIMARY KEY, name TEXT NOT NULL,
  definition TEXT NOT NULL,    -- JSON: months [{name, days}], weekdays [], eras [{name, startDay}]
  is_default INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE events (
  entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE, -- an event is an entity (type 'event')
  calendar_id TEXT NOT NULL REFERENCES calendars(id),
  start_day INTEGER NOT NULL,  -- day number from world epoch (may be negative)
  end_day INTEGER,             -- NULL = single-day event
  importance INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_events_start ON events(start_day);
```

- [x] Add `event` as an entity type; new worlds get one default calendar (12 months × 30 days, one era)
- [x] Rust calendar module: convert day number ↔ (era, year, month, day) and format for display, with thorough unit tests including negative years
- [x] Commands: `list_events(from_day, to_day)` (range query for the visible window only), create/update event dates
- [x] SVG timeline view: horizontal axis, zoom from centuries down to days, drag to pan, adaptive tick labels
- [x] Events as points, ranges as bars; overlapping events stack into lanes
- [x] Click an event to open it; double-click empty space to create an event at that date
- [x] Event pages show "involved" entities using `links` (kind `involved_in`)
- [x] Tests: calendar conversions; range queries; lane-stacking algorithm

**Acceptance:** create a war (range) and a coronation (point) linked to characters, then zoom between the decade and day views.

---

## Phase 8 — Polish and v0.1 release

**Goal:** a stable, installable first release.

- [x] Settings screen: theme (light/dark/system), autosave delay, editor font size
- [x] Global error boundary and toast notifications for command errors
- [x] Keyboard shortcuts: `Ctrl+K` palette, `Ctrl+N` new entity, `Ctrl+W` close tab, `Ctrl+S` force save
- [x] Empty states and onboarding hints for a brand-new world
- [x] Optional sample world ("The Shattered Coast") with a few linked entities, a map and events
- [x] Automatic daily backup of `world.db` on open (keep last 5)
- [x] Playwright end-to-end tests for: create world → create entity → write lore with a mention → search → place a pin → create an event _(Windows via WebView2 CDP; Linux e2e is manual for now)_
- [x] Release workflow: on tag `v*`, build `.msi`/`.exe` (Windows) and `.AppImage`/`.deb` (Linux), attach to a GitHub Release _(untested until the first tag is pushed)_
- [x] Tauri updater configured against GitHub Releases (signing keys documented in README, not committed)
- [x] App icon and installer metadata
- [x] CHANGELOG.md

**Acceptance:** install v0.1.0 from the GitHub Release on a clean Windows machine and a clean Linux machine and complete the end-to-end flow manually.

---

## Beyond v0.1 (roadmap)

### Phase 9 — Relationship graph (v0.2)

- [x] Typed relationships between entities (parent of, member of, ally of, rules…) stored in `links`, with a relationship editor on entity pages
- [x] React Flow graph view: focus on one entity and expand N degrees, filter by relationship and entity type, auto-layout (e.g. ELK or dagre)
- [x] Family-tree layout mode for characters
- Backend (done): a built-in relationship catalog (key, forward/inverse label, symmetric flag, category, entity-type constraints) plus free-text `custom` relationships, all stored as `links` rows; commands to list kinds, list/create/update/delete relationships, `get_entity_graph` (BFS by degrees with kind/type filters and a node cap) and `get_family_tree` (nodes with generation numbers). `mention` and `involved_in` links are never touched by relationship code. The sample world has family, faction and place relationships, and `dev_seed_relationships(count)` makes random ones for performance tests. Frontend (done): a Relationships section on every entity page (grouped list, add with inverse wording, edit, delete), a React Flow graph view (dagre layout, depth, kind/type filters, mentions toggle) and a family-tree layout, reachable from the sidebar, the palette and "Show in graph".
- **Tests:** catalog validation, symmetric canonical storage, perspective labels, mention/relationship isolation, graph depth/filter/truncation, family-tree generations, cascade on entity delete, sample-world relationships (Rust); layout and filter logic (Vitest).
- **Acceptance:** in the sample world, give Queen Isolde a parent, a spouse and a faction; open the graph focused on her, expand to 2 degrees, filter to family only, and switch to family-tree layout.

### Phase 10 — Map editor v2 (v0.2)

- [x] Blank map: create a map without an image — choose a size and a background (parchment, ocean, plain colour) — so the drawing tools work from scratch
- [x] Multiple layers: add, rename, reorder, hide, lock
- [x] Freehand and path tools (rivers, roads, borders) with smoothing
- [x] Area/polygon tool (kingdoms, regions) with fill styles; areas can link to entities
- [x] Text labels, including labels curved along paths
- [x] Icon/stamp library (cities, castles, mountains, forests)
- [x] Undo/redo for all map operations
- [x] Export map as PNG

**Backend status:** the Phase 10 Rust backend is done (migration 009, blank maps, layer commands and locking, path/area/label/stamp geometry, client ids and atomic `apply_map_ops` for undo/redo, PNG `save_export`, sample-map shapes, `dev_seed_map_shapes`). Frontend (done so far): blank-map creation and backgrounds, layers panel, rendering of every object kind, Path/Freehand/Area tools, properties panel and undo/redo. Labels (straight and curved along paths), a 19-stamp library with a Stamp tool, and PNG export are done; the Playwright flow covers freehand, dragging, vertex handles, labels, stamps, undo and export.

**Tests:** Rust services for blank maps, layers (order, visibility, locking), every geometry variant (round trip and validation), label detach on path delete, client-provided ids, atomic op batches and PNG export; Vitest for the undo/redo stack, path smoothing and label placement along a path; Playwright flow for drawing, undo/redo and export.

**Acceptance:** create a blank parchment map, add a 'Borders' layer, draw a river path, a kingdom area linked to a faction, a label curved along the river and a few stamps; undo/redo several steps; hide and lock layers; export to PNG and open the file.

### Phase 11 — Custom calendars and eras (v0.3)

- [x] Calendar editor: months and lengths, weekdays, eras, optional leap rules
  - Leap rules: one month gains N extra days in leap years; Gregorian-style cycles ("every 4 add, every 100 remove, every 400 add"), evaluated on the year count from the calendar's epoch (negative years work)
  - Epoch offset: each calendar can be shifted against the world's absolute day numbers
  - Create, edit, duplicate, delete and set-default for calendars; live preview of an unsaved definition with field-level validation messages
- [x] Multiple calendars per world with conversion between them
  - All calendars share the world's absolute day numbers, so converting a date is formatting the same day with another calendar; events never move when a calendar is edited
  - The timeline and every date editor can switch between calendars; the chosen display calendar is stored per world
- Backend (done): migration 010 (`world_settings`, unique calendar names); `CalendarDefinition` gains `epochOffset` and an optional `leap` rule (nested Gregorian-style cycles, O(rules) maths, valid to +/-10^12 days); calendar commands `get/create/update/delete/duplicate/set_default/preview_calendar`, `days_in_month`, `get/set_display_calendar`; deleting a calendar moves its events to the default; a second calendar ("Elven Reckoning") in the sample world; shared golden fixtures in `src/features/timeline/calendar-fixtures.json` (regenerated by `cargo test`) that the TypeScript calendar port must reproduce.
- Frontend (done): the TypeScript calendar port reproduces the Rust maths (checked against the golden fixtures); timeline ticks follow real year and month boundaries (leap days included); a display-calendar switcher in the timeline toolbar (stored per world); event date editors can be entered in any calendar and show the same date in the others; a Calendars screen (`/world/calendars`, from the gear menu, the command palette and the switcher) with list, set-default, duplicate, delete, an editor for months, weekdays, eras, epoch offset and leap rules, and a live preview with field-level messages.
- **Tests:** leap-year arithmetic against brute force, round trips (including leap days and +/-10^12), epoch shifts, weekday continuity, definition validation, calendar CRUD (delete reassigns events, default/last calendar protected), shared golden fixtures that keep the TypeScript calendar port in lockstep (Rust and Vitest); Playwright flow for the editor.
- **Acceptance:** create a second calendar with 13 months, a leap day every 4 years except every 100, and two eras; switch the timeline and an event's date editor between calendars and see the same moment shown in each.

### Phase 12 — Templates and export (v0.3)

- [x] User-defined entity types and field templates
  - Custom types: label, plural, icon (curated lucide allow-list) and colour (`#RRGGBB`, used for pins and graph nodes); key `custom-<slug>`, immutable once created; at most 50 custom types, labels unique (case-insensitive) across all types
  - Field templates: built-in types keep their fixed fields and can gain extra user fields; custom types own all of theirs (at most 40 per type, keys `[a-z][a-z0-9_]{0,39}` unique per type and never colliding with built-in field keys)
  - Field kinds: text, long text, number, boolean, select, multi-select, date (in-world day number, shown with the display calendar), entity reference (one or many, optionally limited to some types) and URL
  - Removing a field or changing its kind never destroys stored values; unknown keys in an entity's properties are kept but not validated; deleting a type needs a "move entities to" target when entities of that type exist
  - Entity-reference values also create `field_ref` links, so backlinks and the graph (opt-in edge kind) see them
- [x] Export world to Markdown folder (Obsidian-compatible links)
  - One folder per type, one `.md` per entity with YAML frontmatter (fields, dates, `[[links]]`), the lore converted from TipTap JSON to Markdown, a Relationships section, and an index `README.md`
  - Links use the final unique file name (`[[File|Display name]]`); map background images are copied to `assets/` and embedded
  - Runs on a background thread with progress events; written to a temporary folder and renamed at the end, so a failed export leaves nothing half-written
- [x] Export an entity or the whole world to PDF
  - Print-friendly HTML view of an entity (or of the whole world) printed through the webview's print dialog ("Save as PDF"); no Rust PDF engine

**Backend status:** the Phase 12 Rust backend is done: migration 011 (`custom_entity_types`, `type_templates`); `entity_types` now returns the merged list (built-ins with `builtin`/`color`, extra user fields flagged `custom`, then custom types) and can fail; commands `list_type_icons`, `create_entity_type`, `update_entity_type`, `delete_entity_type` (with `reassignTo`), `set_type_template_fields`, `reorder_entity_types`; field kinds longText, boolean, date, entityRef, multiSelect and url with per-kind validation of changed values (unknown keys are kept); entity-reference fields mirrored as `field_ref` links (opt-in graph edge kind); TipTap to Markdown converter; `export_markdown` on a background thread with typed `export-progress` / `export-finished` events; a "Deity" custom type with two deities in the sample world. PDF export needs no backend (print-friendly view in the frontend).

**Frontend status (done):** an Entity types screen (`/world/types`, from the gear menu, the palette and "Manage types…" in the sidebar) with a type and field editor, reorder and delete-with-move; inputs for every field kind (dates in the display calendar, entity references as chips with a picker); every allow-listed icon and the type colour used in the sidebar, pins and graph; a `field_ref` toggle in the graph filters; an "Export to Markdown…" dialog with progress that keeps running when closed; and print views (`/print/entity/$id`, `/print/world`, outside the app shell) that open the system print dialog ("Save as PDF").

**Tests:** migration 011 on an existing database; merged type list; type and field validation (label uniqueness, key collisions, icon and colour, limits); property validation per field kind; `field_ref` link sync; type delete and reassign; TipTap to Markdown converter for every node and mark, escaping and mentions; file name sanitising, collisions and Windows reserved names; full export of the sample world (files, frontmatter, links resolve, assets copied, no temporary folder left, overwrite refusal); Vitest for the field form and type editor; Playwright flow for creating a type and exporting.

**Acceptance:** create a 'Deity' type with Domain (select), Symbol (text) and Worshippers (entity reference to factions) fields; create two deities; export the sample world to Markdown, open the folder in Obsidian and follow `[[links]]` between entities; print a character to PDF.

### Phase 13 — Import and quality of life (v0.4)

- [ ] Import from a Markdown/Obsidian vault
  - Wizard: pick a vault folder, see a preview (notes per type, images, tags, warnings), map folders that match no type to an existing type or a new custom type, then import with progress
  - Pass 1 creates an entity per `.md` note (type from `type_key`, then `type`, then the folder name, else the default type); pass 2 converts the bodies so `[[Target]]`, `[[Target|Alias]]` and `[[Target#Heading]]` become mentions (resolved by file name or `aliases`, case-insensitive); unresolved links stay as text, or become Note entities when asked
  - Markdown becomes lore: headings, lists (nested, tasks), blockquotes and `> [!callouts]`, code, rules, GFM tables, bold/italic/strike/code/links; `%%comments%%` are dropped; `#tags` become entity tags; `![[image.png]]` embeds become gallery images
  - Front matter: `summary`, `tags`, `aliases`, field values by key (per field kind, dates from `<key>_day`), event dates, and unknown keys kept as text properties; Orbis exports also bring back their relationships and event participants, and their generated sections (`## Events`, `## Pins`, ...) are skipped
  - Runs on a background thread in one transaction (all or nothing) with `import-progress` / `import-finished` events
  - **Tests:** Markdown to TipTap converter (every node, mark and Obsidian syntax), TipTap to Markdown to TipTap round trip, front matter per field kind, relationship re-import, vault walking and size limits, preview counts, atomic rollback, and a full round trip: export the sample world, import it into a fresh world and compare entities, fields, tags, relationships, mentions, event dates and gallery images
  - **Acceptance:** export the sample world to Markdown, import that vault into a new world and see the same entities, fields, tags, links and lore; import a hand-made Obsidian vault with nested folders, `#tags` and images
- [ ] Tags, favourites, recently edited
  - Tags: case-insensitive unique names of 1 to 40 characters (letters, digits, `-`, `_`, spaces and `/` for nested tags such as `places/cities`), optional colour; rename merges into an existing tag; deleting a tag untags its entities
  - Sidebar filter by one or several tags (any/all); `tag:name` in search narrows results (and includes nested tags); tags are searchable text and written to the Markdown export
  - Favourites: a star per entity and a favourites list; recently edited: entities ordered by last change (entity edits, lore saves, event date changes)
  - **Tests:** tag rules (trim, length, characters, nesting, case), set/replace/rename/merge/delete, `tag:` search, FTS migration keeps data, favourites and recent ordering (lore save bumps the time)
  - **Acceptance:** tag and favourite entities, filter the sidebar by tag, search `tag:royalty dragon`, and see the most recently edited entities first
- [ ] Image galleries and portraits on entity pages
  - Any entity can hold several images (imported files or assets already in the world such as map backgrounds) with captions and an order; one image can be the portrait, shown as an avatar in the sidebar, graph and pins
  - Removing an image deletes its file only when no other gallery or map uses it; gallery images and the portrait are exported to Markdown
  - **Tests:** add/reorder/caption/remove, portrait set and clear (at most one), asset reference counting, cascade on entity delete, export with images
  - **Acceptance:** set a portrait and add gallery images to a character, reorder them, remove one, and see the portrait in the sidebar

### Later

- Optional sync/collaboration (Yjs-based), plugin system, procedural map generation

---

## How to use this plan with Claude Code

1. Put `CLAUDE.md` in the repo root and this file in `docs/`.
2. Start each session with:
   > Read CLAUDE.md and docs/DEVELOPMENT_PLAN.md. Implement Phase N. Commit and push regularly with short messages, run all checks, update the checkboxes, then stop and summarise.
3. Review, test the acceptance criteria yourself, then start the next phase in a fresh session.
