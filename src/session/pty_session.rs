use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc;
use tui_term::vt100;

use crate::config::{ThemeMode, CLAUDE_COMMAND};
use crate::error::{DaryaError, Result};
use crate::event::AppEvent;

#[allow(dead_code)]
pub struct PtySession {
    pub id: String,
    pub worktree_path: PathBuf,
    pub parser: Arc<RwLock<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
}

impl PtySession {
    pub fn spawn(
        worktree_path: PathBuf,
        rows: u16,
        cols: u16,
        theme_mode: ThemeMode,
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

        // Build command: claude (in the worktree directory)
        let mut cmd = CommandBuilder::new(CLAUDE_COMMAND);
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

        // Create vt100 parser
        let parser = Arc::new(RwLock::new(vt100::Parser::new(rows, cols, 1000)));

        // Reader task: read from PTY → feed to parser → signal event loop
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| DaryaError::Pty(format!("failed to clone reader: {}", e)))?;
        let parser_clone = parser.clone();
        let session_id = id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut p) = parser_clone.write() {
                            p.process(&buf[..n]);
                        }
                        // Signal the event loop to redraw
                        let _ = event_tx.send(AppEvent::PtyOutput {
                            session_id: session_id.clone(),
                        });
                    }
                    Err(_) => break,
                }
            }
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
            writer,
            master: pair.master,
        })
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
