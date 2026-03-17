#![allow(dead_code)]

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use darya::app::App;
use darya::config::{KeybindingsConfig, Theme, CLAUDE_COMMAND};
use darya::event::AppEvent;
use darya::sidebar::types::SessionKind;
use darya::worktree::types::Worktree;

/// Create an App with `n` mock worktrees using temp-like paths.
pub fn make_app(n: usize) -> App {
    let worktrees = make_worktrees(n);
    App::new(worktrees, Theme::dark(), true, KeybindingsConfig::default(), CLAUDE_COMMAND.to_string(), "/bin/sh".to_string())
}

/// Create an App with `n` worktrees where the first worktree has an active session mapped.
pub fn make_app_with_session(n: usize) -> App {
    let mut app = make_app(n);
    let session_id = "test-session-1".to_string();
    // Set session on the first item's first (Claude) slot
    app.sidebar_tree.set_session_id(0, 0, 0, session_id);
    app
}

/// Get the path for item at index i (0-based) from sidebar tree.
pub fn item_path(app: &App, i: usize) -> PathBuf {
    app.sidebar_tree.sections[0].items[i].path.clone()
}

/// Get the selected item index (among items, not visible nodes).
pub fn selected_item_index(app: &App) -> Option<usize> {
    app.sidebar_tree.selected_item_index()
}

/// Set a Claude session ID for item at index.
pub fn set_session(app: &mut App, item_idx: usize, session_id: &str) {
    app.sidebar_tree.set_session_id(0, item_idx, 0, session_id.to_string());
}

/// Set a Shell session for item at index (adds a shell slot if needed, then sets ID).
pub fn set_shell_session(app: &mut App, item_idx: usize, session_id: &str) {
    let item = &app.sidebar_tree.sections[0].items[item_idx];
    let shell_slot = item.sessions.iter().position(|s| s.kind == SessionKind::Shell);
    if let Some(slot_idx) = shell_slot {
        app.sidebar_tree.set_session_id(0, item_idx, slot_idx, session_id.to_string());
    } else {
        // Add a shell slot
        let item = &mut app.sidebar_tree.sections[0].items[item_idx];
        item.sessions.push(darya::sidebar::types::SessionSlot {
            kind: SessionKind::Shell,
            label: "shell".to_string(),
            session_id: Some(session_id.to_string()),
            color: None,
        });
        app.sidebar_tree.rebuild_visible();
    }
}

/// Get the active Claude session ID for the currently selected item.
pub fn active_session_id(app: &App) -> Option<&str> {
    app.active_session_id()
}

/// Get the active shell session ID for the currently selected item.
pub fn active_shell_session_id(app: &App) -> Option<&str> {
    app.active_shell_session_id()
}

pub fn make_worktrees(n: usize) -> Vec<Worktree> {
    (0..n)
        .map(|i| {
            let name = if i == 0 {
                "my-project".to_string()
            } else {
                format!("my-project-feature-{}", i)
            };
            Worktree {
                name: name.clone(),
                path: PathBuf::from(format!("/tmp/test-worktrees/{}", name)),
                branch: Some(if i == 0 {
                    "main".to_string()
                } else {
                    format!("feature-{}", i)
                }),
                is_main: i == 0,
            }
        })
        .collect()
}

/// Shorthand for creating a KeyEvent with no modifiers.
pub fn key(code: KeyCode) -> AppEvent {
    AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Shorthand for creating a Ctrl+key event.
pub fn ctrl_key(c: char) -> AppEvent {
    AppEvent::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::CONTROL,
    ))
}

/// Shorthand for creating a Cmd (Super) key event.
pub fn cmd_key(c: char) -> AppEvent {
    AppEvent::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::SUPER,
    ))
}

/// Shorthand for creating an Alt+key event.
pub fn alt_key(c: char) -> AppEvent {
    AppEvent::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::ALT,
    ))
}

/// Shorthand for creating a Shift+key event (for non-char keys like PageUp/PageDown).
pub fn shift_key(code: KeyCode) -> AppEvent {
    AppEvent::Key(KeyEvent::new(code, KeyModifiers::SHIFT))
}

/// Create an App with `n` worktrees where the first two worktrees have active sessions.
pub fn make_app_with_two_sessions(n: usize) -> App {
    assert!(n >= 2);
    let mut app = make_app(n);
    set_session(&mut app, 0, "test-session-1");
    set_session(&mut app, 1, "test-session-2");
    app
}
