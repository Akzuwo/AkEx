use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use tauri::{AppHandle, State};

use crate::{
    database::Database,
    error::AppError,
    filesystem::{self, PathProperties},
    indexer::IndexManager,
    models::{Entry, Page, StorageAnalysis, VerificationResult, Volume},
    search::SearchQuery,
    watcher::WatcherManager,
};

pub struct AppState {
    pub database: Arc<Database>,
    pub indexer: IndexManager,
    pub watcher: WatcherManager,
}

type CommandResult<T> = Result<T, AppError>;

#[tauri::command]
pub fn list_volumes(state: State<'_, AppState>) -> CommandResult<Vec<Volume>> {
    for volume in filesystem::discover_volumes()? {
        state
            .database
            .ensure_volume(
                &volume.volume_id,
                &volume.root_path,
                volume.label.as_deref(),
                volume.filesystem_type.as_deref(),
                volume.total_bytes,
                volume.free_bytes,
            )
            .map_err(AppError::from)?;
    }
    state.database.list_volumes().map_err(AppError::from)
}

#[tauri::command]
pub fn list_directory(
    state: State<'_, AppState>,
    path: String,
    offset: u64,
    limit: u64,
    sort_field: String,
    sort_direction: String,
) -> CommandResult<Page<Entry>> {
    let path = filesystem::normalize_path(&path).map_err(AppError::from)?;
    state
        .database
        .list_directory(&path, offset, limit, &sort_field, &sort_direction)
        .map_err(|error| {
            log::debug!(target: "database", "Directory query failed for {path}: {error:#}");
            AppError::new(
                "not_indexed",
                "Dieser Ordner ist noch nicht im Index vorhanden.",
            )
        })
}

#[tauri::command]
pub fn get_entry(state: State<'_, AppState>, path: String) -> CommandResult<Option<Entry>> {
    let path = filesystem::normalize_path(&path).map_err(AppError::from)?;
    state.database.get_entry(&path).map_err(AppError::from)
}

#[tauri::command]
pub fn start_index(
    app: AppHandle,
    state: State<'_, AppState>,
    root_path: String,
) -> CommandResult<String> {
    let root_path = filesystem::normalize_path(&root_path).map_err(AppError::from)?;
    let volume = filesystem::discover_volumes()?
        .into_iter()
        .find(|volume| volume.root_path.eq_ignore_ascii_case(&root_path))
        .ok_or_else(|| AppError::new("volume_unavailable", "Das Laufwerk ist nicht verfügbar."))?;
    Ok(state.indexer.start_scan(app, volume))
}

#[tauri::command]
pub fn cancel_index(state: State<'_, AppState>, scan_id: String) -> bool {
    state.indexer.cancel(&scan_id)
}

#[tauri::command]
pub fn search_entries(
    state: State<'_, AppState>,
    query: String,
    offset: u64,
    limit: u64,
    sort_field: String,
    sort_direction: String,
) -> CommandResult<Page<Entry>> {
    let parsed = SearchQuery::parse(&query)
        .map_err(|error| AppError::new("invalid_query", error.to_string()))?;
    state
        .database
        .search(&parsed, offset, limit, &sort_field, &sort_direction)
        .map_err(AppError::from)
}

#[tauri::command]
pub fn analyze_storage(
    state: State<'_, AppState>,
    path: String,
    limit: u64,
) -> CommandResult<StorageAnalysis> {
    let path = filesystem::normalize_path(&path).map_err(AppError::from)?;
    state
        .database
        .storage_analysis(&path, limit)
        .map_err(AppError::from)
}

#[tauri::command]
pub fn verify_index(
    state: State<'_, AppState>,
    volume_id: i64,
) -> CommandResult<VerificationResult> {
    state.database.verify(volume_id).map_err(AppError::from)
}

#[tauri::command]
pub fn start_watchers(state: State<'_, AppState>) -> CommandResult<()> {
    let roots = state
        .database
        .list_volumes()
        .map_err(AppError::from)?
        .into_iter()
        .filter(|volume| matches!(volume.index_status, crate::models::IndexStatus::Ready))
        .map(|volume| PathBuf::from(volume.root_path))
        .collect();
    state.watcher.start(roots).map_err(AppError::from)
}

#[tauri::command]
pub fn open_path(path: String) -> CommandResult<()> {
    filesystem::open_path(&path).map_err(AppError::from)
}

#[tauri::command]
pub fn reveal_path(path: String) -> CommandResult<()> {
    filesystem::reveal_path(&path).map_err(AppError::from)
}

#[tauri::command]
pub fn path_properties(path: String) -> CommandResult<PathProperties> {
    filesystem::properties(&path).map_err(AppError::from)
}

#[tauri::command]
pub fn create_folder(
    state: State<'_, AppState>,
    parent: String,
    name: String,
) -> CommandResult<Entry> {
    let path = filesystem::create_directory(&parent, &name).map_err(AppError::from)?;
    state.database.upsert_path(&path).map_err(AppError::from)?;
    state
        .database
        .get_entry(&path.to_string_lossy())
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "index_update_failed",
                "Der neue Ordner konnte nicht in den Index übernommen werden.",
            )
        })
}

#[tauri::command]
pub fn rename_entry(
    state: State<'_, AppState>,
    path: String,
    new_name: String,
) -> CommandResult<Entry> {
    let destination = filesystem::rename_path(&path, &new_name).map_err(AppError::from)?;
    state
        .database
        .move_path(&path, &destination.to_string_lossy())
        .map_err(AppError::from)?;
    state
        .database
        .get_entry(&destination.to_string_lossy())
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "index_update_failed",
                "Der umbenannte Eintrag fehlt im Index.",
            )
        })
}

#[tauri::command]
pub fn delete_entries(state: State<'_, AppState>, paths: Vec<String>) -> CommandResult<()> {
    for path in paths {
        filesystem::delete_to_trash(std::slice::from_ref(&path)).map_err(AppError::from)?;
        state.database.remove_path(&path).map_err(AppError::from)?;
    }
    Ok(())
}

#[tauri::command]
pub fn copy_entries(
    state: State<'_, AppState>,
    sources: Vec<String>,
    destination: String,
) -> CommandResult<()> {
    for source in sources {
        let copied = filesystem::copy_paths(std::slice::from_ref(&source), &destination)
            .map_err(AppError::from)?;
        if let Some((_, target)) = copied.into_iter().next() {
            sync_tree(&state.database, &target).map_err(AppError::from)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn move_entries(
    state: State<'_, AppState>,
    sources: Vec<String>,
    destination: String,
) -> CommandResult<()> {
    for source_value in sources {
        let source_entry = state
            .database
            .get_entry(&source_value)
            .map_err(AppError::from)?;
        let moved = filesystem::move_paths(std::slice::from_ref(&source_value), &destination)
            .map_err(AppError::from)?;
        let Some((source, target)) = moved.into_iter().next() else {
            continue;
        };
        let destination_parent = target.parent().and_then(|p| {
            state
                .database
                .get_entry(&p.to_string_lossy())
                .ok()
                .flatten()
        });
        if source_entry
            .as_ref()
            .zip(destination_parent.as_ref())
            .is_some_and(|(a, b)| a.volume_id == b.volume_id)
        {
            state
                .database
                .move_path(&source.to_string_lossy(), &target.to_string_lossy())
                .map_err(AppError::from)?;
        } else {
            state
                .database
                .remove_path(&source.to_string_lossy())
                .map_err(AppError::from)?;
            sync_tree(&state.database, &target).map_err(AppError::from)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn refresh_directory(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let path = PathBuf::from(filesystem::normalize_path(&path).map_err(AppError::from)?);
    let indexed = state
        .database
        .direct_child_paths(&path.to_string_lossy())
        .map_err(AppError::from)?;
    let mut actual = HashSet::new();
    for child in fs::read_dir(&path).map_err(AppError::from)? {
        let child = child.map_err(AppError::from)?;
        actual.insert(child.path().to_string_lossy().to_ascii_lowercase());
        state
            .database
            .upsert_path(&child.path())
            .map_err(AppError::from)?;
    }
    for stale in indexed
        .into_iter()
        .filter(|item| !actual.contains(&item.to_ascii_lowercase()))
    {
        state.database.remove_path(&stale).map_err(AppError::from)?;
    }
    Ok(())
}

fn sync_tree(database: &Database, root: &Path) -> anyhow::Result<()> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        database.upsert_path(&path)?;
        if path.is_dir() {
            for child in fs::read_dir(&path)? {
                stack.push(child?.path());
            }
        }
    }
    Ok(())
}
