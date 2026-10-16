# Orbis Scriptor — Project Guide for Claude Code

Orbis Scriptor is a local-first **desktop** app for fantasy worldbuilding: maps, lore, timelines, characters and notes. It targets **Windows and Linux**. There is no server and no internet requirement.

The full roadmap lives in `docs/DEVELOPMENT_PLAN.md`. Work through it **one phase at a time**.

## About the developer

- 4 years of PHP and PowerBuilder; strong SQL; some React; **new to Rust**.
- When you write Rust, add short comments explaining non-obvious concepts (ownership, borrowing, `Result`/`?`, traits, lifetimes) the first few times they appear.
- Prefer simple, readable Rust over clever Rust. Cloning to avoid fighting the borrow checker is acceptable unless it's in a hot path.
- At the end of each phase, give a short summary of what was built and anything new worth learning.

## Stack

| Area               | Choice                                                                          |
| ------------------ | ------------------------------------------------------------------------------- |
| Desktop shell      | Tauri 2                                                                         |
| Backend            | Rust (stable)                                                                   |
| Database           | SQLite via `rusqlite` (feature `bundled`), migrations via `rusqlite_migration`  |
| Search             | SQLite FTS5                                                                     |
| Rust ↔ TS types    | `tauri-specta` (generated bindings in `src/lib/bindings.ts`, never hand-edited) |
| Frontend           | React + TypeScript (strict) + Vite                                              |
| Styling            | Tailwind CSS + shadcn/ui                                                        |
| Server state       | TanStack Query                                                                  |
| UI state           | Zustand                                                                         |
| Routing            | TanStack Router or React Router (pick one in Phase 0, then stay consistent)     |
| Map                | Konva via `react-konva`                                                         |
| Rich text          | TipTap                                                                          |
| Relationship graph | React Flow (`@xyflow/react`)                                                    |
| Timeline           | Custom SVG component                                                            |
| Tests              | Vitest + Testing Library, Rust `#[test]`, Playwright (later)                    |
| CI                 | GitHub Actions (Windows + Linux)                                                |

Use the latest **stable** versions of each library. If a library's current API differs from what you expect, check its docs rather than guessing.

## Architecture rules

1. **The frontend never touches the database or file system directly.** Everything goes through Tauri commands.
2. Rust layers:
   - `commands/` — thin `#[tauri::command]` functions: validate input, call a service, map errors.
   - `services/` — business logic.
   - `db/` — SQL and row mapping (repository functions). Only this layer writes SQL.
   - `models/` — structs shared across layers, deriving `Serialize`, `Deserialize`, `specta::Type`.
3. Errors: one `AppError` enum (using `thiserror`) that serialises to the frontend as `{ kind, message }`. No `unwrap()` or `expect()` outside tests and startup code.
4. IDs: UUID v7 strings for every entity and row that can be linked.
5. Timestamps: stored as UTC ISO-8601 text.
6. In-world dates: stored as `i64` day numbers relative to the world's epoch, and formatted through a calendar definition. Never use real-world date types for in-world dates.
7. Database access: the open world's `rusqlite::Connection` lives in Tauri managed state behind a `Mutex<Option<WorldHandle>>`. Long-running work (indexing, imports, image processing) runs on a background thread.
8. Frontend features live in `src/features/<feature>/` (components, hooks, queries for that feature). Shared UI goes in `src/components/`.
9. All data fetching from the frontend goes through TanStack Query hooks that wrap the generated bindings.

## World format

A world is a folder chosen by the user:

```
MyWorld/
├─ world.json   # { app: "orbis-scriptor", name, formatVersion, createdAt }
├─ world.db     # SQLite database
└─ assets/      # imported images (map backgrounds, portraits), named by asset id
```

## Conventions

- TypeScript `strict: true`; no `any` without a comment explaining why.
- ESLint + Prettier for TS; `cargo fmt` + `cargo clippy -- -D warnings` for Rust.
- Every phase includes tests for its Rust services and key frontend logic.
- Keep `docs/DEVELOPMENT_PLAN.md` checkboxes up to date as tasks are completed.

## Git

- Remote: `origin` → `github.com/Racso1230/orbis-scriptor`, branch `main`.
- **Commit regularly throughout every phase**: one commit per meaningful step (e.g. a migration, a command + its tests, a UI component). Never one big commit at the end of a phase.
- Push to `origin` after each commit or small group of commits, so progress is never only local.
- Messages: conventional prefix (`feat:`, `fix:`, `chore:`, `test:`, `docs:`, `refactor:`) plus a **short** summary, ideally under 60 characters. One line; add a body only when the why isn't obvious. Example: `feat: add create_world command`.
- Make sure checks pass for the code you're committing; don't commit broken builds.

## Before finishing any phase

- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` pass
- [ ] `npm run lint`, `npm run typecheck`, `npm test` pass
- [ ] `npm run tauri dev` starts and the phase's acceptance criteria can be demonstrated
- [ ] Plan checkboxes updated
- [ ] Short summary written for the developer, then **stop and wait** for review before starting the next phase
