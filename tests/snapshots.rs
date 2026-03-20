mod helpers;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use darya::app::{
    App, BlameLine, CommandPaletteState, DiffLine, DiffLineKind, DiffViewState, FileExplorerState,
    GitBlameState, GitFileStatus, GitLogEntry, GitLogState, GitStatusCategory, GitStatusEntry,
    GitStatusState, InputMode, MainView, PaneContent, PaneLayout, PanelFocus, Prompt, SidebarView,
    SplitDirection,
};
use darya::config::{KeybindingsConfig, Theme};
use darya::planet::sprites::PlanetAnimation;
use darya::planet::types::PlanetKind;
use darya::session::manager::SessionManager;

use helpers::{item_path, make_worktrees, set_session};

/// Render the full UI frame into a string buffer for snapshot comparison.
fn render_to_string(
    app: &mut App,
    session_manager: &SessionManager,
    width: u16,
    height: u16,
) -> String {
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
    App::new(
        worktrees,
        Theme::dark(),
        true,
        KeybindingsConfig::default(),
        darya::config::CLAUDE_COMMAND.to_string(),
        "/bin/sh".to_string(),
    )
}

fn make_session_manager() -> SessionManager {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    SessionManager::new(tx)
}

// ── Worktree list snapshot ──────────────────────────────────

#[test]
fn snapshot_worktree_list_3_items() {
    let mut app = make_test_app(3);
    // Move cursor to item 0 (past the section header)
    app.sidebar_tree.jump_to_nth_item(0);
    set_session(&mut app, 0, "session-0");
    set_session(&mut app, 2, "session-2");
    app.exited_sessions.insert("session-2".to_string());

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

// ── Status bar with session counts ──────────────────────────

#[test]
fn snapshot_status_bar_with_sessions() {
    let mut app = make_test_app(3);
    // Move cursor to item 0 so active_session_id returns "s0"
    app.sidebar_tree.jump_to_nth_item(0);
    set_session(&mut app, 0, "s0");
    set_session(&mut app, 1, "s1");
    set_session(&mut app, 2, "s2");
    app.exited_sessions.insert("s2".to_string());

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 5);
    insta::assert_snapshot!("status_bar_with_sessions", output);
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
        worktree_path: item_path(&app, 0),
        stale: false,
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
            DiffLine {
                kind: DiffLineKind::Header,
                content: "diff --git a/src/app.rs b/src/app.rs".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Header,
                content: "@@ -1,3 +1,4 @@".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                content: " use std::collections::HashMap;".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Addition,
                content: "+use std::process::Command;".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                content: " use crossterm::event::KeyCode;".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Deletion,
                content: "-use old_crate::Something;".to_string(),
            },
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

// ── Split pane snapshots ────────────────────────────────────

#[test]
fn snapshot_split_two_panes() {
    let mut app = make_test_app(3);
    app.sidebar_tree.jump_to_nth_item(0);
    set_session(&mut app, 0, "s0");
    set_session(&mut app, 1, "s1");
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;
    app.pane_layout = Some(PaneLayout {
        panes: vec![
            PaneContent::Terminal("s0".to_string()),
            PaneContent::Terminal("s1".to_string()),
        ],
        focused: 0,
        direction: SplitDirection::Horizontal,
    });

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 120, 20);
    insta::assert_snapshot!("split_two_panes", output);
}

#[test]
fn snapshot_split_focused_pane_highlighted() {
    let mut app = make_test_app(3);
    app.sidebar_tree.jump_to_nth_item(0);
    set_session(&mut app, 0, "s0");
    set_session(&mut app, 1, "s1");
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;
    // Focus is on second pane
    app.pane_layout = Some(PaneLayout {
        panes: vec![
            PaneContent::Terminal("s0".to_string()),
            PaneContent::Terminal("s1".to_string()),
        ],
        focused: 1,
        direction: SplitDirection::Horizontal,
    });

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 120, 20);
    insta::assert_snapshot!("split_focused_pane_highlighted", output);
}

#[test]
fn snapshot_split_two_panes_vertical() {
    let mut app = make_test_app(3);
    app.sidebar_tree.jump_to_nth_item(0);
    set_session(&mut app, 0, "s0");
    set_session(&mut app, 1, "s1");
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;
    app.pane_layout = Some(PaneLayout {
        panes: vec![
            PaneContent::Terminal("s0".to_string()),
            PaneContent::Terminal("s1".to_string()),
        ],
        focused: 0,
        direction: SplitDirection::Vertical,
    });

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 120, 20);
    insta::assert_snapshot!("split_two_panes_vertical", output);
}

// ── File explorer with git indicators ───────────────────────

#[test]
fn snapshot_file_explorer_with_git_indicators() {
    // Create a temp dir with a stable-named project dir inside
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("my-project");
    std::fs::create_dir(&project).unwrap();
    let src_dir = project.join("src");
    std::fs::create_dir(&src_dir).unwrap();
    std::fs::write(src_dir.join("app.rs"), "").unwrap();
    std::fs::write(src_dir.join("main.rs"), "").unwrap();
    std::fs::write(project.join("README.md"), "").unwrap();

    let mut app = make_test_app(2);
    app.file_explorer = FileExplorerState::new(project);
    // Expand the src directory
    app.file_explorer.expanded.insert(src_dir.clone());
    app.file_explorer.refresh();

    // Inject git indicators matching relative paths
    app.file_explorer
        .git_indicators
        .insert("src/app.rs".to_string(), GitFileStatus::Modified);
    app.file_explorer
        .git_indicators
        .insert("README.md".to_string(), GitFileStatus::Untracked);
    app.file_explorer
        .git_indicators
        .insert("src/main.rs".to_string(), GitFileStatus::Added);
    app.file_explorer.git_indicators_stale = false; // prevent ensure_git_indicators from clearing injected data

    app.sidebar_view = SidebarView::FileExplorer;
    app.panel_focus = PanelFocus::Left;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 15);
    insta::assert_snapshot!("file_explorer_with_git_indicators", output);
}

// ── Git Blame snapshot ──────────────────────────────────────

#[test]
fn snapshot_git_blame_view() {
    let mut app = make_test_app(3);
    app.git_blame = Some(GitBlameState {
        file_path: "src/main.rs".to_string(),
        lines: vec![
            BlameLine {
                commit_short: "abc12345".to_string(),
                author: "Alice".to_string(),
                relative_time: "2 days ago".to_string(),
                line_number: 1,
                content: "use std::io;".to_string(),
                is_recent: true,
            },
            BlameLine {
                commit_short: "def67890".to_string(),
                author: "Bob".to_string(),
                relative_time: "3 months ago".to_string(),
                line_number: 2,
                content: "fn main() {".to_string(),
                is_recent: false,
            },
            BlameLine {
                commit_short: "def67890".to_string(),
                author: "Bob".to_string(),
                relative_time: "3 months ago".to_string(),
                line_number: 3,
                content: "    println!(\"hello\");".to_string(),
                is_recent: false,
            },
            BlameLine {
                commit_short: "abc12345".to_string(),
                author: "Alice".to_string(),
                relative_time: "2 days ago".to_string(),
                line_number: 4,
                content: "}".to_string(),
                is_recent: true,
            },
        ],
        scroll_offset: 0,
        visible_height: 20,
        worktree_path: item_path(&app, 0),
        stale: false,
    });
    app.main_view = MainView::GitBlame;
    app.panel_focus = PanelFocus::Right;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 15);
    insta::assert_snapshot!("git_blame_view", output);
}

// ── Git Log snapshot ────────────────────────────────────────

#[test]
fn snapshot_git_log_view() {
    let mut app = make_test_app(3);
    app.git_log = Some(GitLogState {
        entries: vec![
            GitLogEntry {
                hash_short: "abc1234".to_string(),
                subject: "Fix bug in parser".to_string(),
                author: "Alice".to_string(),
                relative_date: "2 hours ago".to_string(),
            },
            GitLogEntry {
                hash_short: "def5678".to_string(),
                subject: "Add new feature".to_string(),
                author: "Bob".to_string(),
                relative_date: "3 days ago".to_string(),
            },
            GitLogEntry {
                hash_short: "ghi9012".to_string(),
                subject: "Initial commit".to_string(),
                author: "Alice".to_string(),
                relative_date: "2 weeks ago".to_string(),
            },
        ],
        selected: 0,
        scroll_offset: 0,
        visible_height: 20,
        worktree_path: item_path(&app, 0),
        file_filter: None,
        stale: false,
    });
    app.main_view = MainView::GitLog;
    app.panel_focus = PanelFocus::Right;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 15);
    insta::assert_snapshot!("git_log_view", output);
}

// ── Git Log with file filter snapshot ───────────────────────

#[test]
fn snapshot_git_log_with_file_filter() {
    let mut app = make_test_app(3);
    app.git_log = Some(GitLogState {
        entries: vec![GitLogEntry {
            hash_short: "abc1234".to_string(),
            subject: "Fix parser bug".to_string(),
            author: "Alice".to_string(),
            relative_date: "1 hour ago".to_string(),
        }],
        selected: 0,
        scroll_offset: 0,
        visible_height: 20,
        worktree_path: item_path(&app, 0),
        file_filter: Some("src/parser.rs".to_string()),
        stale: false,
    });
    app.main_view = MainView::GitLog;
    app.panel_focus = PanelFocus::Right;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 15);
    insta::assert_snapshot!("git_log_with_file_filter", output);
}

// ── Help overlay for Git Blame ──────────────────────────────

#[test]
fn snapshot_help_overlay_git_blame() {
    let mut app = make_test_app(2);
    app.show_help = true;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::GitBlame;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("help_overlay_git_blame", output);
}

// ── Help overlay for Git Log ────────────────────────────────

#[test]
fn snapshot_help_overlay_git_log() {
    let mut app = make_test_app(2);
    app.show_help = true;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::GitLog;

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("help_overlay_git_log", output);
}

// ── Command Palette snapshots ───────────────────────────────

#[test]
fn snapshot_command_palette_open() {
    let mut app = make_test_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("command_palette_open", output);
}

#[test]
fn snapshot_command_palette_filtered() {
    let mut app = make_test_app(3);
    let mut palette = CommandPaletteState::new(&app.keybindings);
    palette.input = "git".to_string();
    palette.update_matches();
    app.command_palette = Some(palette);

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("command_palette_filtered", output);
}

// ── Planet theme snapshots ──────────────────────────────────

#[test]
fn snapshot_theme_picker_overlay() {
    let mut app = make_test_app(3);
    let planet = PlanetKind::Earth;
    app.planet_animation = Some(PlanetAnimation::load(planet));
    app.prompt = Some(Prompt::ThemePicker {
        selected: 0,
        previous_theme: app.theme.clone(),
    });

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 35);
    insta::assert_snapshot!("theme_picker_overlay", output);
}

#[test]
fn snapshot_sidebar_with_planet() {
    let mut app = make_test_app(3);
    let planet = PlanetKind::Earth;
    app.planet_kind = Some(planet);
    app.planet_animation = Some(PlanetAnimation::load(planet));
    // Apply earth theme
    app.theme = planet.dark_theme();

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("sidebar_with_planet", output);
}

#[test]
fn snapshot_mars_theme() {
    let mut app = make_test_app(3);
    let planet = PlanetKind::Mars;
    app.planet_kind = Some(planet);
    app.planet_animation = Some(PlanetAnimation::load(planet));
    app.theme = planet.dark_theme();

    let sm = make_session_manager();
    let output = render_to_string(&mut app, &sm, 100, 30);
    insta::assert_snapshot!("mars_theme", output);
}
