# Changelog

All notable changes to Orbis Scriptor are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/Racso1230/orbis-scriptor/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/Racso1230/orbis-scriptor/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Racso1230/orbis-scriptor/releases/tag/v0.1.0
