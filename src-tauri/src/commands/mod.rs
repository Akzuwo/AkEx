use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    database::Database,
    error::AppError,
    filesystem::{self, PathProperties},
    indexer::IndexManager,
    models::{Entry, FilePreview, Page, StorageAnalysis, VerificationResult, Volume},
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
    let mut page = state
        .database
        .list_directory(&path, offset, limit, &sort_field, &sort_direction)
        .map_err(|error| {
            log::debug!(target: "database", "Directory query failed for {path}: {error:#}");
            AppError::new(
                "not_indexed",
                "Dieser Ordner ist noch nicht im Index vorhanden.",
            )
        })?;
    if remove_missing_entries(&state.database, &page.items).map_err(AppError::from)? {
        page = state
            .database
            .list_directory(&path, offset, limit, &sort_field, &sort_direction)
            .map_err(AppError::from)?;
    }
    Ok(page)
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
    let notify_on_complete = state
        .database
        .list_volumes()
        .map_err(AppError::from)?
        .into_iter()
        .find(|item| item.root_path.eq_ignore_ascii_case(&root_path))
        .map_or(true, |item| item.last_full_scan.is_none());
    Ok(state.indexer.start_scan(app, volume, notify_on_complete))
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
    // A native Windows watcher may lose notifications during very large bulk
    // operations. Validating the largest indexed entries makes storage
    // analysis self-healing for exactly those missed deletions that would
    // otherwise leave the displayed disk usage badly wrong.
    let candidates = state
        .database
        .largest_entries_for_reconciliation(&path, 2_000)
        .map_err(AppError::from)?;
    remove_missing_entries(&state.database, &candidates).map_err(AppError::from)?;
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
pub async fn open_window(app: AppHandle, path: String) -> CommandResult<()> {
    let path = filesystem::normalize_path(&path).map_err(AppError::from)?;
    let label = format!("akex-{}", Uuid::new_v4());
    let url =
        tauri::WebviewUrl::App(format!("index.html?path={}", encode_query_component(&path)).into());
    tauri::WebviewWindowBuilder::new(&app, label, url)
        .title(format!("Akex – {path}"))
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 640.0)
        .build()
        .map_err(|error| AppError::new("window_error", error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn validate_drag_paths(paths: Vec<String>) -> CommandResult<()> {
    if paths.is_empty() {
        return Err(AppError::new(
            "empty_drag",
            "Es wurden keine Einträge ausgewählt.",
        ));
    }
    for path in paths {
        let path = filesystem::normalize_path(&path).map_err(AppError::from)?;
        filesystem::validate_drag_path(&path).map_err(AppError::from)?;
    }
    Ok(())
}

#[tauri::command]
pub fn path_properties(path: String) -> CommandResult<PathProperties> {
    filesystem::properties(&path).map_err(AppError::from)
}

#[tauri::command]
pub fn preview_file(path: String) -> CommandResult<FilePreview> {
    const TEXT_LIMIT: u64 = 2 * 1024 * 1024;
    const MEDIA_LIMIT: u64 = 32 * 1024 * 1024;
    let path = PathBuf::from(filesystem::normalize_path(&path).map_err(AppError::from)?);
    let metadata = fs::metadata(&path).map_err(AppError::from)?;
    if !metadata.is_file() {
        return Ok(unavailable_preview(
            "Für Ordner ist keine Dateivorschau verfügbar.",
        ));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let (kind, mime_type) = match extension.as_str() {
        "jpg" | "jpeg" => ("image", "image/jpeg"),
        "png" => ("image", "image/png"),
        "gif" => ("image", "image/gif"),
        "webp" => ("image", "image/webp"),
        "bmp" => ("image", "image/bmp"),
        "svg" => ("image", "image/svg+xml"),
        "pdf" => ("pdf", "application/pdf"),
        "mp3" => ("audio", "audio/mpeg"),
        "wav" => ("audio", "audio/wav"),
        "ogg" | "oga" => ("audio", "audio/ogg"),
        "m4a" => ("audio", "audio/mp4"),
        "flac" => ("audio", "audio/flac"),
        "mp4" | "m4v" => ("video", "video/mp4"),
        "webm" => ("video", "video/webm"),
        "ogv" => ("video", "video/ogg"),
        "mov" => ("video", "video/quicktime"),
        "txt" | "md" | "json" | "jsonl" | "yaml" | "yml" | "toml" | "xml" | "csv" | "log"
        | "ini" | "conf" | "cfg" | "rs" | "ts" | "tsx" | "js" | "jsx" | "css" | "scss" | "html"
        | "htm" | "py" | "ps1" | "bat" | "cmd" | "sh" | "sql" | "java" | "kt" | "go" | "c"
        | "h" | "cpp" | "hpp" | "cs" | "swift" => ("text", "text/plain"),
        _ => {
            return Ok(unavailable_preview(
                "Für diesen Dateityp ist keine Vorschau verfügbar.",
            ))
        }
    };

    let limit = if kind == "text" {
        TEXT_LIMIT
    } else {
        MEDIA_LIMIT
    };
    if metadata.len() > limit {
        return Ok(unavailable_preview(if kind == "text" {
            "Die Textdatei ist für die Vorschau zu groß (maximal 2 MB)."
        } else {
            "Die Datei ist für die Vorschau zu groß (maximal 32 MB)."
        }));
    }
    let bytes = fs::read(path).map_err(AppError::from)?;
    if kind == "text" {
        return Ok(FilePreview {
            kind: kind.into(),
            mime_type: Some(mime_type.into()),
            data: None,
            text: Some(String::from_utf8_lossy(&bytes).into_owned()),
            message: None,
        });
    }
    Ok(FilePreview {
        kind: kind.into(),
        mime_type: Some(mime_type.into()),
        data: Some(STANDARD.encode(bytes)),
        text: None,
        message: None,
    })
}

fn unavailable_preview(message: &str) -> FilePreview {
    FilePreview {
        kind: "unavailable".into(),
        mime_type: None,
        data: None,
        text: None,
        message: Some(message.into()),
    }
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
    let paths = normalize_delete_paths(paths).map_err(AppError::from)?;
    if paths.is_empty() {
        return Ok(());
    }
    let volume_roots = state
        .database
        .list_volumes()
        .map_err(AppError::from)?
        .into_iter()
        .map(|volume| volume.root_path.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    if paths
        .iter()
        .any(|path| volume_roots.contains(&path.to_ascii_lowercase()))
    {
        return Err(AppError::new(
            "protected_path",
            "Ein Laufwerk kann nicht in den Papierkorb verschoben werden.",
        ));
    }

    let trash_result = filesystem::delete_to_trash(&paths);
    let trash_succeeded = trash_result.is_ok();
    let mut index_error = None;
    for path in &paths {
        let was_removed = trash_succeeded
            || matches!(
                fs::symlink_metadata(path),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound
            );
        if was_removed {
            if let Err(error) = state.database.remove_path(path) {
                index_error.get_or_insert(error);
            }
        }
    }
    trash_result.map_err(AppError::from)?;
    if let Some(error) = index_error {
        return Err(AppError::from(error));
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

fn remove_missing_entries(database: &Database, entries: &[Entry]) -> anyhow::Result<bool> {
    let mut missing_roots = Vec::<String>::new();
    for entry in entries {
        if missing_roots
            .iter()
            .any(|parent| is_same_or_descendant(&entry.full_path, parent))
        {
            continue;
        }
        match fs::symlink_metadata(&entry.full_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                database.remove_path(&entry.full_path)?;
                missing_roots.push(entry.full_path.clone());
            }
            Err(_) => {
                // Access errors must not be mistaken for deletions. The index
                // remains useful for protected folders which cannot currently
                // be inspected by the process.
            }
        }
    }
    Ok(!missing_roots.is_empty())
}

fn is_same_or_descendant(path: &str, parent: &str) -> bool {
    let path = path.trim_end_matches('\\');
    let parent = parent.trim_end_matches('\\');
    path.eq_ignore_ascii_case(parent)
        || path
            .get(..parent.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(parent))
            && path.as_bytes().get(parent.len()) == Some(&b'\\')
}

fn encode_query_component(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (*byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn normalize_delete_paths(paths: Vec<String>) -> anyhow::Result<Vec<String>> {
    let mut normalized = paths
        .into_iter()
        .map(|path| filesystem::normalize_path(&path))
        .collect::<anyhow::Result<Vec<_>>>()?;
    normalized.sort_by_key(|path| path.len());
    let mut result = Vec::<String>::new();
    for path in normalized {
        let lower = path.to_ascii_lowercase();
        let nested = result.iter().any(|parent| {
            let parent = parent.trim_end_matches('\\').to_ascii_lowercase();
            lower.trim_end_matches('\\') == parent || lower.starts_with(&format!("{parent}\\"))
        });
        if !nested {
            result.push(path);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{encode_query_component, is_same_or_descendant, normalize_delete_paths};

    #[test]
    fn window_path_is_encoded_as_a_query_component() {
        assert_eq!(
            encode_query_component("C:\\Meine Dateien\\Bilder"),
            "C%3A%5CMeine%20Dateien%5CBilder"
        );
        assert_eq!(encode_query_component("D:\\Grüsse"), "D%3A%5CGr%C3%BCsse");
    }

    #[test]
    fn delete_paths_are_deduplicated_and_nested_entries_are_removed() {
        let paths = normalize_delete_paths(vec![
            "C:\\Daten\\Unterordner\\datei.txt".into(),
            "c:\\daten".into(),
            "C:\\Andere Datei.txt".into(),
            "C:\\DATEN".into(),
        ])
        .unwrap();
        assert_eq!(paths, vec!["C:\\daten", "C:\\Andere Datei.txt"]);
    }

    #[test]
    fn descendant_check_requires_a_path_separator() {
        assert!(is_same_or_descendant(
            "C:\\Steam\\workshop\\123\\file.bin",
            "c:\\steam\\workshop\\123"
        ));
        assert!(is_same_or_descendant(
            "C:\\Steam\\workshop\\123",
            "c:\\steam\\workshop\\123\\"
        ));
        assert!(!is_same_or_descendant(
            "C:\\Steam\\workshop\\1234",
            "C:\\Steam\\workshop\\123"
        ));
    }
}
