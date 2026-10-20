//! Orbis Scriptor backend. `main.rs` is a thin wrapper that calls [`run`].
//!
//! Layers (see CLAUDE.md): `commands` -> `services` -> `db`, with `models`
//! shared by all and `error` providing the one `AppError` type.

pub mod commands;
pub mod db;
pub mod error;
pub mod models;
pub mod services;
pub mod state;

use tauri_specta::{Builder, collect_commands, collect_events};

/// Registers every command with tauri-specta. Used both at startup and by the
/// test that regenerates `src/lib/bindings.ts`.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::app::app_info,
            commands::app::e2e_dir,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::world::create_world,
            commands::world::create_sample_world,
            commands::world::open_world,
            commands::world::close_world,
            commands::world::current_world,
            commands::world::list_recent_worlds,
            commands::world::remove_recent_world,
            commands::entities::entity_types,
            commands::entities::list_entities,
            commands::entities::get_entity,
            commands::entities::create_entity,
            commands::entities::update_entity,
            commands::entities::delete_entity,
            commands::entities::set_favorite,
            commands::entities::list_favorites,
            commands::entities::list_recent,
            commands::gallery::add_entity_image,
            commands::gallery::add_entity_image_from_asset,
            commands::gallery::list_entity_images,
            commands::gallery::update_entity_image,
            commands::gallery::reorder_entity_images,
            commands::gallery::remove_entity_image,
            commands::gallery::set_portrait,
            commands::gallery::list_portraits,
            commands::tags::list_tags,
            commands::tags::set_entity_tags,
            commands::tags::rename_tag,
            commands::tags::delete_tag,
            commands::tags::set_tag_color,
            commands::tags::list_entities_by_tag,
            commands::entity_types::list_type_icons,
            commands::entity_types::create_entity_type,
            commands::entity_types::update_entity_type,
            commands::entity_types::delete_entity_type,
            commands::entity_types::set_type_template_fields,
            commands::entity_types::reorder_entity_types,
            commands::documents::get_document,
            commands::documents::save_document,
            commands::links::get_backlinks,
            commands::search::search,
            commands::search::rebuild_search_index,
            commands::search::dev_seed_entities,
            commands::maps::import_asset,
            commands::maps::get_asset,
            commands::maps::get_map,
            commands::maps::set_map_background,
            commands::maps::create_map_object,
            commands::maps::update_map_object,
            commands::maps::delete_map_object,
            commands::maps::list_entity_map_appearances,
            commands::maps::dev_seed_map_pins,
            commands::maps::dev_seed_map_shapes,
            commands::maps::create_blank_map,
            commands::maps::set_map_background_style,
            commands::maps::set_map_size,
            commands::maps::create_layer,
            commands::maps::rename_layer,
            commands::maps::set_layer_visibility,
            commands::maps::set_layer_locked,
            commands::maps::reorder_layers,
            commands::maps::delete_layer,
            commands::maps::apply_map_ops,
            commands::export::save_export,
            commands::export::export_markdown,
            commands::import::preview_import,
            commands::import::import_vault,
            commands::generate::preview_generation,
            commands::generate::generate_map,
            commands::calendars::get_default_calendar,
            commands::calendars::list_calendars,
            commands::calendars::format_day,
            commands::calendars::format_days,
            commands::calendars::date_parts,
            commands::calendars::day_from_parts,
            commands::calendars::get_calendar,
            commands::calendars::create_calendar,
            commands::calendars::update_calendar,
            commands::calendars::delete_calendar,
            commands::calendars::set_default_calendar,
            commands::calendars::duplicate_calendar,
            commands::calendars::preview_calendar,
            commands::calendars::days_in_month,
            commands::calendars::get_display_calendar,
            commands::calendars::set_display_calendar,
            commands::events::list_events,
            commands::events::get_event,
            commands::events::create_event,
            commands::events::update_event_dates,
            commands::events::list_involved,
            commands::events::add_involved,
            commands::events::remove_involved,
            commands::events::list_events_for_entity,
            commands::events::dev_seed_events,
            commands::relationships::list_relationship_kinds,
            commands::relationships::list_relationships,
            commands::relationships::create_relationship,
            commands::relationships::update_relationship,
            commands::relationships::delete_relationship,
            commands::relationships::get_entity_graph,
            commands::relationships::get_family_tree,
            commands::relationships::dev_seed_relationships,
        ])
        // Typed events: the frontend listens with `events.exportProgress.listen(...)`.
        .events(collect_events![
            models::export::ExportProgress,
            models::export::ExportFinished,
            models::import::ImportProgress,
            models::import::ImportFinished,
            models::generate::GenerateProgress,
            models::generate::GenerateFinished,
        ])
}

/// Writes the TypeScript bindings next to the frontend code.
/// Runs only in debug builds and in `cargo test`, never in a release binary.
#[cfg(any(debug_assertions, test))]
fn export_bindings(builder: &Builder<tauri::Wry>) -> Result<(), specta_typescript::Error> {
    use specta_typescript::Typescript;

    builder.export(
        Typescript::default().header("/* eslint-disable */\n// Generated by tauri-specta. Do not edit by hand; run `cargo test` to regenerate."),
        "../src/lib/bindings.ts",
    )
}

/// Builds and starts the Tauri application.
///
/// `expect` is allowed here because this is startup code: if the app cannot
/// start there is nothing sensible to do except exit with the error.
#[allow(clippy::expect_used)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    export_bindings(&builder).expect("failed to export TypeScript bindings");

    tauri::Builder::default()
        // Native open/save dialogs (folder picker on the welcome screen).
        .plugin(tauri_plugin_dialog::init())
        // In-app updates from GitHub Releases (see README, "Releasing").
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Needed to relaunch after an update has been installed.
        .plugin(tauri_plugin_process::init())
        .manage(state::AppState::default())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            // Registers the typed events (export-progress, export-finished).
            builder.mount_events(app);
            Ok(())
        })
        .run(app_context())
        .expect("error while running tauri application");
}

/// The Tauri context (config from `tauri.conf.json` plus embedded assets).
///
/// `mut` lets us adjust the config before the window is created. In debug
/// builds started by the end-to-end tests we turn on WebView2's DevTools port
/// here, because some WebView2 versions ignore the
/// `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` environment variable.
fn app_context() -> tauri::Context<tauri::Wry> {
    #[allow(unused_mut)] // only mutated in debug builds
    let mut context = tauri::generate_context!();

    #[cfg(debug_assertions)]
    if let Some(port) = e2e_cdp_port() {
        // `iter_mut` hands out mutable references so we can edit each window
        // config in place. Setting browser args replaces Tauri's defaults, so
        // the default `--disable-features` flag is repeated here.
        for window in context.config_mut().app.windows.iter_mut() {
            window.additional_browser_args = Some(format!(
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
                 --remote-debugging-port={port}"
            ));
        }
    }

    context
}

/// The DevTools port for an end-to-end run, or `None` for normal use.
/// Only active when `ORBIS_E2E_DIR` is set (see `commands::app::e2e_dir`).
#[cfg(debug_assertions)]
fn e2e_cdp_port() -> Option<u16> {
    std::env::var("ORBIS_E2E_DIR")
        .ok()
        .filter(|d| !d.trim().is_empty())?;
    // `.ok()` turns the `Result` into an `Option`, and `?` returns `None`
    // early if the variable is missing or not a valid port number.
    let port = std::env::var("E2E_CDP_PORT").unwrap_or_else(|_| "9222".into());
    port.trim().parse().ok()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Regenerates `src/lib/bindings.ts` so it never drifts from the Rust side.
    #[test]
    fn exports_typescript_bindings() {
        export_bindings(&specta_builder()).expect("bindings export");
    }
}
