use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use darya::app::{self, App, InputMode};
use darya::config::{self, KeybindingsConfig};
use darya::event::{self, create_event_handler, AppEvent};
use darya::session::manager::SessionManager;
use darya::ui;
use darya::watcher::FileWatcher;
use darya::worktree::manager::WorktreeManager;

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
    let rect = ui::compute_pty_rect(size.into());
    (rect.height.max(1), rect.width.max(1))
}

/// Compute per-pane PTY sizes for split layout. Returns (session_id, rows, cols) tuples.
fn pane_sizes(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    app: &App,
) -> Vec<(String, u16, u16)> {
    let size = terminal.size().unwrap_or_default();
    if let Some(ref layout) = app.pane_layout {
        if layout.panes.len() > 1 {
            let rects = ui::compute_pane_rects(size.into(), layout.panes.len());
            let block = ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Thick);
            return layout
                .panes
                .iter()
                .enumerate()
                .map(|(i, sid)| {
                    let inner = block.inner(rects[i]);
                    (sid.clone(), inner.height.max(1), inner.width.max(1))
                })
                .collect();
        }
    }
    Vec::new()
}

/// Restore the terminal to normal state. Called on both clean exit and panic.
fn restore_terminal() {
    let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // Load config and sync Claude Code's theme to match
    let app_config = config::load_config();
    let theme = app_config.theme;
    let terminal_start_bottom = app_config.terminal_start_bottom;
    let original_claude_theme = config::sync_claude_theme(theme.mode);
    let claude_theme_for_panic = Arc::new(Mutex::new(original_claude_theme.clone()));

    // Install panic hook that restores terminal and Claude theme before printing the panic
    let panic_theme = Arc::clone(&claude_theme_for_panic);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        if let Ok(val) = panic_theme.lock() {
            config::restore_claude_theme(val.clone());
        }
        default_hook(info);
    }));

    // Find git root and load worktrees
    let repo_root = find_git_root()?;
    let wt_manager = WorktreeManager::new(repo_root, app_config.worktree_dir_format);
    let worktrees = wt_manager.list()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    // Enable keyboard enhancement so Ctrl+number keys are reported correctly
    let _ = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app with loaded theme, keybindings, and session command
    let keybindings = app_config.keybindings;
    let session_command = app_config.session_command;
    let mut app = App::new(worktrees, theme, terminal_start_bottom, keybindings, session_command);
    let (pty_rows, _pty_cols) = pty_size(&terminal);
    app.terminal_height = pty_rows;
    let (mut events, event_tx) = create_event_handler();
    let watcher_tx = event_tx.clone();
    let mut session_manager = SessionManager::new(event_tx.clone());

    // Start file watcher on initial worktree path
    let initial_watch_path = app
        .selected_worktree_path()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));
    let mut file_watcher = FileWatcher::new(initial_watch_path, watcher_tx).ok();

    // Main loop
    let result = run_loop(
        &mut terminal,
        &mut app,
        &mut events,
        &mut session_manager,
        &wt_manager,
        &mut file_watcher,
        &event_tx,
    )
    .await;

    // Restore terminal and Claude theme (normal exit path)
    restore_terminal();
    config::restore_claude_theme(original_claude_theme);
    terminal.show_cursor()?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    events: &mut event::EventHandler,
    session_manager: &mut SessionManager,
    wt_manager: &WorktreeManager,
    file_watcher: &mut Option<FileWatcher>,
    event_tx: &tokio::sync::mpsc::UnboundedSender<AppEvent>,
) -> color_eyre::Result<()> {
    while app.running {
        terminal.draw(|frame| ui::draw(frame, app, session_manager))?;

        if let Some(event) = events.next().await {
            if let AppEvent::Key(key) = &event {
                // Clear status message on any keypress
                app.status_message = None;
                // Ctrl+C: dismiss prompt → close active session → quit
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c')
                {
                    if app.fuzzy_finder.is_some() {
                        app.fuzzy_finder = None;
                    } else if app.prompt.is_some() {
                        app.prompt = None;
                    } else if app.input_mode == InputMode::Editor {
                        // Exit edit mode back to read-only navigation
                        if let Some(ref mut editor) = app.editor {
                            editor.read_only = true;
                            editor.editor_state.mode = edtui::EditorMode::Normal;
                        }
                        app.input_mode = InputMode::Navigation;
                    } else if let Some(session_id) = app.focused_session_id().cloned() {
                        session_manager.remove(&session_id);
                        app.session_ids.retain(|_, v| v != &session_id);
                        app.attention_sessions.remove(&session_id);
                        app.exited_sessions.remove(&session_id);
                        app.activity.remove_session(&session_id);
                        app.remove_session_from_panes(&session_id);
                        if app.active_session_id.as_deref() == Some(&session_id) {
                            app.active_session_id = None;
                        }
                        app.input_mode = InputMode::Navigation;
                        app.status_message = Some("Session closed".to_string());
                    } else {
                        app.running = false;
                    }
                }

                // Fuzzy file finder keybinding
                if KeybindingsConfig::matches(&app.keybindings.fuzzy_finder, key.modifiers, key.code) {
                    if app.fuzzy_finder.is_none() {
                        app.prompt = None; // dismiss any active prompt
                        let root = app.file_explorer.root.clone();
                        app.fuzzy_finder = Some(app::FuzzyFinderState::new(root));
                        app.input_mode = InputMode::Navigation;
                    }
                }

                // Project search keybinding
                if KeybindingsConfig::matches(&app.keybindings.project_search, key.modifiers, key.code) {
                    if app.prompt.is_none() && app.fuzzy_finder.is_none() {
                        app.prompt = Some(app::Prompt::SearchInput {
                            input: String::new(),
                        });
                        app.input_mode = InputMode::Navigation;
                    }
                }

                // Handle worktree creation
                if let Some(branch_name) = app.wants_create_worktree(key) {
                    match wt_manager.add(&branch_name) {
                        Ok(()) => {
                            app.prompt = None;
                            if let Ok(worktrees) = wt_manager.list() {
                                // Select the newly created worktree (last in list)
                                let new_idx = worktrees.len().saturating_sub(1);
                                app.refresh_worktrees(worktrees);
                                app.selected_worktree = new_idx;
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
                else if app.wants_delete_worktree(key) {
                    if let Some(wt) = app.worktrees.get(app.selected_worktree).cloned() {
                        // Clean up session if it exists
                        if let Some(session_id) = app.session_ids.remove(&wt.path) {
                            session_manager.remove(&session_id);
                            app.attention_sessions.remove(&session_id);
                            app.exited_sessions.remove(&session_id);
                            app.activity.remove_session(&session_id);
                            app.remove_session_from_panes(&session_id);
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
                else if app.needs_session_spawn(key) {
                    if let Some(wt_path) = app.selected_worktree_path().cloned() {
                        if !app.session_ids.contains_key(&wt_path) {
                            let (rows, cols) = pty_size(terminal);
                            let command = config::resolve_session_command(&wt_path, &app.session_command);
                            match session_manager.spawn_session(wt_path.clone(), rows, cols, app.theme.mode, &command) {
                                Ok(id) => {
                                    app.session_ids.insert(wt_path, id.clone());
                                    app.active_session_id = Some(id);
                                    app.focus_terminal_panel();
                                    app.input_mode = InputMode::Terminal;
                                    if command != config::CLAUDE_COMMAND {
                                        app.status_message = Some(format!("Started session ({})", command));
                                    }
                                }
                                Err(e) => {
                                    app.status_message =
                                        Some(format!("Failed to start session: {}", e));
                                }
                            }
                        } else {
                            // Session already exists, just switch to it
                            if let Some(id) = app.session_ids.get(&wt_path).cloned() {
                                app.attention_sessions.remove(&id);
                                app.active_session_id = Some(id);
                                app.focus_terminal_panel();
                                app.input_mode = InputMode::Terminal;
                            }
                        }
                    }
                }

                // Handle session restart on 'r' for exited sessions
                else if app.needs_session_restart(key) {
                    if let Some(wt_path) = app.selected_worktree_path().cloned() {
                        if let Some(old_id) = app.session_ids.remove(&wt_path) {
                            session_manager.remove(&old_id);
                            app.attention_sessions.remove(&old_id);
                            app.exited_sessions.remove(&old_id);
                            app.activity.remove_session(&old_id);
                            if app.active_session_id.as_deref() == Some(&old_id) {
                                app.active_session_id = None;
                            }
                        }
                        let (rows, cols) = pty_size(terminal);
                        let command = config::resolve_session_command(&wt_path, &app.session_command);
                        match session_manager.spawn_session(
                            wt_path.clone(),
                            rows,
                            cols,
                            app.theme.mode,
                            &command,
                        ) {
                            Ok(id) => {
                                app.session_ids.insert(wt_path, id.clone());
                                app.active_session_id = Some(id);
                                app.focus_terminal_panel();
                                app.input_mode = InputMode::Terminal;
                                if command != config::CLAUDE_COMMAND {
                                    app.status_message = Some(format!("Started session ({})", command));
                                }
                            }
                            Err(e) => {
                                app.status_message =
                                    Some(format!("Failed to restart session: {}", e));
                            }
                        }
                    }
                }

                // Handle session close on Backspace
                else if app.needs_session_close(key) {
                    if let Some(wt_path) = app.selected_worktree_path().cloned() {
                        if let Some(session_id) = app.session_ids.remove(&wt_path) {
                            session_manager.remove(&session_id);
                            app.attention_sessions.remove(&session_id);
                            app.exited_sessions.remove(&session_id);
                            app.activity.remove_session(&session_id);
                            app.remove_session_from_panes(&session_id);
                            if app.active_session_id.as_deref() == Some(&session_id) {
                                app.active_session_id = None;
                            }
                            app.status_message = Some("Session closed".to_string());
                        }
                    }
                }

                // Split pane (Navigation mode only)
                if app.input_mode == InputMode::Navigation
                    && KeybindingsConfig::matches(&app.keybindings.split_pane, key.modifiers, key.code)
                {
                    if app.split_add_pane() {
                        // Resize sessions to new pane dimensions
                        for (sid, rows, cols) in pane_sizes(terminal, app) {
                            if let Some(session) = session_manager.get_mut(&sid) {
                                let _ = session.resize(rows, cols);
                            }
                        }
                    }
                }

                // Close pane — intercept in ANY mode when panes exist
                // (prevents Ctrl+W from reaching PTY as delete-word)
                if app.pane_layout.is_some()
                    && KeybindingsConfig::matches(&app.keybindings.close_pane, key.modifiers, key.code)
                {
                    app.input_mode = InputMode::Navigation;
                    app.close_focused_pane();
                    // Resize sessions to new pane dimensions (or single pane)
                    let sizes = pane_sizes(terminal, app);
                    if sizes.is_empty() {
                        // Back to single pane — resize all to full
                        let (rows, cols) = pty_size(terminal);
                        session_manager.resize_all(rows, cols);
                    } else {
                        for (sid, rows, cols) in sizes {
                            if let Some(session) = session_manager.get_mut(&sid) {
                                let _ = session.resize(rows, cols);
                            }
                        }
                    }
                }

                // Forward keys to PTY in terminal mode
                if app.input_mode == InputMode::Terminal && app.prompt.is_none() {
                    // Don't forward Tab — it switches to sidebar
                    if key.code != KeyCode::Tab {
                        if let Some(session_id) = app.focused_session_id().cloned() {
                            if !app.exited_sessions.contains(session_id.as_str()) {
                                if let Some(bytes) = key_event_to_bytes(key) {
                                    if let Some(session) =
                                        session_manager.get_mut(&session_id)
                                    {
                                        let _ = session.write_input(&bytes);
                                        app.activity.mark_input(&session_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Reset scroll when new output arrives for a visible session
            if let AppEvent::PtyOutput { ref session_id } = event {
                if app.is_session_visible(session_id) {
                    app.scroll_offsets.remove(session_id);
                }
            }

            // Handle resize
            if let AppEvent::Resize(w, h) = &event {
                let full_size = Rect::new(0, 0, *w, *h);
                let rect = ui::compute_pty_rect(full_size);
                app.terminal_height = rect.height.max(1);

                if let Some(ref layout) = app.pane_layout {
                    if layout.panes.len() > 1 {
                        let pane_rects = ui::compute_pane_rects(full_size, layout.panes.len());
                        let block = ratatui::widgets::Block::default()
                            .borders(ratatui::widgets::Borders::ALL)
                            .border_type(ratatui::widgets::BorderType::Thick);
                        for (i, sid) in layout.panes.iter().enumerate() {
                            let inner = block.inner(pane_rects[i]);
                            if let Some(session) = session_manager.get_mut(sid) {
                                let _ = session.resize(inner.height.max(1), inner.width.max(1));
                            }
                        }
                        // Resize non-visible sessions to single-pane size
                        session_manager.resize_all_except(
                            &layout.panes,
                            rect.height.max(1),
                            rect.width.max(1),
                        );
                    } else {
                        session_manager.resize_all(rect.height.max(1), rect.width.max(1));
                    }
                } else {
                    session_manager.resize_all(rect.height.max(1), rect.width.max(1));
                }
            }

            app.handle_event(&event);

            // Rewatch if the file explorer root changed (worktree switch)
            let current_root = &app.file_explorer.root;
            let needs_rewatch = match file_watcher {
                Some(ref fw) => fw.watched_path() != current_root,
                None => true,
            };
            if needs_rewatch {
                let new_path = current_root.clone();
                *file_watcher = match file_watcher.take() {
                    Some(fw) => fw.rewatch(new_path, event_tx.clone()).ok(),
                    None => FileWatcher::new(new_path, event_tx.clone()).ok(),
                };
            }
        }
    }
    Ok(())
}

/// Convert a crossterm KeyEvent to raw bytes for the PTY.
fn key_event_to_bytes(key: &crossterm::event::KeyEvent) -> Option<Vec<u8>> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let has_alt = key.modifiers.contains(KeyModifiers::ALT);
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let has_super = key.modifiers.contains(KeyModifiers::SUPER);

    // CSI modifier parameter: 1=none, 2=Shift, 3=Alt, 4=Shift+Alt,
    // 5=Ctrl, 6=Ctrl+Shift, 7=Ctrl+Alt, 8=Ctrl+Shift+Alt
    // Super (Cmd) maps the same way as Ctrl for terminal purposes.
    let modifier_param = 1
        + if key.modifiers.contains(KeyModifiers::SHIFT) { 1 } else { 0 }
        + if has_alt { 2 } else { 0 }
        + if has_ctrl || has_super { 4 } else { 0 };
    let has_modifier = modifier_param > 1;

    let bytes = match key.code {
        KeyCode::Char(ch) => {
            if has_ctrl {
                let upper = ch.to_ascii_uppercase();
                match upper {
                    'A'..='Z' => vec![upper as u8 - 64],
                    '2' | '@' | ' ' => vec![0],
                    '3' | '[' => vec![27],
                    _ => vec![ch as u8],
                }
            } else if has_alt {
                // Alt+char sends ESC prefix + char
                let mut buf = vec![27u8];
                let mut char_buf = [0u8; 4];
                let s = ch.encode_utf8(&mut char_buf);
                buf.extend_from_slice(s.as_bytes());
                buf
            } else {
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => {
            if has_alt {
                vec![27, 127] // Alt+Backspace: ESC + DEL (word delete)
            } else {
                vec![8]
            }
        }
        KeyCode::Tab => vec![9],
        KeyCode::Esc => vec![27],
        KeyCode::Left if has_modifier => format!("\x1b[1;{}D", modifier_param).into_bytes(),
        KeyCode::Right if has_modifier => format!("\x1b[1;{}C", modifier_param).into_bytes(),
        KeyCode::Up if has_modifier => format!("\x1b[1;{}A", modifier_param).into_bytes(),
        KeyCode::Down if has_modifier => format!("\x1b[1;{}B", modifier_param).into_bytes(),
        KeyCode::Home if has_modifier => format!("\x1b[1;{}H", modifier_param).into_bytes(),
        KeyCode::End if has_modifier => format!("\x1b[1;{}F", modifier_param).into_bytes(),
        KeyCode::Delete if has_modifier => format!("\x1b[3;{}~", modifier_param).into_bytes(),
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
