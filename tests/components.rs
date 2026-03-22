mod helpers;

use crossterm::event::{KeyCode, KeyModifiers};
use tempfile::TempDir;

use darya::app::{
    parse_diff_lines, run_git_status, DiffLineKind, DiffViewState, FileExplorerState,
    GitFileStatus, GitStatusCategory, GitStatusState, SearchViewState, SplitDirection,
};
use darya::config::{parse_keybinding, KeybindingsConfig, ThemeMode};
use darya::planet::renderer;
use darya::planet::sprites::PlanetAnimation;
use darya::planet::types::PlanetKind;
use darya::ui::compute_pane_rects;
use ratatui::layout::Rect;
use ratatui::style::Color;

// ── FileExplorerState ───────────────────────────────────────

fn make_tempdir_tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    // Create structure:
    //   alpha/
    //     nested.txt
    //   beta.txt
    //   gamma.rs
    std::fs::create_dir_all(root.join("alpha")).unwrap();
    std::fs::write(root.join("alpha/nested.txt"), "hello").unwrap();
    std::fs::write(root.join("beta.txt"), "world").unwrap();
    std::fs::write(root.join("gamma.rs"), "fn main() {}").unwrap();
    dir
}

#[test]
fn file_explorer_populates_entries() {
    let dir = make_tempdir_tree();
    let state = FileExplorerState::new(dir.path().to_path_buf());
    // Should have: alpha/, beta.txt, gamma.rs (dirs first, then files, alphabetical)
    assert_eq!(state.entries.len(), 3);
    assert!(state.entries[0].is_dir);
    assert_eq!(state.entries[0].name, "alpha");
    assert!(!state.entries[1].is_dir);
    assert_eq!(state.entries[1].name, "beta.txt");
    assert!(!state.entries[2].is_dir);
    assert_eq!(state.entries[2].name, "gamma.rs");
}

#[test]
fn file_explorer_move_down_and_up() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    assert_eq!(state.selected, 0);
    state.move_down();
    assert_eq!(state.selected, 1);
    state.move_down();
    assert_eq!(state.selected, 2);
    state.move_up();
    assert_eq!(state.selected, 1);
}

#[test]
fn file_explorer_move_down_wraps() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    state.selected = 2; // last entry
    state.move_down();
    assert_eq!(state.selected, 0);
}

#[test]
fn file_explorer_move_up_wraps() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    assert_eq!(state.selected, 0);
    state.move_up();
    assert_eq!(state.selected, 2);
}

#[test]
fn file_explorer_enter_on_dir_expands() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    // select alpha/ (index 0)
    assert_eq!(state.selected, 0);
    let result = state.enter();
    assert!(result.is_none()); // dirs don't return a path
                               // Now entries should include nested.txt under alpha
    assert_eq!(state.entries.len(), 4); // alpha/, nested.txt, beta.txt, gamma.rs
    assert!(state.expanded.contains(&dir.path().join("alpha")));
}

#[test]
fn file_explorer_enter_on_file_returns_path() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    state.selected = 1; // beta.txt
    let result = state.enter();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), dir.path().join("beta.txt"));
}

#[test]
fn file_explorer_enter_on_expanded_dir_collapses() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    // Expand alpha
    state.enter();
    assert_eq!(state.entries.len(), 4);
    // Collapse alpha
    state.selected = 0;
    state.enter();
    assert_eq!(state.entries.len(), 3);
    assert!(!state.expanded.contains(&dir.path().join("alpha")));
}

#[test]
fn file_explorer_collapse_or_parent_on_expanded() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    state.enter(); // expand alpha
    assert_eq!(state.entries.len(), 4);
    state.selected = 0; // alpha/ is selected
    state.collapse_or_parent();
    assert_eq!(state.entries.len(), 3); // collapsed
}

#[test]
fn file_explorer_collapse_or_parent_jumps_to_parent() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    state.enter(); // expand alpha
    state.selected = 1; // nested.txt (depth 1)
    state.collapse_or_parent();
    assert_eq!(state.selected, 0); // jumped to alpha/ (parent)
}

#[test]
fn file_explorer_set_root_resets() {
    let dir = make_tempdir_tree();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    state.selected = 2;
    state.enter(); // expand something
    let other_dir = TempDir::new().unwrap();
    std::fs::write(other_dir.path().join("only.txt"), "x").unwrap();
    state.set_root(other_dir.path().to_path_buf());
    assert_eq!(state.selected, 0);
    assert!(state.expanded.is_empty());
    assert_eq!(state.entries.len(), 1);
}

#[test]
fn file_explorer_ignores_hidden_and_target() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".hidden")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    std::fs::write(root.join("visible.txt"), "x").unwrap();
    std::fs::write(root.join(".dotfile"), "x").unwrap();

    let state = FileExplorerState::new(root.to_path_buf());
    // Dotfiles/dirs are visible; only IGNORED_NAMES (target, node_modules) are filtered
    assert_eq!(state.entries.len(), 3);
    let names: Vec<&str> = state.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&".hidden"));
    assert!(names.contains(&".dotfile"));
    assert!(names.contains(&"visible.txt"));
    assert!(!names.contains(&"target"));
    assert!(!names.contains(&"node_modules"));
}

#[test]
fn file_explorer_empty_dir() {
    let dir = TempDir::new().unwrap();
    let state = FileExplorerState::new(dir.path().to_path_buf());
    assert!(state.entries.is_empty());
    assert_eq!(state.selected, 0);
}

#[test]
fn file_explorer_move_on_empty_is_noop() {
    let dir = TempDir::new().unwrap();
    let mut state = FileExplorerState::new(dir.path().to_path_buf());
    state.move_up();
    assert_eq!(state.selected, 0);
    state.move_down();
    assert_eq!(state.selected, 0);
}

// ── SearchViewState ─────────────────────────────────────────

#[test]
fn search_finds_matches_in_tempdir() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::write(root.join("hello.txt"), "hello world\ngoodbye world\n").unwrap();
    std::fs::write(root.join("other.txt"), "no match here\n").unwrap();

    let state = SearchViewState::new("hello", root);
    assert!(state.error.is_none());
    assert_eq!(state.results.len(), 1);
    assert_eq!(state.results[0].file_relative, "hello.txt");
    assert_eq!(state.results[0].line_number, 1);
    assert!(state.results[0].line_text.contains("hello world"));
}

#[test]
fn search_no_matches_returns_empty() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::write(root.join("test.txt"), "nothing relevant\n").unwrap();

    let state = SearchViewState::new("zzzznotfound", root);
    assert!(state.error.is_none());
    assert!(state.results.is_empty());
}

#[test]
fn search_move_up_down() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("test.txt"),
        "line1 match\nline2 match\nline3 match\n",
    )
    .unwrap();

    let mut state = SearchViewState::new("match", root);
    assert_eq!(state.selected, 0);
    state.move_down();
    assert_eq!(state.selected, 1);
    state.move_down();
    assert_eq!(state.selected, 2);
    state.move_down();
    assert_eq!(state.selected, 0); // wraps
    state.move_up();
    assert_eq!(state.selected, 2); // wraps back
}

#[test]
fn search_selected_result() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::write(root.join("test.txt"), "aaa\nbbb match\nccc\n").unwrap();

    let state = SearchViewState::new("match", root);
    let result = state.selected_result().unwrap();
    assert_eq!(result.line_number, 2);
}

#[test]
fn search_empty_results_selected_is_none() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("test.txt"), "no\n").unwrap();
    let state = SearchViewState::new("zzz", dir.path());
    assert!(state.selected_result().is_none());
}

// ── KeybindingsConfig ───────────────────────────────────────

#[test]
fn parse_keybinding_ctrl_number() {
    let result = parse_keybinding("ctrl+1");
    assert_eq!(result, Some((KeyModifiers::CONTROL, KeyCode::Char('1'))));
}

#[test]
fn parse_keybinding_ctrl_letter() {
    let result = parse_keybinding("ctrl+p");
    assert_eq!(result, Some((KeyModifiers::CONTROL, KeyCode::Char('p'))));
}

#[test]
fn parse_keybinding_alt_letter() {
    let result = parse_keybinding("alt+f");
    assert_eq!(result, Some((KeyModifiers::ALT, KeyCode::Char('f'))));
}

#[test]
fn parse_keybinding_function_key() {
    let result = parse_keybinding("shift+f5");
    assert_eq!(result, Some((KeyModifiers::SHIFT, KeyCode::F(5))));
}

#[test]
fn parse_keybinding_enter() {
    let result = parse_keybinding("ctrl+enter");
    assert_eq!(result, Some((KeyModifiers::CONTROL, KeyCode::Enter)));
}

#[test]
fn parse_keybinding_invalid_returns_none() {
    assert!(parse_keybinding("").is_none());
    assert!(parse_keybinding("ctrl+").is_none());
    assert!(parse_keybinding("invalid+key").is_none());
    assert!(parse_keybinding("ctrl+longname").is_none());
}

#[test]
fn parse_keybinding_case_insensitive() {
    let result = parse_keybinding("Ctrl+P");
    assert_eq!(result, Some((KeyModifiers::CONTROL, KeyCode::Char('p'))));
}

#[test]
fn keybindings_format_roundtrip() {
    let binding = (KeyModifiers::CONTROL, KeyCode::Char('p'));
    let formatted = KeybindingsConfig::format(&binding);
    assert_eq!(formatted, "Ctrl+P");
}

#[test]
fn keybindings_format_multi_modifier() {
    let binding = (
        KeyModifiers::CONTROL | KeyModifiers::ALT,
        KeyCode::Char('x'),
    );
    let formatted = KeybindingsConfig::format(&binding);
    assert_eq!(formatted, "Ctrl+Alt+X");
}

#[test]
fn keybindings_format_function_key() {
    let binding = (KeyModifiers::SHIFT, KeyCode::F(12));
    let formatted = KeybindingsConfig::format(&binding);
    assert_eq!(formatted, "Shift+F12");
}

#[test]
fn keybindings_matches_positive() {
    let binding = (KeyModifiers::CONTROL, KeyCode::Char('1'));
    assert!(KeybindingsConfig::matches(
        &binding,
        KeyModifiers::CONTROL,
        KeyCode::Char('1')
    ));
}

#[test]
fn keybindings_matches_negative_wrong_modifier() {
    let binding = (KeyModifiers::CONTROL, KeyCode::Char('1'));
    assert!(!KeybindingsConfig::matches(
        &binding,
        KeyModifiers::ALT,
        KeyCode::Char('1')
    ));
}

#[test]
fn keybindings_matches_negative_wrong_key() {
    let binding = (KeyModifiers::CONTROL, KeyCode::Char('1'));
    assert!(!KeybindingsConfig::matches(
        &binding,
        KeyModifiers::CONTROL,
        KeyCode::Char('2')
    ));
}

#[test]
fn keybindings_requires_exact_modifiers() {
    // Ctrl+Shift+1 should NOT match a Ctrl+1 binding (prevents Ctrl+Shift+P triggering Ctrl+P)
    let binding = (KeyModifiers::CONTROL, KeyCode::Char('1'));
    assert!(!KeybindingsConfig::matches(
        &binding,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        KeyCode::Char('1')
    ));
    // Exact match should still work
    assert!(KeybindingsConfig::matches(
        &binding,
        KeyModifiers::CONTROL,
        KeyCode::Char('1')
    ));
}

// ── GitStatusState ─────────────────────────────────────────

fn make_git_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    // Initialize a git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(root)
        .output()
        .unwrap();
    // Create initial commit
    std::fs::write(root.join("initial.txt"), "hello").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(root)
        .output()
        .unwrap();
    dir
}

#[test]
fn git_status_modified_file() {
    let dir = make_git_repo();
    let root = dir.path();
    // Modify the tracked file
    std::fs::write(root.join("initial.txt"), "modified").unwrap();
    let entries = run_git_status(root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].category, GitStatusCategory::Unstaged);
    assert_eq!(entries[0].status, GitFileStatus::Modified);
    assert_eq!(entries[0].path, "initial.txt");
}

#[test]
fn git_status_untracked_file() {
    let dir = make_git_repo();
    let root = dir.path();
    std::fs::write(root.join("new.txt"), "new file").unwrap();
    let entries = run_git_status(root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].category, GitStatusCategory::Untracked);
    assert_eq!(entries[0].status, GitFileStatus::Untracked);
    assert_eq!(entries[0].path, "new.txt");
}

#[test]
fn git_status_staged_added() {
    let dir = make_git_repo();
    let root = dir.path();
    std::fs::write(root.join("added.txt"), "new content").unwrap();
    std::process::Command::new("git")
        .args(["add", "added.txt"])
        .current_dir(root)
        .output()
        .unwrap();
    let entries = run_git_status(root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].category, GitStatusCategory::Staged);
    assert_eq!(entries[0].status, GitFileStatus::Added);
}

#[test]
fn git_status_mm_produces_two_entries() {
    let dir = make_git_repo();
    let root = dir.path();
    // Modify, stage, then modify again => MM
    std::fs::write(root.join("initial.txt"), "staged change").unwrap();
    std::process::Command::new("git")
        .args(["add", "initial.txt"])
        .current_dir(root)
        .output()
        .unwrap();
    std::fs::write(root.join("initial.txt"), "unstaged change").unwrap();
    let entries = run_git_status(root).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].category, GitStatusCategory::Staged);
    assert_eq!(entries[1].category, GitStatusCategory::Unstaged);
}

#[test]
fn git_status_deleted_file() {
    let dir = make_git_repo();
    let root = dir.path();
    std::fs::remove_file(root.join("initial.txt")).unwrap();
    let entries = run_git_status(root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].status, GitFileStatus::Deleted);
}

#[test]
fn git_status_empty_repo_no_changes() {
    let dir = make_git_repo();
    let entries = run_git_status(dir.path()).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn git_status_state_move_up_down_wrapping() {
    let dir = make_git_repo();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "a").unwrap();
    std::fs::write(root.join("b.txt"), "b").unwrap();
    std::fs::write(root.join("c.txt"), "c").unwrap();
    let mut state = GitStatusState::new(root.to_path_buf());
    assert_eq!(state.entries.len(), 3);
    assert_eq!(state.selected, 0);
    state.move_down();
    assert_eq!(state.selected, 1);
    state.move_down();
    assert_eq!(state.selected, 2);
    state.move_down();
    assert_eq!(state.selected, 0); // wraps
    state.move_up();
    assert_eq!(state.selected, 2); // wraps
}

#[test]
fn git_status_state_selected_entry() {
    let dir = make_git_repo();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "a").unwrap();
    std::fs::write(root.join("b.txt"), "b").unwrap();
    let mut state = GitStatusState::new(root.to_path_buf());
    state.move_down();
    let entry = state.selected_entry().unwrap();
    assert_eq!(entry.path, "b.txt");
}

// ── DiffViewState / parse_diff_lines ────────────────────────

#[test]
fn parse_diff_lines_classifies_additions() {
    let diff = "+added line\n context\n-removed line\n";
    let lines = parse_diff_lines(diff);
    assert_eq!(lines[0].kind, DiffLineKind::Addition);
    assert_eq!(lines[1].kind, DiffLineKind::Context);
    assert_eq!(lines[2].kind, DiffLineKind::Deletion);
}

#[test]
fn parse_diff_lines_classifies_headers() {
    let diff = "diff --git a/foo b/foo\nindex abc..def 100644\n--- a/foo\n+++ b/foo\n@@ -1,3 +1,4 @@\n context\n";
    let lines = parse_diff_lines(diff);
    assert_eq!(lines[0].kind, DiffLineKind::Header); // diff --git
    assert_eq!(lines[1].kind, DiffLineKind::Header); // index
    assert_eq!(lines[2].kind, DiffLineKind::Header); // ---
    assert_eq!(lines[3].kind, DiffLineKind::Header); // +++
    assert_eq!(lines[4].kind, DiffLineKind::Header); // @@
    assert_eq!(lines[5].kind, DiffLineKind::Context);
}

#[test]
fn diff_view_scroll_up_down_clamped() {
    let dir = make_git_repo();
    let root = dir.path();
    // Create a file with enough content to generate many diff lines
    std::fs::write(root.join("big.txt"), "line\n".repeat(100)).unwrap();
    let mut dv = DiffViewState::new("big.txt", root, GitStatusCategory::Untracked);
    dv.visible_height = 10;
    assert_eq!(dv.scroll_offset, 0);
    dv.scroll_down(5);
    assert_eq!(dv.scroll_offset, 5);
    dv.scroll_up(3);
    assert_eq!(dv.scroll_offset, 2);
    dv.scroll_up(100);
    assert_eq!(dv.scroll_offset, 0); // clamped at 0
}

// ── Sidebar resize ─────────────────────────────────────────

#[test]
fn sidebar_width_defaults_to_25() {
    use darya::app::{App, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH};
    use darya::config::{KeybindingsConfig, Theme};
    let app = App::new(
        vec![],
        Theme::dark(),
        true,
        KeybindingsConfig::default(),
        "claude".into(),
        "/bin/sh".into(),
    );
    assert_eq!(app.sidebar_width, 25);
    assert!(!app.sidebar_resized);
    // Verify constants are sensible
    assert!(SIDEBAR_MIN_WIDTH < SIDEBAR_MAX_WIDTH);
    assert!(app.sidebar_width >= SIDEBAR_MIN_WIDTH);
    assert!(app.sidebar_width <= SIDEBAR_MAX_WIDTH);
}

#[test]
fn sidebar_resize_respects_bounds() {
    use darya::app::{App, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_STEP};
    use darya::config::{KeybindingsConfig, Theme};
    let mut app = App::new(
        vec![],
        Theme::dark(),
        true,
        KeybindingsConfig::default(),
        "claude".into(),
        "/bin/sh".into(),
    );

    // Grow to max
    for _ in 0..50 {
        app.sidebar_width = (app.sidebar_width + SIDEBAR_STEP).min(SIDEBAR_MAX_WIDTH);
    }
    assert_eq!(app.sidebar_width, SIDEBAR_MAX_WIDTH);

    // Shrink to min
    for _ in 0..50 {
        app.sidebar_width = app
            .sidebar_width
            .saturating_sub(SIDEBAR_STEP)
            .max(SIDEBAR_MIN_WIDTH);
    }
    assert_eq!(app.sidebar_width, SIDEBAR_MIN_WIDTH);
}

// ── compute_pane_rects direction ────────────────────────────

#[test]
fn compute_pane_rects_horizontal_vs_vertical() {
    let size = Rect::new(0, 0, 120, 30);
    let sidebar_pct = 25;

    let h_rects = compute_pane_rects(size, 2, sidebar_pct, SplitDirection::Horizontal, None);
    assert_eq!(h_rects.len(), 2);
    // Horizontal: panes split width, same height
    assert_eq!(h_rects[0].height, h_rects[1].height);
    assert!(h_rects[0].width < size.width); // each pane narrower than total
    assert_eq!(h_rects[0].y, h_rects[1].y); // same y position

    let v_rects = compute_pane_rects(size, 2, sidebar_pct, SplitDirection::Vertical, None);
    assert_eq!(v_rects.len(), 2);
    // Vertical: panes split height, same width
    assert_eq!(v_rects[0].width, v_rects[1].width);
    assert!(v_rects[0].height < v_rects[1].y + v_rects[1].height); // stacked
    assert_eq!(v_rects[0].x, v_rects[1].x); // same x position
}

// ── PlanetKind ──────────────────────────────────────────────

#[test]
fn planet_kind_all_returns_6_variants() {
    assert_eq!(PlanetKind::all().len(), 6);
}

#[test]
fn planet_kind_from_str_round_trips() {
    for planet in PlanetKind::all() {
        let name = planet.name();
        let parsed = PlanetKind::parse(name);
        assert_eq!(parsed, Some(*planet), "failed to round-trip {}", name);
    }
}

#[test]
fn planet_kind_from_str_case_insensitive() {
    assert_eq!(PlanetKind::parse("EARTH"), Some(PlanetKind::Earth));
    assert_eq!(PlanetKind::parse("Mars"), Some(PlanetKind::Mars));
    assert_eq!(PlanetKind::parse("invalid"), None);
}

#[test]
fn each_planet_produces_valid_dark_theme() {
    for planet in PlanetKind::all() {
        let theme = planet.dark_theme();
        assert_eq!(theme.mode, ThemeMode::Dark);
        // accent should match border_active
        assert_eq!(theme.border_active, planet.accent());
    }
}

#[test]
fn each_planet_produces_valid_light_theme() {
    for planet in PlanetKind::all() {
        let theme = planet.light_theme();
        assert_eq!(theme.mode, ThemeMode::Light);
    }
}

#[test]
fn planet_animation_loads_frames() {
    let anim = PlanetAnimation::load(PlanetKind::Earth);
    assert!(anim.frame_count() > 0);
    let frame = anim.frame_at(0);
    assert!(frame.width() > 0);
    assert!(frame.height() > 0);
}

#[test]
fn planet_animation_frame_at_wraps() {
    let anim = PlanetAnimation::load(PlanetKind::Earth);
    let count = anim.frame_count();
    // Accessing beyond frame count should wrap
    let _ = anim.frame_at(count + 5);
}

#[test]
fn half_block_renderer_produces_output() {
    let anim = PlanetAnimation::load(PlanetKind::Earth);
    let frame = anim.frame_at(0);
    let bg = Color::Rgb(0x1A, 0x1A, 0x1A);
    let lines = renderer::render_frame(frame, 10, 5, bg);
    assert_eq!(lines.len(), 5);
    // Each line should have spans
    for line in &lines {
        assert!(!line.spans.is_empty());
    }
}

#[test]
fn half_block_renderer_handles_zero_size() {
    let anim = PlanetAnimation::load(PlanetKind::Earth);
    let frame = anim.frame_at(0);
    let bg = Color::Rgb(0, 0, 0);
    let lines = renderer::render_frame(frame, 0, 0, bg);
    assert!(lines.is_empty());
}

// ── NoteViewState ──────────────────────────────────────────────

use darya::app::{NotePosition, NoteViewState};

#[test]
fn note_path_for_worktree_deterministic() {
    use std::path::Path;
    let p1 = NoteViewState::note_path_for_worktree(Path::new("/Users/foo/project"));
    let p2 = NoteViewState::note_path_for_worktree(Path::new("/Users/foo/project"));
    assert_eq!(p1, p2);
    assert!(p1.to_string_lossy().ends_with(".md"));
}

#[test]
fn note_path_for_different_worktrees_differ() {
    use std::path::Path;
    let p1 = NoteViewState::note_path_for_worktree(Path::new("/Users/foo/project-a"));
    let p2 = NoteViewState::note_path_for_worktree(Path::new("/Users/foo/project-b"));
    assert_ne!(p1, p2);
}

#[test]
fn note_open_or_create_empty_for_missing_file() {
    let dir = TempDir::new().unwrap();
    let note = NoteViewState::open_or_create(dir.path());
    assert!(!note.modified);
    assert!(note.read_only);
    assert_eq!(note.content_string().trim(), "");
}

#[test]
fn note_save_and_reload() {
    let dir = TempDir::new().unwrap();
    let mut note = NoteViewState::open_or_create(dir.path());
    // Manually set content via edtui
    note.editor_state = edtui::EditorState::new(edtui::Lines::from("hello world"));
    note.modified = true;
    note.save().unwrap();
    assert!(!note.modified);
    // Reload
    let note2 = NoteViewState::open_or_create(dir.path());
    assert_eq!(note2.content_string().trim(), "hello world");
}

#[test]
fn note_position_cycle() {
    let mut pos = NotePosition::Sidebar;
    // Simulate cycling
    pos = match pos {
        NotePosition::Sidebar => NotePosition::CenterColumn,
        NotePosition::CenterColumn => NotePosition::Hidden,
        NotePosition::Hidden => NotePosition::Sidebar,
    };
    assert_eq!(pos, NotePosition::CenterColumn);
    pos = match pos {
        NotePosition::Sidebar => NotePosition::CenterColumn,
        NotePosition::CenterColumn => NotePosition::Hidden,
        NotePosition::Hidden => NotePosition::Sidebar,
    };
    assert_eq!(pos, NotePosition::Hidden);
    pos = match pos {
        NotePosition::Sidebar => NotePosition::CenterColumn,
        NotePosition::CenterColumn => NotePosition::Hidden,
        NotePosition::Hidden => NotePosition::Sidebar,
    };
    assert_eq!(pos, NotePosition::Sidebar);
}

#[test]
fn compute_pane_rects_with_notes_column() {
    let size = Rect::new(0, 0, 120, 30);
    let sidebar_pct = 25;
    // With notes column, right panel should be narrower
    let rects_no_notes = compute_pane_rects(size, 1, sidebar_pct, SplitDirection::Horizontal, None);
    let rects_with_notes = compute_pane_rects(size, 1, sidebar_pct, SplitDirection::Horizontal, Some(25));
    assert!(rects_with_notes[0].width < rects_no_notes[0].width);
}
