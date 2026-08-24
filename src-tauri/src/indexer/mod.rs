use std::{
    collections::HashMap,
    fs::{self, ReadDir},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::{
    database::{Database, NewEntry},
    filesystem::DiscoveredVolume,
    models::ScanProgress,
};

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

#[derive(Clone)]
pub struct IndexManager {
    database: Arc<Database>,
    cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

struct DirectoryFrame {
    id: i64,
    path: PathBuf,
    children: ReadDir,
    recursive_size: u64,
}

enum TraversalAction {
    Child(Box<std::io::Result<fs::DirEntry>>),
    Complete,
}

impl IndexManager {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_scan(&self, app: AppHandle, volume: DiscoveredVolume) -> String {
        let scan_id = Uuid::new_v4().to_string();
        let cancellation = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .insert(scan_id.clone(), cancellation.clone());
        let manager = self.clone();
        let result_id = scan_id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let result = manager.scan(&app, &result_id, &volume, &cancellation);
            manager.cancellations.lock().remove(&result_id);
            if let Err(error) = result {
                log::error!(target: "indexer", "Scan {} failed: {error:#}", result_id);
                let _ = manager
                    .database
                    .set_volume_error(&volume.root_path, &error.to_string());
                let _ = app.emit(
                    "index:error",
                    serde_json::json!({ "scanId": result_id, "message": error.to_string() }),
                );
            }
        });
        scan_id
    }

    pub fn cancel(&self, scan_id: &str) -> bool {
        if let Some(token) = self.cancellations.lock().get(scan_id) {
            token.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    fn scan(
        &self,
        app: &AppHandle,
        scan_id: &str,
        volume: &DiscoveredVolume,
        cancellation: &AtomicBool,
    ) -> Result<()> {
        let root = Path::new(&volume.root_path);
        let volume_db_id = self.database.ensure_volume(
            &volume.volume_id,
            &volume.root_path,
            volume.label.as_deref(),
            volume.filesystem_type.as_deref(),
            volume.total_bytes,
            volume.free_bytes,
        )?;
        log::info!(target: "indexer", "Starting full scan of {}", volume.root_path);
        let mut writer = self.database.create_index_writer(volume_db_id)?;
        let root_meta = fs::metadata(root)
            .with_context(|| format!("Laufwerk {} ist nicht erreichbar", volume.root_path))?;
        let root_id = writer.insert(new_entry(None, volume_db_id, root, &root_meta, true))?;
        let root_children = fs::read_dir(root)
            .with_context(|| format!("Laufwerk {} kann nicht gelesen werden", volume.root_path))?;
        let mut stack = vec![DirectoryFrame {
            id: root_id,
            path: root.to_path_buf(),
            children: root_children,
            recursive_size: 0,
        }];
        let mut count = 1_u64;
        let mut bytes = 0_u64;
        let mut errors = 0_u64;
        let mut last_progress = Instant::now() - Duration::from_secs(2);
        let used_space = volume
            .total_bytes
            .zip(volume.free_bytes)
            .map(|(total, free)| total.saturating_sub(free))
            .filter(|v| *v > 0);

        while !stack.is_empty() {
            if cancellation.load(Ordering::Relaxed) {
                writer.commit_batch()?;
                self.database
                    .mark_out_of_date(&volume.root_path, "Indexierung abgebrochen")?;
                app.emit("index:cancelled", serde_json::json!({ "scanId": scan_id }))?;
                log::info!(target: "indexer", "Scan {} cancelled after {} entries", scan_id, count);
                return Ok(());
            }
            let action = {
                let frame = stack.last_mut().expect("stack is not empty");
                match frame.children.next() {
                    Some(child) => TraversalAction::Child(Box::new(child)),
                    None => TraversalAction::Complete,
                }
            };
            match action {
                TraversalAction::Child(child) => match *child {
                    Err(error) => {
                        errors += 1;
                        log::warn!(target: "indexer", "Directory entry could not be read: {error}");
                    }
                    Ok(child) => {
                        let path = child.path();
                        let metadata = match fs::symlink_metadata(&path) {
                            Ok(metadata) => metadata,
                            Err(error) => {
                                errors += 1;
                                log::debug!(target: "indexer", "Skipping {}: {error}", path.display());
                                continue;
                            }
                        };
                        let file_type = metadata.file_type();
                        let is_directory = file_type.is_dir() && !file_type.is_symlink();
                        let parent_id = stack.last().map(|f| f.id);
                        let id = match writer.insert(new_entry(
                            parent_id,
                            volume_db_id,
                            &path,
                            &metadata,
                            is_directory,
                        )) {
                            Ok(id) => id,
                            Err(error) => {
                                errors += 1;
                                log::warn!(target: "indexer", "Could not index {}: {error:#}", path.display());
                                continue;
                            }
                        };
                        count += 1;
                        if is_directory {
                            match fs::read_dir(&path) {
                                Ok(children) => stack.push(DirectoryFrame {
                                    id,
                                    path,
                                    children,
                                    recursive_size: 0,
                                }),
                                Err(error) => {
                                    errors += 1;
                                    log::debug!(target: "indexer", "Directory {} is not readable: {error}", path.display());
                                    writer.set_directory_size(id, 0)?;
                                }
                            }
                        } else {
                            let size = metadata.len();
                            bytes = bytes.saturating_add(size);
                            if let Some(parent) = stack.last_mut() {
                                parent.recursive_size = parent.recursive_size.saturating_add(size);
                            }
                        }
                    }
                },
                TraversalAction::Complete => {
                    let completed = stack.pop().expect("stack is not empty");
                    writer.set_directory_size(completed.id, completed.recursive_size)?;
                    if let Some(parent) = stack.last_mut() {
                        parent.recursive_size = parent
                            .recursive_size
                            .saturating_add(completed.recursive_size);
                    }
                }
            }
            if last_progress.elapsed() >= Duration::from_millis(200) {
                let current_path = stack
                    .last()
                    .map(|f| f.path.to_string_lossy().to_string())
                    .unwrap_or_else(|| volume.root_path.clone());
                let percent =
                    used_space.map(|used| ((bytes as f64 / used as f64) * 100.0).min(99.0));
                app.emit(
                    "index:progress",
                    ScanProgress {
                        scan_id: scan_id.to_string(),
                        root_path: volume.root_path.clone(),
                        entries_found: count,
                        bytes_found: bytes,
                        current_path,
                        percent,
                        phase: "Scanning".into(),
                        errors,
                    },
                )?;
                last_progress = Instant::now();
            }
        }
        writer.finish(count)?;
        app.emit(
            "index:complete",
            ScanProgress {
                scan_id: scan_id.to_string(),
                root_path: volume.root_path.clone(),
                entries_found: count,
                bytes_found: bytes,
                current_path: volume.root_path.clone(),
                percent: Some(100.0),
                phase: "Complete".into(),
                errors,
            },
        )?;
        log::info!(target: "indexer", "Completed scan of {}: {} entries, {} bytes, {} errors", volume.root_path, count, bytes, errors);
        Ok(())
    }
}

fn new_entry<'a>(
    parent_id: Option<i64>,
    volume_id: i64,
    path: &'a Path,
    metadata: &fs::Metadata,
    is_directory: bool,
) -> NewEntry<'a> {
    let attributes = file_attributes(metadata);
    NewEntry {
        parent_id,
        volume_id,
        name: path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_str()
            .unwrap_or("<ungültiger Name>"),
        full_path: path.to_str().unwrap_or(""),
        extension: path.extension().and_then(|v| v.to_str()),
        is_directory,
        size: if is_directory { 0 } else { metadata.len() },
        created_at: timestamp(metadata.created().ok()),
        modified_at: timestamp(metadata.modified().ok()),
        file_attributes: attributes,
        hidden: attributes.is_some_and(|value| value & FILE_ATTRIBUTE_HIDDEN != 0)
            || path
                .file_name()
                .is_some_and(|v| v.to_string_lossy().starts_with('.')),
        read_only: metadata.permissions().readonly(),
        system: attributes.is_some_and(|value| value & FILE_ATTRIBUTE_SYSTEM != 0),
    }
}

#[cfg(windows)]
fn file_attributes(metadata: &fs::Metadata) -> Option<u32> {
    use std::os::windows::fs::MetadataExt;
    Some(metadata.file_attributes())
}
#[cfg(not(windows))]
fn file_attributes(_: &fs::Metadata) -> Option<u32> {
    None
}

fn timestamp(value: Option<SystemTime>) -> Option<String> {
    value
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
        .map(|d| d.to_rfc3339())
}
