//! Plain data structs shared across layers and sent to the frontend.
//!
//! Every model derives `Serialize`/`Deserialize` (serde, for the JSON that
//! crosses the Tauri bridge) and `specta::Type` (so tauri-specta can write the
//! matching TypeScript type into `src/lib/bindings.ts`).

pub mod app;
