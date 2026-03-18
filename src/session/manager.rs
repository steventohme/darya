use std::collections::HashMap;
use std::path::PathBuf;

use tokio::sync::mpsc;

use super::pty_session::PtySession;
use crate::config::ThemeMode;
use crate::error::Result;
use crate::event::AppEvent;

pub struct SessionManager {
    sessions: HashMap<String, PtySession>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
}

impl SessionManager {
    pub fn new(event_tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        Self {
            sessions: HashMap::new(),
            event_tx,
        }
    }

    /// Spawn a new session for the given worktree path.
    /// Returns the session ID.
    pub fn spawn_session(
        &mut self,
        worktree_path: PathBuf,
        rows: u16,
        cols: u16,
        theme_mode: ThemeMode,
        command: &str,
    ) -> Result<String> {
        let session = PtySession::spawn(
            worktree_path,
            rows,
            cols,
            theme_mode,
            command,
            self.event_tx.clone(),
        )?;
        let id = session.id.clone();
        self.sessions.insert(id.clone(), session);
        Ok(id)
    }

    pub fn get(&self, id: &str) -> Option<&PtySession> {
        self.sessions.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut PtySession> {
        self.sessions.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<PtySession> {
        self.sessions.remove(id)
    }

    pub fn session_status(&self, id: &str) -> Option<String> {
        self.sessions.get(id).map(|s| s.status_text())
    }

    pub fn resize_all(&mut self, rows: u16, cols: u16) {
        for session in self.sessions.values_mut() {
            let _ = session.resize(rows, cols);
        }
    }

    /// Resize all sessions except those in the exclusion list.
    pub fn resize_all_except(&mut self, exclude: &[String], rows: u16, cols: u16) {
        for (id, session) in self.sessions.iter_mut() {
            if !exclude.contains(id) {
                let _ = session.resize(rows, cols);
            }
        }
    }
}
