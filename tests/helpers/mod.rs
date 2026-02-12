#![allow(dead_code)]

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use darya::app::App;
use darya::config::{KeybindingsConfig, Theme, CLAUDE_COMMAND};
use darya::event::AppEvent;
use darya::worktree::types::Worktree;

/// Create an App with `n` mock worktrees using temp-like paths.
pub fn make_app(n: usize) -> App {
    let worktrees = make_worktrees(n);
    App::new(worktrees, Theme::dark(), true, KeybindingsConfig::default(), CLAUDE_COMMAND.to_string())
}

/// Create an App with `n` worktrees where the first worktree has an active session mapped.
pub fn make_app_with_session(n: usize) -> App {
    let mut app = make_app(n);
    if let Some(wt) = app.worktrees.first() {
        let session_id = "test-session-1".to_string();
        app.session_ids.insert(wt.path.clone(), session_id.clone());
        app.active_session_id = Some(session_id);
    }
    app
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

/// Shorthand for creating an Alt+key event.
pub fn alt_key(c: char) -> AppEvent {
    AppEvent::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::ALT,
    ))
}

/// Create an App with `n` worktrees where the first two worktrees have active sessions.
pub fn make_app_with_two_sessions(n: usize) -> App {
    assert!(n >= 2);
    let mut app = make_app(n);
    let wt0 = app.worktrees[0].path.clone();
    let wt1 = app.worktrees[1].path.clone();
    let s0 = "test-session-1".to_string();
    let s1 = "test-session-2".to_string();
    app.session_ids.insert(wt0, s0.clone());
    app.session_ids.insert(wt1, s1);
    app.active_session_id = Some(s0);
    app
}
