mod app;
mod config;
mod error;
mod event;
mod session;
mod ui;
mod widgets;
mod worktree;

use std::io;
use std::path::PathBuf;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, InputMode, Panel};
use event::{create_event_handler, AppEvent};
use session::manager::SessionManager;
use worktree::manager::WorktreeManager;

fn find_git_root() -> color_eyre::Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err(color_eyre::eyre::eyre!(
            "Not in a git repository. Run darya from within a git repo."
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(path))
}

/// Calculate the terminal area available for the PTY (excluding borders and sidebar).
fn pty_size(terminal: &Terminal<CrosstermBackend<io::Stdout>>) -> (u16, u16) {
    let size = terminal.size().unwrap_or_default();
    let cols = (size.width * 75 / 100).saturating_sub(2).max(10);
    let rows = size.height.saturating_sub(3).max(4);
    (rows, cols)
}

/// Restore the terminal to normal state. Called on both clean exit and panic.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // Install panic hook that restores terminal before printing the panic
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    // Find git root and load worktrees
    let repo_root = find_git_root()?;
    let wt_manager = WorktreeManager::new(repo_root);
    let worktrees = wt_manager.list()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app, event handler, and session manager
    let mut app = App::new(worktrees);
    let (mut events, event_tx) = create_event_handler();
    let mut session_manager = SessionManager::new(event_tx);

    // Main loop
    let result = run_loop(
        &mut terminal,
        &mut app,
        &mut events,
        &mut session_manager,
        &wt_manager,
    )
    .await;

    // Restore terminal (normal exit path)
    restore_terminal();
    terminal.show_cursor()?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    events: &mut event::EventHandler,
    session_manager: &mut SessionManager,
    wt_manager: &WorktreeManager,
) -> color_eyre::Result<()> {
    while app.running {
        terminal.draw(|frame| ui::draw(frame, app, session_manager))?;
        app.status_message = None;

        if let Some(event) = events.next().await {
            if let AppEvent::Key(key) = &event {
                // Handle worktree creation
                if let Some(branch_name) = app.wants_create_worktree(key) {
                    match wt_manager.add(&branch_name, &branch_name) {
                        Ok(()) => {
                            app.prompt = None;
                            if let Ok(worktrees) = wt_manager.list() {
                                app.refresh_worktrees(worktrees);
                            }
                            app.status_message =
                                Some(format!("Created worktree '{}'", branch_name));
                        }
                        Err(e) => {
                            app.prompt = None;
                            app.status_message = Some(format!("Error: {}", e));
                        }
                    }
                }

                // Handle worktree deletion
                if app.wants_delete_worktree(key) {
                    if let Some(wt) = app.worktrees.get(app.selected_worktree).cloned() {
                        // Clean up session if it exists
                        if let Some(session_id) = app.session_ids.remove(&wt.path) {
                            session_manager.remove(&session_id);
                            if app.active_session_id.as_deref() == Some(&session_id) {
                                app.active_session_id = None;
                            }
                        }
                        match wt_manager.remove(&wt.path) {
                            Ok(()) => {
                                app.prompt = None;
                                if let Ok(worktrees) = wt_manager.list() {
                                    app.refresh_worktrees(worktrees);
                                }
                                app.status_message =
                                    Some(format!("Deleted worktree '{}'", wt.name));
                            }
                            Err(e) => {
                                app.prompt = None;
                                app.status_message = Some(format!("Error: {}", e));
                            }
                        }
                    }
                }

                // Handle session spawning on Enter in sidebar
                if app.needs_session_spawn(key) {
                    if let Some(wt_path) = app.selected_worktree_path().cloned() {
                        if !app.session_ids.contains_key(&wt_path) {
                            let (rows, cols) = pty_size(terminal);
                            match session_manager.spawn_session(wt_path.clone(), rows, cols) {
                                Ok(id) => {
                                    app.session_ids.insert(wt_path, id.clone());
                                    app.active_session_id = Some(id);
                                    app.active_panel = Panel::Terminal;
                                    app.input_mode = InputMode::Terminal;
                                }
                                Err(e) => {
                                    app.status_message =
                                        Some(format!("Failed to start session: {}", e));
                                }
                            }
                        } else {
                            // Session already exists, just switch to it
                            if let Some(id) = app.session_ids.get(&wt_path).cloned() {
                                app.active_session_id = Some(id);
                                app.active_panel = Panel::Terminal;
                                app.input_mode = InputMode::Terminal;
                            }
                        }
                    }
                }

                // Forward keys to PTY in terminal mode
                if app.input_mode == InputMode::Terminal && app.prompt.is_none() {
                    // Don't forward Esc — it exits terminal mode
                    if key.code != KeyCode::Esc {
                        if let Some(ref session_id) = app.active_session_id {
                            if let Some(bytes) = key_event_to_bytes(key) {
                                if let Some(session) = session_manager.get_mut(session_id) {
                                    let _ = session.write_input(&bytes);
                                }
                            }
                        }
                    }
                }
            }

            // Handle resize
            if let AppEvent::Resize(w, h) = &event {
                let cols = (w * 75 / 100).saturating_sub(2).max(10);
                let rows = h.saturating_sub(3).max(4);
                session_manager.resize_all(rows, cols);
            }

            app.handle_event(&event);
        }
    }
    Ok(())
}

/// Convert a crossterm KeyEvent to raw bytes for the PTY.
fn key_event_to_bytes(key: &crossterm::event::KeyEvent) -> Option<Vec<u8>> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let bytes = match key.code {
        KeyCode::Char(ch) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let upper = ch.to_ascii_uppercase();
                match upper {
                    'A'..='Z' => vec![upper as u8 - 64],
                    '2' | '@' | ' ' => vec![0],
                    '3' | '[' => vec![27],
                    _ => vec![ch as u8],
                }
            } else {
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![8],
        KeyCode::Tab => vec![9],
        KeyCode::Esc => vec![27],
        KeyCode::Left => vec![27, 91, 68],
        KeyCode::Right => vec![27, 91, 67],
        KeyCode::Up => vec![27, 91, 65],
        KeyCode::Down => vec![27, 91, 66],
        KeyCode::Home => vec![27, 91, 72],
        KeyCode::End => vec![27, 91, 70],
        KeyCode::PageUp => vec![27, 91, 53, 126],
        KeyCode::PageDown => vec![27, 91, 54, 126],
        KeyCode::BackTab => vec![27, 91, 90],
        KeyCode::Delete => vec![27, 91, 51, 126],
        KeyCode::Insert => vec![27, 91, 50, 126],
        _ => return None,
    };
    Some(bytes)
}
