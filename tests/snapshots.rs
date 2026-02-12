mod helpers;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use darya::app::{
    App, DiffLine, DiffLineKind, DiffViewState, GitFileStatus, GitStatusCategory,
    GitStatusEntry, GitStatusState, InputMode, MainView, PanelFocus, SidebarView,
};
use darya::config::{KeybindingsConfig, Theme};
use darya::session::manager::SessionManager;

use helpers::make_worktrees;

/// Render the full UI frame into a string buffer for snapshot comparison.
fn render_to_string(app: &mut App, session_manager: &SessionManager, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| darya::ui::draw(frame, app, session_manager))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    buffer_to_string(&buffer)
}

fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area;
    let mut lines = Vec::new();
    for y in area.y..area.y + area.height {
        let mut line = String::new();
        for x in area.x..area.x + area.width {
            let cell = &buf[(x, y)];
            line.push_str(cell.symbol());
        }
        // Trim trailing spaces for cleaner snapshots
        let trimmed = line.trim_end();
        lines.push(trimmed.to_string());
    }
    // Remove trailing empty lines
    while lines.last().map_or(false, |l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn make_test_app(n_worktrees: usize) -> App {
    let worktrees = make_worktrees(n_worktrees);
    App::new(worktrees, Theme::dark(), true, KeybindingsConfig::default())
}

fn make_session_manager() -> SessionManager {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    SessionManager::new(tx)
}

// ── Worktree list snapshot ──────────────────────────────────

#[test]
fn snapshot_worktree_list_3_items() {
    let mut app = make_test_app(3);
    // Add session state to make it interesting
    let wt0_path = app.worktrees[0].path.clone();
    let wt2_path = app.worktrees[2].path.clone();
    app.session_ids.insert(wt0_path, "session-0".to_string());
    app.session_ids.insert(wt2_path, "session-2".to_string());
    app.active_session_id = Some("session-0".to_string());
    app.exited_sessions.insert("session-2".to_string());
    app.selected_worktree = 0;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 20);
    insta::assert_snapshot!("worktree_list_3_items", output);
}

// ── Status bar snapshots ────────────────────────────────────

#[test]
fn snapshot_status_bar_nav_mode() {
    let mut app = make_test_app(2);
    app.input_mode = InputMode::Navigation;
    app.panel_focus = PanelFocus::Left;
    app.sidebar_view = SidebarView::Worktrees;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 5);
    insta::assert_snapshot!("status_bar_nav", output);
}

#[test]
fn snapshot_status_bar_terminal_mode() {
    let mut app = make_test_app(2);
    app.input_mode = InputMode::Terminal;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 5);
    insta::assert_snapshot!("status_bar_terminal", output);
}

#[test]
fn snapshot_status_bar_with_message() {
    let mut app = make_test_app(2);
    app.status_message = Some("Session closed".to_string());

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 5);
    insta::assert_snapshot!("status_bar_message", output);
}

// ── Help overlay snapshot ───────────────────────────────────

#[test]
fn snapshot_help_overlay_worktrees() {
    let mut app = make_test_app(2);
    app.show_help = true;
    app.panel_focus = PanelFocus::Left;
    app.sidebar_view = SidebarView::Worktrees;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("help_overlay_worktrees", output);
}

#[test]
fn snapshot_help_overlay_terminal() {
    let mut app = make_test_app(2);
    app.show_help = true;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("help_overlay_terminal", output);
}

// ── Full layout snapshot ────────────────────────────────────

#[test]
fn snapshot_full_layout_default() {
    let mut app = make_test_app(3);
    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 120, 30);
    insta::assert_snapshot!("full_layout_default", output);
}

// ── Git Status sidebar snapshot ─────────────────────────────

#[test]
fn snapshot_git_status_sidebar_mixed() {
    let mut app = make_test_app(3);
    app.git_status = Some(GitStatusState {
        entries: vec![
            GitStatusEntry {
                category: GitStatusCategory::Staged,
                status: GitFileStatus::Modified,
                path: "src/app.rs".to_string(),
                orig_path: None,
            },
            GitStatusEntry {
                category: GitStatusCategory::Unstaged,
                status: GitFileStatus::Modified,
                path: "src/ui.rs".to_string(),
                orig_path: None,
            },
            GitStatusEntry {
                category: GitStatusCategory::Untracked,
                status: GitFileStatus::Untracked,
                path: "new_file.txt".to_string(),
                orig_path: None,
            },
        ],
        selected: 0,
        error: None,
        worktree_path: app.worktrees[0].path.clone(),
    });
    app.sidebar_view = SidebarView::GitStatus;
    app.panel_focus = PanelFocus::Left;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 20);
    insta::assert_snapshot!("git_status_sidebar_mixed", output);
}

// ── Diff view snapshot ──────────────────────────────────────

#[test]
fn snapshot_diff_view_with_changes() {
    let mut app = make_test_app(3);
    app.diff_view = Some(DiffViewState {
        file_path: "src/app.rs".to_string(),
        lines: vec![
            DiffLine { kind: DiffLineKind::Header, content: "diff --git a/src/app.rs b/src/app.rs".to_string() },
            DiffLine { kind: DiffLineKind::Header, content: "@@ -1,3 +1,4 @@".to_string() },
            DiffLine { kind: DiffLineKind::Context, content: " use std::collections::HashMap;".to_string() },
            DiffLine { kind: DiffLineKind::Addition, content: "+use std::process::Command;".to_string() },
            DiffLine { kind: DiffLineKind::Context, content: " use crossterm::event::KeyCode;".to_string() },
            DiffLine { kind: DiffLineKind::Deletion, content: "-use old_crate::Something;".to_string() },
        ],
        scroll_offset: 0,
        visible_height: 20,
    });
    app.main_view = MainView::DiffView;
    app.panel_focus = PanelFocus::Right;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 20);
    insta::assert_snapshot!("diff_view_with_changes", output);
}

// ── Help overlay for GitStatus ──────────────────────────────

#[test]
fn snapshot_help_overlay_git_status() {
    let mut app = make_test_app(2);
    app.show_help = true;
    app.panel_focus = PanelFocus::Left;
    app.sidebar_view = SidebarView::GitStatus;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("help_overlay_git_status", output);
}

// ── Help overlay for DiffView ───────────────────────────────

#[test]
fn snapshot_help_overlay_diff_view() {
    let mut app = make_test_app(2);
    app.show_help = true;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::DiffView;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("help_overlay_diff_view", output);
}
