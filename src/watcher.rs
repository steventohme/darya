use std::path::PathBuf;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::event::AppEvent;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    watched_path: PathBuf,
}

impl FileWatcher {
    /// Create a file watcher on `path` (recursive). Sends debounced `AppEvent`s to `app_tx`.
    pub fn new(
        path: PathBuf,
        app_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Result<Self, notify::Error> {
        let (raw_tx, raw_rx) = mpsc::unbounded_channel::<Event>();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = raw_tx.send(event);
            }
        })?;

        watcher.watch(&path, RecursiveMode::Recursive)?;

        // Spawn debounce task
        tokio::spawn(debounce_task(raw_rx, app_tx));

        Ok(Self {
            _watcher: watcher,
            watched_path: path,
        })
    }

    /// Drop the old watcher and create a new one for a different directory.
    pub fn rewatch(
        self,
        new_path: PathBuf,
        app_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Result<Self, notify::Error> {
        drop(self);
        Self::new(new_path, app_tx)
    }

    /// The directory currently being watched.
    pub fn watched_path(&self) -> &PathBuf {
        &self.watched_path
    }
}

/// Debounce task: collects raw notify events, waits 200ms of quiet, then batches into AppEvents.
async fn debounce_task(
    mut raw_rx: mpsc::UnboundedReceiver<Event>,
    app_tx: mpsc::UnboundedSender<AppEvent>,
) {
    loop {
        // Wait for the first event
        let Some(first) = raw_rx.recv().await else {
            return; // channel closed
        };

        let mut modified_paths: Vec<PathBuf> = Vec::new();
        let mut has_create_or_delete = false;

        classify_event(&first, &mut modified_paths, &mut has_create_or_delete);

        // Drain additional events arriving within 200ms of quiet
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(200), raw_rx.recv()).await {
                Ok(Some(event)) => {
                    classify_event(&event, &mut modified_paths, &mut has_create_or_delete);
                }
                Ok(None) => return, // channel closed
                Err(_) => break,    // 200ms of quiet — flush batch
            }
        }

        // Deduplicate modified paths
        modified_paths.sort();
        modified_paths.dedup();

        // Send batched events
        if !modified_paths.is_empty() {
            let _ = app_tx.send(AppEvent::FileChanged {
                paths: modified_paths,
            });
        }
        if has_create_or_delete {
            let _ = app_tx.send(AppEvent::FilesCreatedOrDeleted);
        }
    }
}

fn classify_event(event: &Event, modified: &mut Vec<PathBuf>, created_or_deleted: &mut bool) {
    match event.kind {
        EventKind::Modify(_) => {
            modified.extend(event.paths.iter().cloned());
        }
        EventKind::Create(_) | EventKind::Remove(_) => {
            *created_or_deleted = true;
            // Created files are also "changed" so the editor can reload if open
            modified.extend(event.paths.iter().cloned());
        }
        _ => {}
    }
}
