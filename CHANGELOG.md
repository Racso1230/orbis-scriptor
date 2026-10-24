# Changelog

All notable changes to Orbis Scriptor are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-10-24

A big step up from 0.1: relationships, a much stronger map editor, custom calendars and entity types, Markdown export and Obsidian import, tags and portraits, and world generation from a seed.

### Added

- Relationships and graph: typed relationships between entities (parent of, spouse of, member of, rules and more, with matching wording from either side), a Relationships section on every entity page, an interactive graph focused on one entity with depth, relationship and entity-type filters, and a family-tree layout for characters. "Show in graph" and a palette action open it from anywhere.
- Map editor v2: blank maps on parchment, ocean or a plain colour; layers you can add, rename, reorder, hide and lock; river, road and border paths with smoothing; kingdom and region areas with fill styles that can link to entities; text labels, including labels that curve along a path; a library of map stamps (cities, castles, mountains, forests and more); undo and redo for every map change; and PNG export.
- Custom calendars: define your own months, weekdays, eras and leap rules, with a live preview. A world can have several calendars; the timeline and every date field can show any of them, and the same moment is shown in each. Timeline ticks follow real month and year boundaries, leap days included.
- Custom entity types and fields: create your own types (for example "Deity") with their own icon and colour, add fields to any type (text, long text, number, yes/no, select, multi-select, date, link to other entities, URL), and see links between entities in backlinks and the graph.
- Markdown export: export a world to a folder of Markdown files with front matter and `[[links]]` that open as an Obsidian vault, with images copied alongside. Any entity, or the whole world, can also be printed to PDF.
- Obsidian and Markdown import: bring an existing vault in through a wizard with a preview and a type mapping. Links, tags, front matter and images come across, and the whole import happens in one step or not at all.
- Tags, favourites and recent: tag entities (including nested tags such as `places/cities`), filter the sidebar by tag, search with `tag:name`, star favourites, and see recently edited entities first.
- Portraits and image galleries on any entity, shown as avatars in the sidebar, graph and search.
- World generation: create a world from a seed. Choose a land shape (continent, twin continents, archipelago, pangaea or coastline) and a climate (temperate, frozen, arid, tropical or volcanic) and get terrain with coasts, rivers and biomes; optionally add settlements, roads and names, and kingdoms with rulers, dynasties, wars and short lore entries. Everything generated is ordinary, editable content, and the same seed and settings always give the same world. Terrain can also be generated into an existing empty map.
- A new welcome screen with the Orbis Scriptor logo and wordmark.
- The sample world "The Shattered Coast" now includes relationships, a second calendar, a custom "Deity" type, tags and a portrait.

### Fixed

- The autosave indicator no longer stays on "Saving" while lists refresh.

## [0.1.1] - 2026-10-17

First signed release: installs from this version onward can update themselves in-app.

### Added

- Source-available licence (see `LICENSE`): read the code and use official builds; no copying, modification or redistribution without permission.

### Changed

- Release builds are signed for the in-app updater and publish `latest.json`; the release workflow falls back to unsigned installers when no signing key is configured.

## [0.1.0] - 2026-10-17

First release. A local-first desktop app for fantasy worldbuilding on Windows and Linux.

### Added

- Worlds: create or open a world folder (`world.json`, `world.db`, `assets/`), with a recent worlds list and versioned database migrations.
- Entities: characters, locations, factions, items, notes, maps and events, each with type-specific fields, an icon, a virtualised sidebar with filter, tabs that persist per world, and debounced autosave.
- Lore editor: rich text (headings, lists, quotes, links, tables) with a slash-command menu, written with TipTap.
- Wiki links: type `[[` to mention another entity (or create one on the spot); mentions follow renames, and every entity shows a backlinks panel with snippets.
- Search: instant full-text search (SQLite FTS5) over names, summaries and lore, with prefix matching and highlighted snippets, plus a `Ctrl+K` command palette with actions.
- Map editor: import a map image, pan and zoom, place and move pins linked to entities, and see where an entity appears on maps.
- Timeline: events as points or ranges on a zoomable SVG timeline, with a default calendar, involved entities and an events list.
- Settings: theme (light, dark, system), autosave delay and editor font size.
- Polish: global error boundary and error toasts, keyboard shortcuts (`Ctrl+K`, `Ctrl+N`, `Ctrl+W`, `Ctrl+S`), empty states and onboarding hints, and an optional sample world ("The Shattered Coast").
- Safety: an automatic daily backup of `world.db` when a world is opened (the newest 5 are kept).
- Release engineering: app icon and installer metadata (MSI and NSIS on Windows, deb and AppImage on Linux), signed in-app update checks via GitHub Releases, a tag-triggered release workflow, and Playwright end-to-end tests of the main flow on Windows.

[Unreleased]: https://github.com/Racso1230/orbis-scriptor/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/Racso1230/orbis-scriptor/compare/v0.1.1...v0.5.0
[0.1.1]: https://github.com/Racso1230/orbis-scriptor/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Racso1230/orbis-scriptor/releases/tag/v0.1.0
