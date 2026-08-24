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

use crate::database::Database;

pub trait ChangeProvider: Send {
    fn start(&mut self, roots: &[PathBuf]) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

pub struct FileSystemWatcherProvider {
    database: Arc<Database>,
    watcher: Option<RecommendedWatcher>,
    worker: Option<thread::JoinHandle<()>>,
    stop_tx: Option<mpsc::Sender<()>>,
}

impl FileSystemWatcherProvider {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
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
        let (event_tx, event_rx) = mpsc::sync_channel::<notify::Result<Event>>(16_384);
        let (stop_tx, stop_rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            if event_tx.try_send(event).is_err() {
                log::warn!(target: "watcher", "Watcher queue is full; index may need verification");
            }
        })?;
        for root in roots {
            watcher.watch(root, RecursiveMode::Recursive)?;
            log::info!(target: "watcher", "Watching {}", root.display());
        }
        let database = self.database.clone();
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
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            provider: Arc::new(Mutex::new(FileSystemWatcherProvider::new(database))),
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

    #[test]
    fn usn_provider_is_explicitly_inactive() {
        let mut provider = UsnJournalProvider;
        assert!(provider.start(&[]).is_err());
    }
}
