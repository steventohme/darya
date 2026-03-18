mod helpers;

use crossterm::event::KeyCode;

use darya::app::Prompt;
use darya::config;

use helpers::{key, make_app};

// ── Setup done marker ────────────────────────────────────────

#[test]
fn setup_done_returns_false_when_no_marker() {
    let dir = tempfile::tempdir().unwrap();
    // Point HOME at a temp dir with no marker file
    std::env::set_var("HOME", dir.path());
    assert!(!config::setup_done());
}

#[test]
fn mark_setup_done_creates_marker() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", dir.path());
    assert!(!config::setup_done());
    config::mark_setup_done();
    assert!(config::setup_done());
    assert!(dir.path().join(".config/darya/.setup_done").exists());
}

// ── Prompt dismiss ───────────────────────────────────────────

#[test]
fn setup_guide_dismissed_on_enter() {
    let mut app = make_app(1);
    app.prompt = Some(Prompt::SetupGuide);
    app.handle_event(&key(KeyCode::Enter));
    assert!(app.prompt.is_none());
}

#[test]
fn setup_guide_dismissed_on_esc() {
    let mut app = make_app(1);
    app.prompt = Some(Prompt::SetupGuide);
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.prompt.is_none());
}

#[test]
fn setup_guide_not_dismissed_on_other_keys() {
    let mut app = make_app(1);
    app.prompt = Some(Prompt::SetupGuide);
    app.handle_event(&key(KeyCode::Char('j')));
    assert!(matches!(app.prompt, Some(Prompt::SetupGuide)));
}
