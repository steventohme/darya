use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc;
use tui_term::vt100;

use crate::config::{ThemeMode, CLAUDE_COMMAND};
use crate::error::{DaryaError, Result};
use crate::event::AppEvent;

/// Callback that detects when Claude Code finishes a task.
/// Claude Code emits OSC 9;4;0 (clear progress indicator) when done,
/// rather than a standalone BEL character.
pub struct PtyCallback {
    pub done_count: Arc<AtomicUsize>,
    pub status_text: Arc<RwLock<String>>,
}

impl PtyCallback {
    pub fn new() -> Self {
        Self {
            done_count: Arc::new(AtomicUsize::new(0)),
            status_text: Arc::new(RwLock::new(String::new())),
        }
    }
}

impl vt100::Callbacks for PtyCallback {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        if let Ok(text) = std::str::from_utf8(title) {
            if let Ok(mut status) = self.status_text.write() {
                *status = text.to_string();
            }
        }
    }

    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        // Standalone BEL — also counts as a notification
        self.done_count.fetch_add(1, Ordering::Relaxed);
    }

    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        match params.first().copied() {
            // OSC 9 — iTerm2-style notifications and progress
            Some(b"9") => {
                // Skip "still working" progress states (9;4;3 indeterminate, 9;4;1 percentage)
                if params.len() >= 3 && params[1] == b"4" {
                    match params[2] {
                        b"3" | b"1" => return,
                        _ => {}
                    }
                }
                // Everything else is attention-worthy:
                // - 9;4;0 = progress done (task finished)
                // - 9;4;2 = progress error
                // - 9;<message> = notification (e.g. permission request)
                self.done_count.fetch_add(1, Ordering::Relaxed);
            }
            // OSC 777 — Ghostty/rxvt-unicode notifications
            // Format: 777;notify;title;body
            Some(b"777") => {
                self.done_count.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

#[allow(dead_code)]
pub struct PtySession {
    pub id: String,
    pub worktree_path: PathBuf,
    pub parser: Arc<RwLock<vt100::Parser<PtyCallback>>>,
    status_text: Arc<RwLock<String>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
}

impl PtySession {
    pub fn spawn(
        worktree_path: PathBuf,
        rows: u16,
        cols: u16,
        theme_mode: ThemeMode,
        command: &str,
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| DaryaError::Pty(format!("failed to open pty: {}", e)))?;

        // Build command from configured string (e.g. "claude --model opus")
        let parts: Vec<&str> = command.split_whitespace().collect();
        let program = parts.first().copied().unwrap_or(CLAUDE_COMMAND);
        let mut cmd = CommandBuilder::new(program);
        for arg in &parts[1..] {
            cmd.arg(arg);
        }
        cmd.cwd(&worktree_path);
        if theme_mode == ThemeMode::Light {
            cmd.env("COLORFGBG", "0;15");
        }

        // Spawn child on the slave
        let slave = pair.slave;
        std::thread::spawn(move || {
            match slave.spawn_command(cmd) {
                Ok(mut child) => {
                    let _ = child.wait();
                }
                Err(e) => {
                    eprintln!("Failed to spawn claude: {}", e);
                }
            }
        });

        // Create vt100 parser with task-done detection callback
        let done_count = Arc::new(AtomicUsize::new(0));
        let status_text = Arc::new(RwLock::new(String::new()));
        let callbacks = PtyCallback {
            done_count: done_count.clone(),
            status_text: status_text.clone(),
        };
        let parser = Arc::new(RwLock::new(vt100::Parser::new_with_callbacks(
            rows, cols, 1000, callbacks,
        )));

        // Reader task: read from PTY → feed to parser → signal event loop
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| DaryaError::Pty(format!("failed to clone reader: {}", e)))?;
        let parser_clone = parser.clone();
        let session_id = id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut last_done_count = 0usize;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut p) = parser_clone.write() {
                            p.process(&buf[..n]);
                        }
                        // Check if Claude Code signaled task completion (OSC 9;4;0)
                        let current = done_count.load(Ordering::Relaxed);
                        if current > last_done_count {
                            last_done_count = current;
                            let _ = event_tx.send(AppEvent::SessionBell {
                                session_id: session_id.clone(),
                            });
                        }
                        // Signal the event loop to redraw
                        let _ = event_tx.send(AppEvent::PtyOutput {
                            session_id: session_id.clone(),
                        });
                    }
                    Err(_) => break,
                }
            }
            let _ = event_tx.send(AppEvent::SessionExited {
                session_id: session_id.clone(),
            });
        });

        // Get writer for sending input
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| DaryaError::Pty(format!("failed to take writer: {}", e)))?;

        Ok(Self {
            id,
            worktree_path,
            parser,
            status_text,
            writer,
            master: pair.master,
        })
    }

    pub fn status_text(&self) -> String {
        self.status_text.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer
            .write_all(bytes)
            .map_err(|e| DaryaError::Pty(format!("write failed: {}", e)))?;
        self.writer
            .flush()
            .map_err(|e| DaryaError::Pty(format!("flush failed: {}", e)))?;
        Ok(())
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        if let Ok(mut p) = self.parser.write() {
            p.screen_mut().set_size(rows, cols);
        }
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| DaryaError::Pty(format!("resize failed: {}", e)))?;
        Ok(())
    }
}
