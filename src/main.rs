use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, KeyCode,
    KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use darya::app::{self, App, InputMode};
use darya::config::{self, KeybindingsConfig, CLAUDE_COMMAND};
use darya::event::{self, create_event_handler, AppEvent};
use darya::session::manager::SessionManager;
use darya::sidebar::types::SessionKind;
use darya::ui;
use darya::watcher::FileWatcher;
use darya::worktree::manager::WorktreeManager;
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGTERM};
use signal_hook_tokio::Signals;

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
fn pty_size(terminal: &Terminal<CrosstermBackend<io::Stdout>>, sidebar_pct: u16) -> (u16, u16) {
    let size = terminal.size().unwrap_or_default();
    let rect = ui::compute_pty_rect(size.into(), sidebar_pct);
    (rect.height.max(1), rect.width.max(1))
}

/// Compute per-pane PTY sizes for split layout. Returns (session_id, rows, cols) tuples.
/// Only includes Terminal/Shell panes (they have PTY sessions).
fn pane_sizes(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    app: &App,
) -> Vec<(String, u16, u16)> {
    let size = terminal.size().unwrap_or_default();
    if let Some(ref layout) = app.pane_layout {
        if layout.panes.len() > 1 {
            let rects = ui::compute_pane_rects(
                size.into(),
                layout.panes.len(),
                app.sidebar_width,
                layout.direction,
            );
            let block = ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Thick);
            return layout
                .panes
                .iter()
                .enumerate()
                .filter_map(|(i, content)| {
                    content.session_id().map(|sid| {
                        let inner = block.inner(rects[i]);
                        (sid.to_string(), inner.height.max(1), inner.width.max(1))
                    })
                })
                .collect();
        }
    }
    Vec::new()
}

/// Restore the terminal to normal state. Called on both clean exit and panic.
fn restore_terminal() {
    let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
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
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
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
    let shell_command = app_config.shell_command;
    let auto_resume = app_config.auto_resume;
    let mut app = App::new(
        worktrees,
        theme,
        terminal_start_bottom,
        keybindings,
        session_command,
        shell_command,
    );

    // Load planet if configured
    if let Some(planet_kind) = app_config.planet {
        app.planet_kind = Some(planet_kind);
        app.planet_animation = Some(darya::planet::sprites::PlanetAnimation::load(planet_kind));
    }
    app.show_planet = app_config.show_planet;

    // Show theme picker on first launch (before setup guide), or just setup guide
    if !config::setup_done() {
        let selected = app
            .planet_kind
            .and_then(|k| {
                darya::planet::types::PlanetKind::all()
                    .iter()
                    .position(|p| *p == k)
            })
            .unwrap_or(0);
        let planet = darya::planet::types::PlanetKind::all()[selected];
        app.planet_animation = Some(darya::planet::sprites::PlanetAnimation::load(planet));
        app.planet_tick = 0;
        app.prompt = Some(darya::app::Prompt::ThemePicker {
            selected,
            previous_theme: app.theme.clone(),
        });
    }

    // Load sections config if it exists, merge with discovered worktrees
    if let Some(sections_config) = config::load_sections() {
        let wt_list = wt_manager.list().unwrap_or_default();
        app.sidebar_tree =
            darya::sidebar::tree::SidebarTree::from_config(&sections_config, &wt_list);
    }
    // Discover worktrees for sections with root_path
    for si in 0..app.sidebar_tree.sections.len() {
        if let Some(root) = app.sidebar_tree.sections[si].root_path.clone() {
            if let Ok(wts) = darya::worktree::manager::list_worktrees_for_root(&root) {
                app.sidebar_tree.refresh_section_worktrees(si, &wts);
            }
        }
    }
    // Load saved layout and decide whether to restore sessions
    if let Some(layout) = config::load_layout() {
        if !layout.sessions.is_empty() {
            if auto_resume {
                app.pending_layout = Some(layout);
                app.restore_approved = true;
            } else if app.prompt.is_none() {
                // Only show prompt if no other prompt (e.g. SetupGuide) is active
                let count = layout.sessions.len();
                app.pending_layout = Some(layout);
                app.prompt = Some(darya::app::Prompt::RestoreSession { count });
            }
        }
    }

    let (pty_rows, _pty_cols) = pty_size(&terminal, app.sidebar_width);
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

    // Register signal handlers for graceful shutdown (e.g. cargo-watch sends SIGTERM)
    let mut signals = Signals::new([SIGTERM, SIGINT, SIGHUP])?;

    // Main loop
    let result = run_loop(
        &mut terminal,
        &mut app,
        &mut events,
        &mut session_manager,
        &wt_manager,
        &mut file_watcher,
        &event_tx,
        &mut signals,
    )
    .await;

    // Save sections config and layout before exit
    config::save_sections(&app.sidebar_tree.to_sections_config());
    config::save_layout(&app.to_layout_config());

    // Restore terminal and Claude theme (normal exit path)
    restore_terminal();
    config::restore_claude_theme(original_claude_theme);
    terminal.show_cursor()?;

    result
}

#[allow(clippy::too_many_arguments)]
async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    events: &mut event::EventHandler,
    session_manager: &mut SessionManager,
    wt_manager: &WorktreeManager,
    file_watcher: &mut Option<FileWatcher>,
    event_tx: &tokio::sync::mpsc::UnboundedSender<AppEvent>,
    signals: &mut Signals,
) -> color_eyre::Result<()> {
    use futures::StreamExt as _;
    use std::collections::HashMap;
    use std::time::Instant;

    // Debounce native notifications: only fire if no PtyOutput for that session within timeout
    let mut pending_native: HashMap<String, (Instant, String)> = HashMap::new();
    // Debounce attention indicator: only mark attention if no PtyOutput follows within timeout.
    // This prevents every tool call completion from turning the sidebar green.
    let mut pending_attention: HashMap<String, Instant> = HashMap::new();
    const DEBOUNCE_SECS: f64 = 3.0;

    while app.running {
        terminal.draw(|frame| ui::draw(frame, app, session_manager))?;

        // Wait for the first event or a termination signal
        let event = tokio::select! {
            ev = events.next() => {
                let Some(ev) = ev else { break };
                ev
            }
            _ = signals.next() => {
                app.running = false;
                break;
            }
        };
        // Process the first event, then drain all pending events before redrawing.
        // This batches rapid keystrokes and PtyOutput events into a single redraw.
        process_event(
            &event,
            terminal,
            app,
            session_manager,
            wt_manager,
            &mut pending_native,
            &mut pending_attention,
        );

        // Drain remaining queued events without blocking
        while let Ok(event) = events.try_recv() {
            process_event(
                &event,
                terminal,
                app,
                session_manager,
                wt_manager,
                &mut pending_native,
                &mut pending_attention,
            );
            if !app.running {
                break;
            }
        }

        // Post-event housekeeping (once per batch, not per event)

        // Flush debounced native notifications that have aged past the threshold
        pending_native.retain(|_sid, (queued_at, msg)| {
            if queued_at.elapsed().as_secs_f64() >= DEBOUNCE_SECS {
                send_native_notification(msg.clone());
                false // remove from map
            } else {
                true // keep waiting
            }
        });

        // Flush debounced attention indicators — mark sessions that have been
        // idle (no PtyOutput) for the debounce period
        pending_attention.retain(|sid, queued_at| {
            if queued_at.elapsed().as_secs_f64() >= DEBOUNCE_SECS {
                app.attention_sessions.insert(sid.clone());
                false
            } else {
                true
            }
        });

        // Handle pending section refresh (after dir browser creates a section)
        if let Some((section_idx, root_path)) = app.pending_section_refresh.take() {
            if let Ok(wts) = darya::worktree::manager::list_worktrees_for_root(&root_path) {
                app.sidebar_tree
                    .refresh_section_worktrees(section_idx, &wts);
            }
        }

        // Deferred session restore: user approved (or auto_resume) + layout pending
        if app.restore_approved {
            if let Some(layout) = app.pending_layout.take() {
                app.restore_approved = false;
                restore_sessions(terminal, app, session_manager, &layout);
            }
        }

        // Handle sidebar resize — trigger PTY resize for all sessions
        if app.sidebar_resized {
            app.sidebar_resized = false;
            let full_size = terminal.size().unwrap_or_default();
            let rect = ui::compute_pty_rect(full_size.into(), app.sidebar_width);
            app.terminal_height = rect.height.max(1);

            let mut paned_sids: Vec<String> = Vec::new();
            if let Some(ref layout) = app.pane_layout {
                if layout.panes.len() > 1 {
                    let pane_rects = ui::compute_pane_rects(
                        full_size.into(),
                        layout.panes.len(),
                        app.sidebar_width,
                        layout.direction,
                    );
                    let block = ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .border_type(ratatui::widgets::BorderType::Thick);
                    for (i, content) in layout.panes.iter().enumerate() {
                        if let Some(sid) = content.session_id() {
                            let inner = block.inner(pane_rects[i]);
                            if let Some(session) = session_manager.get_mut(sid) {
                                let _ = session.resize(inner.height.max(1), inner.width.max(1));
                            }
                            paned_sids.push(sid.to_string());
                        }
                    }
                }
            }
            if paned_sids.is_empty() {
                session_manager.resize_all(rect.height.max(1), rect.width.max(1));
            } else {
                session_manager.resize_all_except(
                    &paned_sids,
                    rect.height.max(1),
                    rect.width.max(1),
                );
            }
        }

        // Handle layout direction change — resize all pane sessions
        if app.layout_changed {
            app.layout_changed = false;
            for (sid, rows, cols) in pane_sizes(terminal, app) {
                if let Some(session) = session_manager.get_mut(&sid) {
                    let _ = session.resize(rows, cols);
                }
            }
        }

        // Clean up sessions from deleted sections
        if !app.pending_removed_sessions.is_empty() {
            for sid in app.pending_removed_sessions.drain(..) {
                session_manager.remove(&sid);
            }
        }

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
    Ok(())
}

/// Send a native macOS notification. The msg format is "subtitle\nbody".
fn send_native_notification(msg: String) {
    std::thread::spawn(move || {
        let (subtitle, body) = match msg.split_once('\n') {
            Some((s, b)) => (s, b),
            None => ("", msg.as_str()),
        };
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let script = if subtitle.is_empty() {
            format!(
                "display notification \"{}\" with title \"Darya\"",
                esc(body)
            )
        } else {
            format!(
                "display notification \"{}\" with title \"Darya\" subtitle \"{}\"",
                esc(body),
                esc(subtitle)
            )
        };
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    });
}

fn process_event(
    event: &AppEvent,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    session_manager: &mut SessionManager,
    wt_manager: &WorktreeManager,
    pending_native: &mut std::collections::HashMap<String, (std::time::Instant, String)>,
    pending_attention: &mut std::collections::HashMap<String, std::time::Instant>,
) {
    // Track whether a main-loop operation consumed this key event.
    // When true, we skip app.handle_event() to prevent the key from leaking
    // into navigation handlers (e.g. Enter toggling section collapse after branch switch).
    let mut key_consumed = false;

    if let AppEvent::Key(_) = event {
        // Clear text selection on any keypress
        app.text_selection = None;
    }

    if let AppEvent::Key(key) = event {
        // Clear status message on any keypress
        app.status_message = None;
        // Ctrl+C: dismiss prompt → close active session → quit
        if key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('c')
        {
            if app.command_palette.is_some() {
                app.command_palette = None;
            } else if app.fuzzy_finder.is_some() {
                app.fuzzy_finder = None;
            } else if app.dir_browser.is_some() {
                app.dir_browser = None;
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
                app.cleanup_session(&session_id);
                app.input_mode = InputMode::Navigation;
                app.status_message = Some("Session closed".to_string());
            } else {
                app.running = false;
            }
        }

        // Fuzzy file finder keybinding
        if KeybindingsConfig::matches(&app.keybindings.fuzzy_finder, key.modifiers, key.code)
            && app.fuzzy_finder.is_none()
        {
            app.prompt = None; // dismiss any active prompt
            let root = app.file_explorer.root.clone();
            app.fuzzy_finder = Some(app::FuzzyFinderState::new(root));
            app.input_mode = InputMode::Navigation;
        }

        // Project search keybinding
        if KeybindingsConfig::matches(&app.keybindings.project_search, key.modifiers, key.code)
            && app.prompt.is_none()
            && app.fuzzy_finder.is_none()
        {
            app.prompt = Some(app::Prompt::SearchInput {
                input: String::new(),
            });
            app.input_mode = InputMode::Navigation;
        }

        // Command palette keybinding
        if KeybindingsConfig::matches(&app.keybindings.command_palette, key.modifiers, key.code)
            && app.command_palette.is_none()
        {
            app.prompt = None;
            app.fuzzy_finder = None;
            app.branch_switcher = None;
            app.command_palette = Some(app::CommandPaletteState::new(&app.keybindings));
            app.input_mode = InputMode::Navigation;
        }

        // Branch switcher keybinding
        if KeybindingsConfig::matches(&app.keybindings.branch_switcher, key.modifiers, key.code)
            && app.branch_switcher.is_none()
        {
            app.prompt = None;
            app.fuzzy_finder = None;
            app.command_palette = None;
            app.execute_command(app::CommandId::BranchSwitcher);
            app.input_mode = InputMode::Navigation;
        }

        // Mark key as consumed by main-loop handlers below
        // (prevents Enter leaking to PTY and to app.handle_event)

        // Handle worktree creation
        if let Some(branch_name) = app.wants_create_worktree(key) {
            match wt_manager.add(&branch_name) {
                Ok(()) => {
                    app.prompt = None;
                    if let Ok(worktrees) = wt_manager.list() {
                        app.refresh_worktrees(worktrees);
                        // Jump to last item (newly created)
                        let item_count = app.sidebar_tree.all_items().len();
                        if item_count > 0 {
                            app.sidebar_tree.jump_to_nth_item(item_count - 1);
                        }
                    }
                    app.status_message = Some(format!("Created worktree '{}'", branch_name));
                }
                Err(e) => {
                    app.prompt = None;
                    app.status_message = Some(format!("Error: {}", e));
                }
            }
        }
        // Handle worktree deletion
        else if app.wants_delete_worktree(key) {
            if let Some(item) = app.sidebar_tree.selected_item().cloned() {
                // Clean up all sessions for this item
                for slot in &item.sessions {
                    if let Some(ref session_id) = slot.session_id {
                        session_manager.remove(session_id);
                        app.cleanup_session(session_id);
                    }
                }
                match wt_manager.remove(&item.path) {
                    Ok(()) => {
                        app.prompt = None;
                        if let Ok(worktrees) = wt_manager.list() {
                            app.refresh_worktrees(worktrees);
                        }
                        app.status_message =
                            Some(format!("Deleted worktree '{}'", item.display_name));
                    }
                    Err(e) => {
                        app.prompt = None;
                        app.status_message = Some(format!("Error: {}", e));
                    }
                }
            }
        }
        // Handle branch switch
        if let Some((worktree_path, branch_name)) = app.wants_switch_branch(key) {
            match darya::worktree::manager::switch_branch(&worktree_path, &branch_name) {
                Ok(()) => {
                    app.branch_switcher = None;
                    app.status_message =
                        Some(format!("Switched to branch '{}'", branch_name));
                    // Refresh worktree list to show updated branch names
                    if let Ok(worktrees) = wt_manager.list() {
                        app.refresh_worktrees(worktrees);
                    }
                }
                Err(e) => {
                    app.branch_switcher = None;
                    app.status_message = Some(format!("Error: {}", e));
                }
            }
            key_consumed = true;
        }
        // Handle unified session spawning on Enter
        else if app.needs_session_spawn(key) {
            key_consumed = true;
            if let Some((kind, existing_id, wt_path)) = app
                .cursor_session_info()
                .map(|(k, id, p)| (k, id.map(|s| s.to_string()), p.clone()))
            {
                if let Some(id) = existing_id {
                    // Session already exists, just switch to it
                    app.attention_sessions.remove(&id);
                    match kind {
                        SessionKind::Claude => {
                            app.focus_terminal_panel();
                        }
                        SessionKind::Shell => {
                            app.panel_focus = app::PanelFocus::Right;
                            app.main_view = app::MainView::Shell;
                        }
                    }
                    app.input_mode = InputMode::Terminal;
                } else {
                    // No session yet — spawn one
                    let (rows, cols) = pty_size(terminal, app.sidebar_width);
                    let command = match kind {
                        SessionKind::Claude => {
                            config::resolve_session_command(&wt_path, &app.session_command)
                        }
                        SessionKind::Shell => {
                            config::resolve_shell_command(&wt_path, &app.shell_command)
                        }
                    };
                    match session_manager.spawn_session(
                        wt_path.clone(),
                        rows,
                        cols,
                        app.theme.mode,
                        &command,
                    ) {
                        Ok(id) => {
                            // Set session ID on the correct slot
                            if let Some((si, ii, slot_idx)) =
                                app.sidebar_tree.cursor_session_coords()
                            {
                                app.sidebar_tree
                                    .set_session_id(si, ii, slot_idx, id.clone());
                            }
                            match kind {
                                SessionKind::Claude => {
                                    app.focus_terminal_panel();
                                }
                                SessionKind::Shell => {
                                    app.panel_focus = app::PanelFocus::Right;
                                    app.main_view = app::MainView::Shell;
                                }
                            }
                            app.input_mode = InputMode::Terminal;
                            if command != CLAUDE_COMMAND && kind == SessionKind::Claude {
                                app.status_message = Some(format!("Started session ({})", command));
                            }
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Failed to start session: {}", e));
                        }
                    }
                }
            }
        }
        // Handle session restart on 'r' for exited sessions
        else if app.needs_session_restart(key) {
            key_consumed = true;
            if let Some((kind, Some(old_id), wt_path)) = app
                .cursor_session_info()
                .map(|(k, id, p)| (k, id.map(|s| s.to_string()), p.clone()))
            {
                let coords = app.sidebar_tree.cursor_session_coords();
                session_manager.remove(&old_id);
                app.cleanup_session(&old_id);

                let (rows, cols) = pty_size(terminal, app.sidebar_width);
                let command = match kind {
                    SessionKind::Claude => {
                        config::resolve_session_command(&wt_path, &app.session_command)
                    }
                    SessionKind::Shell => {
                        config::resolve_shell_command(&wt_path, &app.shell_command)
                    }
                };
                match session_manager.spawn_session(wt_path, rows, cols, app.theme.mode, &command) {
                    Ok(id) => {
                        if let Some((si, ii, slot_idx)) = coords {
                            app.sidebar_tree.set_session_id(si, ii, slot_idx, id);
                        }
                        match kind {
                            SessionKind::Claude => app.focus_terminal_panel(),
                            SessionKind::Shell => {
                                app.panel_focus = app::PanelFocus::Right;
                                app.main_view = app::MainView::Shell;
                            }
                        }
                        app.input_mode = InputMode::Terminal;
                        if command != CLAUDE_COMMAND && kind == SessionKind::Claude {
                            app.status_message = Some(format!("Started session ({})", command));
                        }
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Failed to restart session: {}", e));
                    }
                }
            }
        }
        // Handle session force-restart on Shift+R (works on running or exited sessions)
        else if app.needs_session_force_restart(key) {
            key_consumed = true;
            if let Some((kind, Some(old_id), wt_path)) = app
                .cursor_session_info()
                .map(|(k, id, p)| (k, id.map(|s| s.to_string()), p.clone()))
            {
                let coords = app.sidebar_tree.cursor_session_coords();
                session_manager.remove(&old_id);
                app.cleanup_session(&old_id);

                let (rows, cols) = pty_size(terminal, app.sidebar_width);
                let command = match kind {
                    SessionKind::Claude => {
                        config::resolve_session_command(&wt_path, &app.session_command)
                    }
                    SessionKind::Shell => {
                        config::resolve_shell_command(&wt_path, &app.shell_command)
                    }
                };
                match session_manager.spawn_session(wt_path, rows, cols, app.theme.mode, &command) {
                    Ok(id) => {
                        if let Some((si, ii, slot_idx)) = coords {
                            app.sidebar_tree.set_session_id(si, ii, slot_idx, id);
                        }
                        match kind {
                            SessionKind::Claude => app.focus_terminal_panel(),
                            SessionKind::Shell => {
                                app.panel_focus = app::PanelFocus::Right;
                                app.main_view = app::MainView::Shell;
                            }
                        }
                        app.input_mode = InputMode::Terminal;
                        if command != CLAUDE_COMMAND && kind == SessionKind::Claude {
                            app.status_message = Some(format!("Restarted session ({})", command));
                        }
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Failed to restart session: {}", e));
                    }
                }
            }
        }
        // Handle session close on Backspace
        else if app.needs_session_close(key) {
            key_consumed = true;
            if let Some(session_id) = app.cursor_session_id().map(|s| s.to_string()) {
                session_manager.remove(&session_id);
                app.cleanup_session(&session_id);
                app.status_message = Some("Session closed".to_string());
            }
        }

        // Split pane (Navigation mode only)
        if app.input_mode == InputMode::Navigation
            && KeybindingsConfig::matches(&app.keybindings.split_pane, key.modifiers, key.code)
            && app.split_add_pane()
        {
            // Resize sessions to new pane dimensions
            for (sid, rows, cols) in pane_sizes(terminal, app) {
                if let Some(session) = session_manager.get_mut(&sid) {
                    let _ = session.resize(rows, cols);
                }
            }
        }

        // Split pane vertical (Navigation mode only)
        if app.input_mode == InputMode::Navigation
            && KeybindingsConfig::matches(
                &app.keybindings.split_pane_vertical,
                key.modifiers,
                key.code,
            )
        {
            app.split_direction = darya::app::SplitDirection::Vertical;
            if app.split_add_pane() {
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
                let (rows, cols) = pty_size(terminal, app.sidebar_width);
                session_manager.resize_all(rows, cols);
            } else {
                for (sid, rows, cols) in sizes {
                    if let Some(session) = session_manager.get_mut(&sid) {
                        let _ = session.resize(rows, cols);
                    }
                }
            }
        }

        // Shift+PageUp/Down: scroll in ANY mode (intercept before PTY)
        if key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::PageUp {
            app.scroll_up(app.terminal_height.saturating_sub(2) as usize);
        } else if key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::PageDown {
            app.scroll_down(app.terminal_height.saturating_sub(2) as usize);
        }
        // Forward keys to PTY in terminal mode
        else if !key_consumed && app.input_mode == InputMode::Terminal && app.prompt.is_none() {
            // Don't forward Tab — it switches to sidebar
            if key.code != KeyCode::Tab {
                if let Some(session_id) = app.focused_session_id().cloned() {
                    if !app.exited_sessions.contains(session_id.as_str()) {
                        if let Some(bytes) = key_event_to_bytes(key) {
                            if let Some(session) = session_manager.get_mut(&session_id) {
                                let _ = session.write_input(&bytes);
                                app.activity.mark_input(&session_id);
                                // Reset scroll to live view on user input
                                app.scroll_offsets.remove(&session_id);
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
        // Capture window title (OSC 0/2) as session status
        if let Some(status) = session_manager.session_status(session_id) {
            if !status.is_empty() {
                app.session_statuses.insert(session_id.clone(), status);
            }
        }
    }

    // Handle mouse scroll — works in ALL modes
    if let AppEvent::MouseScroll { delta } = event {
        app.text_selection = None;
        if *delta > 0 {
            app.scroll_up(*delta as usize);
        } else if *delta < 0 {
            app.scroll_down((-delta) as usize);
        }
    }

    // Handle mouse down — start text selection
    if let AppEvent::MouseDown { column, row } = event {
        let term_size = terminal.size().unwrap_or_default();
        if let Some((session_id, inner)) =
            app.pane_session_at_coords(*column, *row, term_size.into())
        {
            let screen_row = row.saturating_sub(inner.y);
            let screen_col = column.saturating_sub(inner.x);
            app.text_selection = Some(app::TextSelection {
                session_id,
                pane_inner: inner,
                start: (screen_row, screen_col),
                end: (screen_row, screen_col),
                active: true,
            });
        } else {
            app.text_selection = None;
        }
    }

    // Handle mouse drag — extend text selection
    if let AppEvent::MouseDrag { column, row } = event {
        if let Some(ref mut sel) = app.text_selection {
            if sel.active {
                let inner = sel.pane_inner;
                let clamped_col = (*column).clamp(inner.x, inner.x + inner.width.saturating_sub(1));
                let clamped_row = (*row).clamp(inner.y, inner.y + inner.height.saturating_sub(1));
                sel.end = (
                    clamped_row.saturating_sub(inner.y),
                    clamped_col.saturating_sub(inner.x),
                );
            }
        }
    }

    // Handle mouse up — finalize selection, extract text, copy to clipboard
    if let AppEvent::MouseUp { column, row } = event {
        if let Some(ref mut sel) = app.text_selection {
            if sel.active {
                let inner = sel.pane_inner;
                let clamped_col = (*column).clamp(inner.x, inner.x + inner.width.saturating_sub(1));
                let clamped_row = (*row).clamp(inner.y, inner.y + inner.height.saturating_sub(1));
                sel.end = (
                    clamped_row.saturating_sub(inner.y),
                    clamped_col.saturating_sub(inner.x),
                );
                sel.active = false;
            }
        }
        // Extract text and copy to clipboard
        if let Some(ref sel) = app.text_selection {
            if !sel.active {
                let text = extract_selection_text(sel, session_manager, app);
                if !text.is_empty() {
                    copy_to_clipboard(&text);
                    app.status_message = Some("Copied to clipboard".to_string());
                } else {
                    // Empty selection (single click) — clear it
                    app.text_selection = None;
                }
            }
        }
    }

    // Handle paste — forward to PTY with bracketed paste wrapping
    if let AppEvent::Paste(ref text) = event {
        if app.input_mode == InputMode::Terminal && app.prompt.is_none() {
            if let Some(session_id) = app.focused_session_id().cloned() {
                if !app.exited_sessions.contains(session_id.as_str()) {
                    if let Some(session) = session_manager.get_mut(&session_id) {
                        let use_bracketed = session
                            .parser
                            .read()
                            .map(|p| p.screen().bracketed_paste())
                            .unwrap_or(false);
                        let payload = if use_bracketed {
                            let mut buf = Vec::with_capacity(text.len() + 12);
                            buf.extend_from_slice(b"\x1b[200~");
                            buf.extend_from_slice(text.as_bytes());
                            buf.extend_from_slice(b"\x1b[201~");
                            buf
                        } else {
                            text.as_bytes().to_vec()
                        };
                        let _ = session.write_input(&payload);
                        app.activity.mark_input(&session_id);
                        app.scroll_offsets.remove(&session_id);
                    }
                }
            }
        }
    }

    // Handle resize
    if let AppEvent::Resize(w, h) = &event {
        let full_size = Rect::new(0, 0, *w, *h);
        let rect = ui::compute_pty_rect(full_size, app.sidebar_width);
        app.terminal_height = rect.height.max(1);

        // Collect all pane session IDs that need custom sizing
        let mut paned_sids: Vec<String> = Vec::new();

        // Resize panes (only Terminal/Shell have PTY sessions)
        if let Some(ref layout) = app.pane_layout {
            if layout.panes.len() > 1 {
                let pane_rects = ui::compute_pane_rects(
                    full_size,
                    layout.panes.len(),
                    app.sidebar_width,
                    layout.direction,
                );
                let block = ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Thick);
                for (i, content) in layout.panes.iter().enumerate() {
                    if let Some(sid) = content.session_id() {
                        let inner = block.inner(pane_rects[i]);
                        if let Some(session) = session_manager.get_mut(sid) {
                            let _ = session.resize(inner.height.max(1), inner.width.max(1));
                        }
                        paned_sids.push(sid.to_string());
                    }
                }
            }
        }

        if paned_sids.is_empty() {
            session_manager.resize_all(rect.height.max(1), rect.width.max(1));
        } else {
            session_manager.resize_all_except(&paned_sids, rect.height.max(1), rect.width.max(1));
        }
    }

    // Send notifications
    let (iterm_msg, native_msg) = app.notification_for_event(event);
    if let Some(msg) = iterm_msg {
        let _ = write!(terminal.backend_mut(), "\x1b]9;{}\x07\x07", msg);
        let _ = terminal.backend_mut().flush();
    }
    // Native notifications: only SessionExited fires (immediately)
    if let Some(msg) = native_msg {
        send_native_notification(msg);
    }

    // Debounced attention indicator: queue on SessionDone, cancel on PtyOutput,
    // set attention_sessions after timeout. This prevents every tool call completion
    // from turning the sidebar green — only truly idle sessions get marked.
    match event {
        AppEvent::SessionDone { ref session_id } => {
            // Only queue if not currently viewing this session
            let viewing = app.focused_session_id().map(|s| s.as_str()) == Some(session_id)
                && app.input_mode == InputMode::Terminal;
            if !viewing {
                pending_attention.insert(session_id.clone(), std::time::Instant::now());
            }
        }
        AppEvent::SessionExited { ref session_id } => {
            // Session exited — immediate attention, no debounce needed
            pending_attention.remove(session_id);
            app.attention_sessions.insert(session_id.clone());
        }
        AppEvent::PtyOutput { ref session_id } => {
            // New output cancels pending attention — session is still working
            pending_attention.remove(session_id);
            pending_native.remove(session_id);
        }
        _ => {}
    }

    if !key_consumed {
        app.handle_event(event);
    }
}

/// Restore sessions from a saved layout config.
fn restore_sessions(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    session_manager: &mut SessionManager,
    layout: &config::LayoutConfig,
) {
    let (rows, cols) = pty_size(terminal, app.sidebar_width);

    for saved in &layout.sessions {
        let saved_path = PathBuf::from(&saved.path);
        let saved_kind = match saved.slot_kind.as_str() {
            "claude" => SessionKind::Claude,
            "shell" => SessionKind::Shell,
            _ => continue,
        };

        // Find matching sidebar item by path, then matching slot by kind + label
        let mut found = None;
        for (si, section) in app.sidebar_tree.sections.iter().enumerate() {
            for (ii, item) in section.items.iter().enumerate() {
                if item.path == saved_path {
                    for (slot_idx, slot) in item.sessions.iter().enumerate() {
                        if slot.kind == saved_kind
                            && slot.label == saved.slot_label
                            && slot.session_id.is_none()
                        {
                            found = Some((si, ii, slot_idx));
                            break;
                        }
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }

        let Some((si, ii, slot_idx)) = found else {
            continue;
        };

        let command = match saved_kind {
            SessionKind::Claude => {
                let base = config::resolve_session_command(&saved_path, &app.session_command);
                format!("{} --continue", base)
            }
            SessionKind::Shell => config::resolve_shell_command(&saved_path, &app.shell_command),
        };

        if let Ok(id) =
            session_manager.spawn_session(saved_path, rows, cols, app.theme.mode, &command)
        {
            app.sidebar_tree.set_session_id(si, ii, slot_idx, id);
        }
    }

    // Restore UI state
    if let Some(ref sv) = layout.sidebar_view {
        app.sidebar_view = match sv.as_str() {
            "worktrees" => app::SidebarView::Worktrees,
            "files" => app::SidebarView::FileExplorer,
            "search" => app::SidebarView::Search,
            "git_status" => app::SidebarView::GitStatus,
            _ => app.sidebar_view,
        };
    }
    if let Some(ref mv) = layout.main_view {
        app.main_view = match mv.as_str() {
            "terminal" => app::MainView::Terminal,
            "editor" => app::MainView::Editor,
            "diff" => app::MainView::DiffView,
            "blame" => app::MainView::GitBlame,
            "log" => app::MainView::GitLog,
            "shell" => app::MainView::Shell,
            _ => app.main_view,
        };
    }
    if let Some(ref pf) = layout.panel_focus {
        app.panel_focus = match pf.as_str() {
            "left" => app::PanelFocus::Left,
            "right" => app::PanelFocus::Right,
            _ => app.panel_focus,
        };
    }
}

/// Extract text from a finalized TextSelection using the vt100 screen contents.
fn extract_selection_text(
    sel: &app::TextSelection,
    session_manager: &SessionManager,
    app: &App,
) -> String {
    let Some(session) = session_manager.get(&sel.session_id) else {
        return String::new();
    };
    let Ok(mut parser) = session.parser.write() else {
        return String::new();
    };

    let offset = app.scroll_offset_for(&sel.session_id);
    parser.screen_mut().set_scrollback(offset);

    let screen = parser.screen();

    // Account for bottom-align shift: find last content row and compute shift
    let rows = screen.size().0;
    let cols = screen.size().1;
    let mut last_content_row: u16 = 0;
    if app.terminal_start_bottom {
        for r in (0..rows).rev() {
            let mut has_content = false;
            for c in 0..cols {
                let cell = screen.cell(r, c);
                if let Some(cell) = cell {
                    if cell.contents() != " " && !cell.contents().is_empty() {
                        has_content = true;
                        break;
                    }
                }
            }
            if has_content {
                last_content_row = r;
                break;
            }
        }
    }

    let pane_height = sel.pane_inner.height;
    let shift = if app.terminal_start_bottom && pane_height > 0 {
        (pane_height.saturating_sub(1)).saturating_sub(last_content_row)
    } else {
        0
    };

    // Map selection coords back to screen coords accounting for bottom-align shift
    let (mut sr, sc) = sel.start;
    let (mut er, ec) = sel.end;
    sr = sr.saturating_sub(shift);
    er = er.saturating_sub(shift);

    // Normalize: ensure start is before end
    let (start_row, start_col, end_row, end_col) = if (sr, sc) <= (er, ec) {
        (sr, sc, er, ec)
    } else {
        (er, ec, sr, sc)
    };

    screen.contents_between(
        start_row,
        start_col,
        end_row,
        end_col.saturating_add(1).min(cols),
    )
}

/// Copy text to clipboard using OSC 52 escape sequence (terminal-native, works in
/// iTerm2, Kitty, and over SSH without needing pbcopy).
fn copy_to_clipboard(text: &str) {
    let encoded = base64_encode(text.as_bytes());
    // OSC 52: \x1b]52;c;<base64>\x07
    let osc = format!("\x1b]52;c;{}\x07", encoded);
    let _ = io::stdout().write_all(osc.as_bytes());
    let _ = io::stdout().flush();
}

/// Minimal base64 encoder (no external dependency).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
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
    let modifier_param =
        1 + if key.modifiers.contains(KeyModifiers::SHIFT) {
            1
        } else {
            0
        } + if has_alt { 2 } else { 0 }
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
