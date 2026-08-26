use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        mpsc::{self, RecvTimeoutError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, Result};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use crate::database::Database;

pub trait ChangeProvider: Send {
    fn start(&mut self, roots: &[PathBuf]) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

pub struct FileSystemWatcherProvider {
    database: Arc<Database>,
    app: AppHandle,
    watcher: Option<RecommendedWatcher>,
    worker: Option<thread::JoinHandle<()>>,
    stop_tx: Option<mpsc::Sender<()>>,
}

impl FileSystemWatcherProvider {
    pub fn new(database: Arc<Database>, app: AppHandle) -> Self {
        Self {
            database,
            app,
            watcher: None,
            worker: None,
            stop_tx: None,
        }
    }
}

impl ChangeProvider for FileSystemWatcherProvider {
    fn start(&mut self, roots: &[PathBuf]) -> Result<()> {
        self.stop()?;
        if roots.is_empty() {
            return Ok(());
        }
        // A large Steam Workshop cleanup can produce far more than 16k events.
        // Dropping any of them leaves durable ghost entries in the index, so
        // keep accepting events and reconcile them once the burst settles.
        let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
        let (stop_tx, stop_rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            if event_tx.send(event).is_err() {
                log::debug!(target: "watcher", "Watcher worker has stopped");
            }
        })?;
        for root in roots {
            watcher.watch(root, RecursiveMode::Recursive)?;
            log::info!(target: "watcher", "Watching {}", root.display());
        }
        let database = self.database.clone();
        let app = self.app.clone();
        let worker = thread::Builder::new()
            .name("akex-index-watcher".into())
            .spawn(move || {
                let mut pending = Vec::new();
                let mut last_event = Instant::now();
                loop {
                    if stop_rx.try_recv().is_ok() {
                        break;
                    }
                    match event_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(Ok(event)) => {
                            pending.push(event);
                            last_event = Instant::now();
                        }
                        Ok(Err(error)) => log::warn!(target: "watcher", "Watcher error: {error}"),
                        Err(RecvTimeoutError::Timeout)
                            if !pending.is_empty()
                                && last_event.elapsed() >= Duration::from_millis(150) =>
                        {
                            reconcile_batch(&database, std::mem::take(&mut pending));
                            let _ = app.emit("index:changed", ());
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                }
            })?;
        self.watcher = Some(watcher);
        self.worker = Some(worker);
        self.stop_tx = Some(stop_tx);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.watcher.take();
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        Ok(())
    }
}

impl Drop for FileSystemWatcherProvider {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Placeholder for the NTFS USN implementation. The provider boundary and
/// checkpoint column already exist, so it can be introduced without changing
/// commands or UI contracts.
#[allow(dead_code)]
pub struct UsnJournalProvider;

impl ChangeProvider for UsnJournalProvider {
    fn start(&mut self, _roots: &[PathBuf]) -> Result<()> {
        bail!("USN Journal Provider ist für Phase 9 vorbereitet, aber noch nicht aktiviert")
    }
    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct WatcherManager {
    provider: Arc<Mutex<FileSystemWatcherProvider>>,
}

impl WatcherManager {
    pub fn new(database: Arc<Database>, app: AppHandle) -> Self {
        Self {
            provider: Arc::new(Mutex::new(FileSystemWatcherProvider::new(database, app))),
        }
    }
    pub fn start(&self, roots: Vec<PathBuf>) -> Result<()> {
        self.provider.lock().start(&roots)
    }
}

fn reconcile_batch(database: &Database, events: Vec<Event>) {
    let mut touched = HashMap::<PathBuf, EventKind>::new();
    let mut moves = Vec::<(PathBuf, PathBuf)>::new();
    let mut rename_sources = VecDeque::<PathBuf>::new();
    for event in events {
        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        ) && event.paths.len() >= 2
        {
            moves.push((event.paths[0].clone(), event.paths[1].clone()));
            continue;
        }
        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::From))
        ) {
            rename_sources.extend(event.paths);
            continue;
        }
        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::To))
        ) {
            for destination in event.paths {
                if let Some(source) = rename_sources.pop_front() {
                    moves.push((source, destination));
                } else {
                    touched.insert(destination, event.kind);
                }
            }
            continue;
        }
        for path in event.paths {
            touched.insert(path, event.kind);
        }
    }
    for source in rename_sources {
        touched.insert(source, EventKind::Remove(notify::event::RemoveKind::Any));
    }
    for (source, destination) in moves {
        if let Err(error) =
            database.move_path(&source.to_string_lossy(), &destination.to_string_lossy())
        {
            log::warn!(target: "watcher", "Could not apply move {} -> {}: {error:#}", source.display(), destination.display());
        }
    }
    for (path, kind) in touched {
        let result = if path.exists() && !matches!(kind, EventKind::Remove(_)) {
            database.upsert_path(&path)
        } else {
            database.remove_path(&path.to_string_lossy())
        };
        if let Err(error) = result {
            log::debug!(target: "watcher", "Could not reconcile {}: {error:#}", path.display());
            let _ = database.mark_out_of_date(&path.to_string_lossy(), &error.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::RemoveKind;
    use tempfile::tempdir;

    #[test]
    fn usn_provider_is_explicitly_inactive() {
        let mut provider = UsnJournalProvider;
        assert!(provider.start(&[]).is_err());
    }

    #[test]
    fn removed_directory_event_prunes_the_whole_indexed_subtree() {
        let database_dir = tempdir().unwrap();
        let filesystem_root = tempdir().unwrap();
        let child = filesystem_root.path().join("workshop-mod");
        let payload = child.join("payload.bin");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(&payload, vec![0_u8; 128]).unwrap();

        let database = Database::new(database_dir.path().join("index.db"));
        database.initialize().unwrap();
        let root = filesystem_root.path().to_string_lossy().to_string();
        database
            .ensure_volume("test-volume", &root, None, Some("NTFS"), None, None)
            .unwrap();
        database.upsert_path(filesystem_root.path()).unwrap();
        database.upsert_path(&child).unwrap();
        database.upsert_path(&payload).unwrap();

        std::fs::remove_dir_all(&child).unwrap();
        reconcile_batch(
            &database,
            vec![Event::new(EventKind::Remove(RemoveKind::Any)).add_path(child.clone())],
        );

        assert!(database
            .get_entry(&child.to_string_lossy())
            .unwrap()
            .is_none());
        assert_eq!(
            database.get_entry(&root).unwrap().unwrap().recursive_size,
            0
        );
        assert_eq!(database.list_volumes().unwrap()[0].entry_count, 1);
    }
}
