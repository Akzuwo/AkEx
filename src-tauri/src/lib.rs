mod commands;
mod database;
mod error;
mod filesystem;
mod indexer;
mod models;
mod search;
mod updater;
mod watcher;

use std::sync::Arc;

use commands::AppState;
use database::Database;
use indexer::IndexManager;
use tauri::Manager;
use watcher::WatcherManager;

pub(crate) const APP_ICON: tauri::image::Image<'static> =
    tauri::include_image!("./icons/128x128.png");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_drag::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(APP_ICON.clone())?;
            }
            let data_dir = app.path().app_data_dir()?;
            let database = Arc::new(Database::new(data_dir.join("akex-index.sqlite3")));
            database.initialize()?;
            let indexer = IndexManager::new(database.clone());
            let watcher = WatcherManager::new(database.clone(), app.handle().clone());
            app.manage(AppState {
                database,
                indexer,
                watcher,
            });
            updater::spawn_auto_update(app.handle().clone());
            log::info!(target: "database", "Akex database initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_volumes,
            commands::list_directory,
            commands::get_entry,
            commands::start_index,
            commands::cancel_index,
            commands::search_entries,
            commands::analyze_storage,
            commands::verify_index,
            commands::start_watchers,
            commands::open_path,
            commands::reveal_path,
            commands::open_window,
            commands::validate_drag_paths,
            commands::path_properties,
            commands::preview_file,
            commands::create_folder,
            commands::rename_entry,
            commands::delete_entries,
            commands::copy_entries,
            commands::move_entries,
            commands::refresh_directory,
        ])
        .run(tauri::generate_context!())
        .expect("Akex konnte nicht gestartet werden");
}
