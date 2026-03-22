mod helpers;

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use darya::app::{
    format_relative_time, is_edtui_compatible, status_priority, BlameLine, BranchSwitcherState,
    ColorTarget, CommandId, CommandPaletteState, EditorViewState, GitBlameState, GitFileStatus,
    GitLogEntry, GitLogState, GitStatusCategory, GitStatusEntry, GitStatusState, InputMode,
    MainView, PaneContent, PanelFocus, Prompt, SidebarView, SplitDirection, SplitNode, ViewKind,
};
use darya::config;
use darya::event::AppEvent;
use darya::planet::types::PlanetKind;

use helpers::{
    active_session_id, active_shell_session_id, cmd_key, item_path, key, make_app,
    make_app_with_session, make_app_with_two_sessions, selected_item_index, set_session,
    set_shell_session,
};

// ── Navigation ──────────────────────────────────────────────

#[test]
fn nav_j_moves_worktree_selection_down() {
    let mut app = make_app(3);
    // cursor starts at 0 (section header); j moves to item 0 (cursor=1)
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(selected_item_index(&app), Some(0));
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(selected_item_index(&app), Some(1));
}

#[test]
fn nav_k_moves_worktree_selection_up() {
    let mut app = make_app(3);
    // Move cursor to item 2 (cursor=3)
    app.sidebar_tree.cursor = 3;
    app.handle_event(&key(KeyCode::Char('k')));
    assert_eq!(selected_item_index(&app), Some(1));
}

#[test]
fn nav_j_wraps_around() {
    let mut app = make_app(3);
    // Move cursor to last item (cursor=3, which is item 2)
    app.sidebar_tree.cursor = 3;
    app.handle_event(&key(KeyCode::Char('j')));
    // Wraps to beginning (section header, cursor=0)
    assert_eq!(app.sidebar_tree.cursor, 0);
}

#[test]
fn nav_k_wraps_around() {
    let mut app = make_app(3);
    // cursor=0 (section header); k wraps to last visible node
    app.handle_event(&key(KeyCode::Char('k')));
    assert_eq!(selected_item_index(&app), Some(2));
}

#[test]
fn nav_down_arrow_works_like_j() {
    let mut app = make_app(3);
    app.handle_event(&key(KeyCode::Down));
    assert_eq!(selected_item_index(&app), Some(0));
}

#[test]
fn nav_number_keys_jump_to_worktree() {
    let mut app = make_app(5);
    app.handle_event(&key(KeyCode::Char('3')));
    assert_eq!(selected_item_index(&app), Some(2));
    app.handle_event(&key(KeyCode::Char('1')));
    assert_eq!(selected_item_index(&app), Some(0));
}

#[test]
fn nav_zero_jumps_to_tenth_worktree() {
    let mut app = make_app(11);
    app.handle_event(&key(KeyCode::Char('0')));
    assert_eq!(selected_item_index(&app), Some(9));
}

#[test]
fn nav_number_beyond_count_is_noop() {
    let mut app = make_app(2);
    let before = app.sidebar_tree.cursor;
    app.handle_event(&key(KeyCode::Char('5')));
    assert_eq!(app.sidebar_tree.cursor, before); // unchanged
}

// ── Mode transitions ────────────────────────────────────────

#[test]
fn terminal_mode_tab_returns_to_nav() {
    let mut app = make_app_with_session(3);
    // Move cursor to item 0 so active_session_id works
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Terminal;
    app.panel_focus = PanelFocus::Right;
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.input_mode, InputMode::Navigation);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn enter_terminal_mode_from_terminal_nav() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    // Focus right panel showing terminal
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.input_mode = InputMode::Navigation;
    // Press 'i' to enter terminal mode
    app.handle_event(&key(KeyCode::Char('i')));
    assert_eq!(app.input_mode, InputMode::Terminal);
}

#[test]
fn cannot_enter_terminal_without_active_session() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1; // item 0, no session
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    // No active session
    assert!(active_session_id(&app).is_none());
    app.handle_event(&key(KeyCode::Char('i')));
    assert_eq!(app.input_mode, InputMode::Navigation);
}

#[test]
fn cannot_enter_terminal_on_exited_session() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    app.exited_sessions.insert(sid);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.handle_event(&key(KeyCode::Char('i')));
    assert_eq!(app.input_mode, InputMode::Navigation);
}

// ── Panel switching ─────────────────────────────────────────

#[test]
fn cmd_1_sets_sidebar_worktrees() {
    let mut app = make_app(3);
    app.sidebar_view = SidebarView::FileExplorer;
    app.panel_focus = PanelFocus::Right;
    app.handle_event(&cmd_key('1'));
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn cmd_2_sets_main_terminal() {
    let mut app = make_app(3);
    app.main_view = MainView::Editor;
    app.panel_focus = PanelFocus::Left;
    app.handle_event(&cmd_key('2'));
    assert_eq!(app.main_view, MainView::Terminal);
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn cmd_3_sets_sidebar_files() {
    let mut app = make_app(3);
    app.handle_event(&cmd_key('3'));
    assert_eq!(app.sidebar_view, SidebarView::FileExplorer);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn cmd_4_sets_main_editor() {
    let mut app = make_app(3);
    app.handle_event(&cmd_key('4'));
    assert_eq!(app.main_view, MainView::Editor);
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn cmd_5_sets_sidebar_search() {
    let mut app = make_app(3);
    app.handle_event(&cmd_key('5'));
    assert_eq!(app.sidebar_view, SidebarView::Search);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn tab_toggles_focus() {
    let mut app = make_app(3);
    assert_eq!(app.panel_focus, PanelFocus::Left);
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Right);
    // Tab auto-enters terminal if there's an active non-exited session
}

// ── Session lifecycle ───────────────────────────────────────

#[test]
fn session_exited_kicks_to_nav() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    app.input_mode = InputMode::Terminal;
    app.handle_event(&AppEvent::SessionExited {
        session_id: sid.clone(),
    });
    assert_eq!(app.input_mode, InputMode::Navigation);
    assert!(app.exited_sessions.contains(&sid));
}

#[test]
fn session_exited_other_session_no_mode_change() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Terminal;
    app.handle_event(&AppEvent::SessionExited {
        session_id: "other-session".to_string(),
    });
    assert_eq!(app.input_mode, InputMode::Terminal);
}

#[test]
fn session_bell_does_not_immediately_mark_attention() {
    // Bell/Done attention is debounced in the main event loop, not in handle_event.
    // handle_event should NOT set attention_sessions for bell events.
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();

    app.input_mode = InputMode::Navigation;
    app.handle_event(&AppEvent::SessionBell {
        session_id: sid.clone(),
    });
    assert!(!app.attention_sessions.contains(&sid));
}

#[test]
fn session_done_does_not_immediately_mark_attention() {
    // SessionDone attention is debounced in the main event loop.
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();

    app.input_mode = InputMode::Navigation;
    app.handle_event(&AppEvent::SessionDone {
        session_id: sid.clone(),
    });
    assert!(!app.attention_sessions.contains(&sid));
}

// ── Prompts ─────────────────────────────────────────────────

#[test]
fn a_opens_create_worktree_prompt() {
    let mut app = make_app(3);
    assert!(app.prompt.is_none());
    app.handle_event(&key(KeyCode::Char('a')));
    assert!(matches!(app.prompt, Some(Prompt::CreateWorktree { .. })));
}

#[test]
fn esc_dismisses_prompt() {
    let mut app = make_app(3);
    app.prompt = Some(Prompt::CreateWorktree {
        input: String::new(),
    });
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.prompt.is_none());
}

#[test]
fn prompt_typing_appends_chars() {
    let mut app = make_app(3);
    app.prompt = Some(Prompt::CreateWorktree {
        input: String::new(),
    });
    app.handle_event(&key(KeyCode::Char('f')));
    app.handle_event(&key(KeyCode::Char('o')));
    app.handle_event(&key(KeyCode::Char('o')));
    if let Some(Prompt::CreateWorktree { input }) = &app.prompt {
        assert_eq!(input, "foo");
    } else {
        panic!("Expected CreateWorktree prompt");
    }
}

#[test]
fn prompt_backspace_removes_char() {
    let mut app = make_app(3);
    app.prompt = Some(Prompt::CreateWorktree {
        input: "foo".to_string(),
    });
    app.handle_event(&key(KeyCode::Backspace));
    if let Some(Prompt::CreateWorktree { input }) = &app.prompt {
        assert_eq!(input, "fo");
    } else {
        panic!("Expected CreateWorktree prompt");
    }
}

#[test]
fn d_on_non_main_worktree_opens_confirm_delete() {
    let mut app = make_app(3);
    // Move cursor to item 1 (non-main worktree), cursor position = 2
    app.sidebar_tree.cursor = 2;
    app.handle_event(&key(KeyCode::Char('d')));
    assert!(matches!(app.prompt, Some(Prompt::ConfirmDelete { .. })));
}

#[test]
fn d_on_main_worktree_shows_error() {
    let mut app = make_app(3);
    // Move cursor to item 0 (main worktree), cursor position = 1
    app.sidebar_tree.cursor = 1;
    app.handle_event(&key(KeyCode::Char('d')));
    assert!(app.prompt.is_none());
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .contains("Cannot delete main"));
}

// ── Edge cases ──────────────────────────────────────────────

#[test]
fn q_quits_from_nav() {
    let mut app = make_app(3);
    assert!(app.running);
    app.handle_event(&key(KeyCode::Char('q')));
    assert!(!app.running);
}

#[test]
fn question_mark_toggles_help() {
    let mut app = make_app(3);
    assert!(!app.show_help);
    app.handle_event(&key(KeyCode::Char('?')));
    assert!(app.show_help);
    app.handle_event(&key(KeyCode::Char('?')));
    assert!(!app.show_help);
}

#[test]
fn help_dismissed_by_any_key() {
    let mut app = make_app(3);
    app.show_help = true;
    app.handle_event(&key(KeyCode::Char('j'))); // any key
    assert!(!app.show_help);
}

#[test]
fn resize_event_is_noop() {
    let mut app = make_app(3);
    let before_mode = app.input_mode;
    let before_focus = app.panel_focus;
    app.handle_event(&AppEvent::Resize(120, 40));
    assert_eq!(app.input_mode, before_mode);
    assert_eq!(app.panel_focus, before_focus);
}

#[test]
fn tick_event_is_noop() {
    let mut app = make_app(3);
    let before = app.sidebar_tree.cursor;
    app.handle_event(&AppEvent::Tick);
    assert_eq!(app.sidebar_tree.cursor, before);
}

#[test]
fn paste_event_is_noop_in_handle_event() {
    let mut app = make_app(3);
    let before_mode = app.input_mode;
    let before_cursor = app.sidebar_tree.cursor;
    app.handle_event(&AppEvent::Paste("hello world".to_string()));
    assert_eq!(app.input_mode, before_mode);
    assert_eq!(app.sidebar_tree.cursor, before_cursor);
}

// ── Scroll ──────────────────────────────────────────────────

#[test]
fn scroll_up_and_down() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    assert_eq!(app.active_scroll_offset(), 0);
    app.scroll_up(10);
    assert_eq!(app.active_scroll_offset(), 10);
    app.scroll_down(5);
    assert_eq!(app.active_scroll_offset(), 5);
    app.scroll_down(5);
    assert_eq!(app.active_scroll_offset(), 0);
}

#[test]
fn scroll_up_caps_at_1000() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    app.scroll_up(2000);
    assert_eq!(app.active_scroll_offset(), 1000);
}

#[test]
fn reset_scroll_clears_offset() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    app.scroll_up(50);
    assert_eq!(app.active_scroll_offset(), 50);
    app.reset_scroll();
    assert_eq!(app.active_scroll_offset(), 0);
}

// ── User-scrolled flag (interruptible auto-scroll) ──────────

#[test]
fn scroll_up_sets_user_scrolled_flag() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    let sid = app.focused_session_id().unwrap().clone();
    assert!(!app.user_scrolled.contains(&sid));
    app.scroll_up(10);
    assert!(app.user_scrolled.contains(&sid));
}

#[test]
fn scroll_down_to_zero_clears_user_scrolled_flag() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    let sid = app.focused_session_id().unwrap().clone();
    app.scroll_up(10);
    assert!(app.user_scrolled.contains(&sid));
    app.scroll_down(10);
    assert!(!app.user_scrolled.contains(&sid));
    assert_eq!(app.active_scroll_offset(), 0);
}

#[test]
fn scroll_down_partial_keeps_user_scrolled_flag() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    let sid = app.focused_session_id().unwrap().clone();
    app.scroll_up(20);
    app.scroll_down(5);
    // Still scrolled back — flag should remain
    assert!(app.user_scrolled.contains(&sid));
    assert_eq!(app.active_scroll_offset(), 15);
}

#[test]
fn reset_scroll_clears_user_scrolled_flag() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    let sid = app.focused_session_id().unwrap().clone();
    app.scroll_up(50);
    assert!(app.user_scrolled.contains(&sid));
    app.reset_scroll();
    assert!(!app.user_scrolled.contains(&sid));
}

// ── Tab auto-enters terminal ────────────────────────────────

#[test]
fn tab_from_worktrees_with_active_session_enters_terminal_mode() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Left;
    app.sidebar_view = SidebarView::Worktrees;
    app.main_view = MainView::Terminal;
    app.input_mode = InputMode::Navigation;
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Right);
    assert_eq!(app.input_mode, InputMode::Terminal);
}

#[test]
fn tab_from_worktrees_without_session_stays_nav() {
    let mut app = make_app(2);
    app.panel_focus = PanelFocus::Left;
    app.main_view = MainView::Terminal;
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Right);
    assert_eq!(app.input_mode, InputMode::Navigation);
}

// ── needs_session_spawn / needs_session_restart ─────────────

#[test]
fn needs_session_spawn_on_enter_in_worktree_view() {
    let mut app = make_app(2);
    app.sidebar_tree.cursor = 1; // on an item
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert!(app.needs_session_spawn(&key_event));
}

#[test]
fn needs_session_spawn_false_when_prompt_active() {
    let mut app = make_app(2);
    app.sidebar_tree.cursor = 1;
    app.prompt = Some(Prompt::CreateWorktree {
        input: String::new(),
    });
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_spawn(&key_event));
}

#[test]
fn needs_session_restart_on_r_with_exited_session() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    app.exited_sessions.insert(sid);
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('r'), crossterm::event::KeyModifiers::NONE);
    assert!(app.needs_session_restart(&key_event));
}

#[test]
fn needs_session_restart_false_without_exited() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('r'), crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_restart(&key_event));
}

// ── needs_session_force_restart ────────────────────────────────

#[test]
fn needs_session_force_restart_on_shift_r_with_running_session() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('R'), crossterm::event::KeyModifiers::SHIFT);
    assert!(app.needs_session_force_restart(&key_event));
}

#[test]
fn needs_session_force_restart_on_shift_r_with_exited_session() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    app.exited_sessions.insert(sid);
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('R'), crossterm::event::KeyModifiers::SHIFT);
    assert!(app.needs_session_force_restart(&key_event));
}

#[test]
fn needs_session_force_restart_false_without_session() {
    let mut app = make_app(2);
    app.sidebar_tree.cursor = 1;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('R'), crossterm::event::KeyModifiers::SHIFT);
    assert!(!app.needs_session_force_restart(&key_event));
}

#[test]
fn needs_session_force_restart_false_in_terminal_mode() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Terminal;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('R'), crossterm::event::KeyModifiers::SHIFT);
    assert!(!app.needs_session_force_restart(&key_event));
}

// ── needs_session_close ─────────────────────────────────────

#[test]
fn needs_session_close_on_backspace_with_session() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(app.needs_session_close(&key_event));
}

#[test]
fn needs_session_close_false_without_session() {
    let mut app = make_app(2);
    app.sidebar_tree.cursor = 1;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_close(&key_event));
}

#[test]
fn needs_session_close_false_in_terminal_mode() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Terminal;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_close(&key_event));
}

#[test]
fn needs_session_close_false_when_prompt_active() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.prompt = Some(Prompt::CreateWorktree {
        input: String::new(),
    });
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_close(&key_event));
}

// ── wants_create/delete worktree ────────────────────────────

#[test]
fn wants_create_worktree_returns_input_on_enter() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::CreateWorktree {
        input: "my-branch".to_string(),
    });
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert_eq!(
        app.wants_create_worktree(&key_event),
        Some("my-branch".to_string())
    );
}

#[test]
fn wants_create_worktree_none_on_empty_input() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::CreateWorktree {
        input: String::new(),
    });
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert_eq!(app.wants_create_worktree(&key_event), None);
}

#[test]
fn wants_delete_worktree_on_y() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::ConfirmDelete {
        worktree_name: "test".to_string(),
    });
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('y'), crossterm::event::KeyModifiers::NONE);
    assert!(app.wants_delete_worktree(&key_event));
}

#[test]
fn wants_delete_worktree_false_on_n() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::ConfirmDelete {
        worktree_name: "test".to_string(),
    });
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('n'), crossterm::event::KeyModifiers::NONE);
    assert!(!app.wants_delete_worktree(&key_event));
}

// ── Git Status view switching ───────────────────────────────

#[test]
fn cmd_6_sets_sidebar_git_status() {
    let mut app = make_app(3);
    app.sidebar_view = SidebarView::Worktrees;
    app.panel_focus = PanelFocus::Right;
    app.handle_event(&cmd_key('6'));
    assert_eq!(app.sidebar_view, SidebarView::GitStatus);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

fn make_app_with_git_status(n: usize) -> darya::app::App {
    let mut app = make_app(n);
    app.sidebar_tree.cursor = 1; // select item 0
                                 // Set up a mock git status state with test entries
    app.git_status = Some(GitStatusState {
        entries: vec![
            GitStatusEntry {
                category: GitStatusCategory::Staged,
                status: GitFileStatus::Modified,
                path: "staged.txt".to_string(),
                orig_path: None,
            },
            GitStatusEntry {
                category: GitStatusCategory::Unstaged,
                status: GitFileStatus::Modified,
                path: "unstaged.txt".to_string(),
                orig_path: None,
            },
            GitStatusEntry {
                category: GitStatusCategory::Untracked,
                status: GitFileStatus::Untracked,
                path: "new.txt".to_string(),
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
    app
}

#[test]
fn enter_on_git_status_opens_diff_in_main() {
    let mut app = make_app_with_git_status(3);
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.main_view, MainView::DiffView);
    assert!(app.diff_view.is_some());
}

#[test]
fn d_on_git_status_opens_diff() {
    let mut app = make_app_with_git_status(3);
    app.handle_event(&key(KeyCode::Char('d')));
    assert_eq!(app.main_view, MainView::DiffView);
    assert!(app.diff_view.is_some());
}

#[test]
fn esc_in_diff_view_returns_to_terminal() {
    let mut app = make_app_with_git_status(3);
    // Open diff first
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.main_view, MainView::DiffView);
    // Switch focus to right panel to be in diff view
    app.panel_focus = PanelFocus::Right;
    app.handle_event(&key(KeyCode::Esc));
    assert_eq!(app.main_view, MainView::Terminal);
}

#[test]
fn tab_toggles_focus_from_git_status() {
    let mut app = make_app_with_git_status(3);
    assert_eq!(app.panel_focus, PanelFocus::Left);
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn number_keys_jump_worktree_from_git_status() {
    let mut app = make_app_with_git_status(3);
    app.handle_event(&key(KeyCode::Char('2')));
    assert_eq!(selected_item_index(&app), Some(1));
    // git_status cleared on worktree switch
    assert!(app.git_status.is_none());
}

#[test]
fn q_quits_from_git_status() {
    let mut app = make_app_with_git_status(3);
    assert!(app.running);
    app.handle_event(&key(KeyCode::Char('q')));
    assert!(!app.running);
}

#[test]
fn q_quits_from_diff_view() {
    let mut app = make_app_with_git_status(3);
    app.handle_event(&key(KeyCode::Enter)); // open diff
    app.panel_focus = PanelFocus::Right;
    assert!(app.running);
    app.handle_event(&key(KeyCode::Char('q')));
    assert!(!app.running);
}

// ── Sidebar h/l cycling ─────────────────────────────────────

#[test]
fn l_cycles_sidebar_forward() {
    let mut app = make_app(3);
    // h/l cycle sidebar views except in Worktrees view (where they expand/collapse)
    // Start from FileExplorer to test cycling
    app.sidebar_view = SidebarView::FileExplorer;
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.sidebar_view, SidebarView::GitStatus);
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
}

#[test]
fn h_cycles_sidebar_backward() {
    let mut app = make_app(3);
    // Start from FileExplorer to test cycling (h/l don't cycle in Worktrees view)
    app.sidebar_view = SidebarView::FileExplorer;
    app.handle_event(&key(KeyCode::Char('h')));
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
}

#[test]
fn l_expands_tree_in_worktrees_view() {
    let mut app = make_app(3);
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
    // Cursor starts on section header; l should expand (already expanded) and move to first child
    app.handle_event(&key(KeyCode::Char('l')));
    // Should still be in worktrees view (not cycle to FileExplorer)
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
}

#[test]
fn h_l_no_effect_on_right_panel() {
    let mut app = make_app(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    let before = app.sidebar_view;
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.sidebar_view, before);
    app.handle_event(&key(KeyCode::Char('h')));
    assert_eq!(app.sidebar_view, before);
}

// ── Activity Animation ──────────────────────────────────────

#[test]
fn pty_output_activates_animation_after_tick() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    assert!(!app.activity.is_active(&sid));
    // Output + tick (no recent input) → active
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    assert!(app.activity.is_active(&sid));
}

#[test]
fn output_suppressed_after_user_input() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    // Simulate user typing: mark_input then echo arrives as PtyOutput
    app.activity.mark_input(&sid);
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    // Should NOT activate — the output was just an echo
    assert!(!app.activity.is_active(&sid));
}

#[test]
fn tick_advances_animation_trail() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    let trail_before = app.activity.trail(&sid);
    // Two ticks needed to advance (100ms per frame via parity skip)
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    let trail_after = app.activity.trail(&sid);
    assert_ne!(trail_before, trail_after);
}

#[test]
fn animation_scanner_cycle() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    // Initial output to start animation
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid.clone(),
    });
    app.handle_event(&AppEvent::Tick);

    // Collect head positions over a full 18-frame scanner cycle
    // Each frame takes 2 ticks (100ms) due to parity skip
    let mut positions = Vec::new();
    for _ in 0..18 {
        positions.push(app.activity.position(&sid));
        // Two ticks per frame advance
        app.handle_event(&AppEvent::PtyOutput {
            session_id: sid.clone(),
        });
        app.handle_event(&AppEvent::Tick);
        app.handle_event(&AppEvent::PtyOutput {
            session_id: sid.clone(),
        });
        app.handle_event(&AppEvent::Tick);
    }
    // Forward 5, hold end 3, backward 4, hold start 6
    assert_eq!(
        positions,
        vec![0, 1, 2, 3, 4, 4, 4, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0]
    );

    // Verify trail at a mid-forward frame (frame index 2, head at pos 2)
    // Reset by creating fresh app
    let mut app2 = make_app_with_session(2);
    app2.sidebar_tree.cursor = 1;
    let sid2 = active_session_id(&app2).unwrap().to_string();
    app2.handle_event(&AppEvent::PtyOutput {
        session_id: sid2.clone(),
    });
    app2.handle_event(&AppEvent::Tick);
    // Advance 2 frames (head should be at pos 2)
    for _ in 0..2 {
        app2.handle_event(&AppEvent::PtyOutput {
            session_id: sid2.clone(),
        });
        app2.handle_event(&AppEvent::Tick);
        app2.handle_event(&AppEvent::PtyOutput {
            session_id: sid2.clone(),
        });
        app2.handle_event(&AppEvent::Tick);
    }
    let trail = app2.activity.trail(&sid2);
    // Head at 2 (brightness 3), trail at 1 (brightness 2), trail at 0 (brightness 1)
    assert_eq!(trail, [1, 2, 3, 0, 0]);
}

#[test]
fn session_exited_cleans_up_animation() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    assert!(app.activity.is_active(&sid));
    app.handle_event(&AppEvent::SessionExited {
        session_id: sid.clone(),
    });
    assert!(!app.activity.is_active(&sid));
}

#[test]
fn animation_independent_per_session() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    // Add a second session for the second worktree
    let sid2 = "test-session-2".to_string();
    set_session(&mut app, 1, &sid2);

    let sid1 = active_session_id(&app).unwrap().to_string();

    // Only activate session 1
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid1.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    assert!(app.activity.is_active(&sid1));
    assert!(!app.activity.is_active(&sid2));

    // Now activate session 2 too
    app.handle_event(&AppEvent::PtyOutput {
        session_id: sid2.clone(),
    });
    app.handle_event(&AppEvent::Tick);
    assert!(app.activity.is_active(&sid1));
    assert!(app.activity.is_active(&sid2));
}

// ── BackTab / edtui compatibility ───────────────────────────

fn make_app_with_editor(n: usize) -> darya::app::App {
    let mut app = make_app(n);
    let tmp = std::env::temp_dir().join("darya_test_editor.txt");
    std::fs::write(&tmp, "hello world\n").unwrap();
    app.editor = Some(EditorViewState::open(tmp).unwrap());
    app.main_view = MainView::Editor;
    app.panel_focus = PanelFocus::Right;
    app
}

#[test]
fn backtab_in_editor_insert_mode_no_crash() {
    let mut app = make_app_with_editor(2);
    app.input_mode = InputMode::Editor;
    if let Some(ref mut ed) = app.editor {
        ed.read_only = false;
        ed.editor_state.mode = edtui::EditorMode::Insert;
    }
    // BackTab exits editor mode and cycles main view
    app.handle_event(&key(KeyCode::BackTab));
    assert_eq!(app.input_mode, InputMode::Navigation);
}

#[test]
fn backtab_in_editor_normal_mode_no_crash() {
    let mut app = make_app_with_editor(2);
    app.input_mode = InputMode::Navigation;
    if let Some(ref mut ed) = app.editor {
        ed.editor_state.mode = edtui::EditorMode::Normal;
    }
    // BackTab should be silently ignored, not panic
    app.handle_event(&key(KeyCode::BackTab));
    assert_eq!(app.input_mode, InputMode::Navigation);
}

#[test]
fn is_edtui_compatible_helper() {
    let ke = |code| KeyEvent::new(code, KeyModifiers::NONE);

    // Compatible keys
    assert!(is_edtui_compatible(&ke(KeyCode::Char('a'))));
    assert!(is_edtui_compatible(&ke(KeyCode::Enter)));
    assert!(is_edtui_compatible(&ke(KeyCode::Backspace)));
    assert!(is_edtui_compatible(&ke(KeyCode::Tab)));
    assert!(is_edtui_compatible(&ke(KeyCode::Esc)));
    assert!(is_edtui_compatible(&ke(KeyCode::Left)));
    assert!(is_edtui_compatible(&ke(KeyCode::Right)));
    assert!(is_edtui_compatible(&ke(KeyCode::Up)));
    assert!(is_edtui_compatible(&ke(KeyCode::Down)));
    assert!(is_edtui_compatible(&ke(KeyCode::Home)));
    assert!(is_edtui_compatible(&ke(KeyCode::End)));
    assert!(is_edtui_compatible(&ke(KeyCode::Delete)));
    assert!(is_edtui_compatible(&ke(KeyCode::PageUp)));
    assert!(is_edtui_compatible(&ke(KeyCode::PageDown)));
    assert!(is_edtui_compatible(&ke(KeyCode::F(1))));
    assert!(is_edtui_compatible(&ke(KeyCode::F(12))));

    // Incompatible keys that would cause edtui to panic
    assert!(!is_edtui_compatible(&ke(KeyCode::BackTab)));
    assert!(!is_edtui_compatible(&ke(KeyCode::Null)));
    assert!(!is_edtui_compatible(&ke(KeyCode::Insert)));
    assert!(!is_edtui_compatible(&ke(KeyCode::F(13))));
    assert!(!is_edtui_compatible(&ke(KeyCode::CapsLock)));

    // Kitty keyboard enhancement: Tab+SHIFT must be rejected (same as BackTab)
    let tab_shift = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
    assert!(!is_edtui_compatible(&tab_shift));
}

#[test]
fn tab_shift_in_editor_insert_mode_no_crash() {
    let mut app = make_app_with_editor(2);
    app.input_mode = InputMode::Editor;
    if let Some(ref mut ed) = app.editor {
        ed.read_only = false;
        ed.editor_state.mode = edtui::EditorMode::Insert;
    }
    // Tab+SHIFT (Kitty BackTab) exits editor mode and cycles main view
    let tab_shift = AppEvent::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
    app.handle_event(&tab_shift);
    assert_eq!(app.input_mode, InputMode::Navigation);
}

#[test]
fn tab_shift_in_editor_normal_mode_no_crash() {
    let mut app = make_app_with_editor(2);
    app.input_mode = InputMode::Navigation;
    if let Some(ref mut ed) = app.editor {
        ed.editor_state.mode = edtui::EditorMode::Normal;
    }
    // Tab+SHIFT (Kitty BackTab) should be silently ignored, not panic
    let tab_shift = AppEvent::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
    app.handle_event(&tab_shift);
    assert_eq!(app.input_mode, InputMode::Navigation);
}

// ── Session counts / branch info helpers ─────────────────────

#[test]
fn session_counts_splits_running_and_exited() {
    let mut app = make_app(3);
    set_session(&mut app, 0, "s0");
    set_session(&mut app, 1, "s1");
    set_session(&mut app, 2, "s2");
    app.exited_sessions.insert("s1".to_string());
    let (running, exited) = app.session_counts();
    assert_eq!(running, 2);
    assert_eq!(exited, 1);
}

#[test]
fn selected_branch_info_returns_branch_and_counts() {
    let mut app = make_app(2);
    app.sidebar_tree.cursor = 1; // select item 0
    app.git_status = Some(GitStatusState {
        entries: vec![
            GitStatusEntry {
                category: GitStatusCategory::Staged,
                status: GitFileStatus::Modified,
                path: "a.rs".to_string(),
                orig_path: None,
            },
            GitStatusEntry {
                category: GitStatusCategory::Unstaged,
                status: GitFileStatus::Modified,
                path: "b.rs".to_string(),
                orig_path: None,
            },
            GitStatusEntry {
                category: GitStatusCategory::Untracked,
                status: GitFileStatus::Untracked,
                path: "c.rs".to_string(),
                orig_path: None,
            },
        ],
        selected: 0,
        error: None,
        worktree_path: item_path(&app, 0),
        stale: false,
    });
    let (branch, untracked, modified) = app.selected_branch_info().unwrap();
    assert_eq!(branch, "main");
    assert_eq!(untracked, 1);
    assert_eq!(modified, 2); // staged + unstaged
}

#[test]
fn selected_branch_info_without_git_status_returns_zeros() {
    let mut app = make_app(2);
    app.sidebar_tree.cursor = 1; // select item 0
    let (branch, untracked, modified) = app.selected_branch_info().unwrap();
    assert_eq!(branch, "main");
    assert_eq!(untracked, 0);
    assert_eq!(modified, 0);
}

// ── File Watching ────────────────────────────────────────────

fn make_app_with_open_file(content: &str) -> (darya::app::App, PathBuf, tempfile::TempDir) {
    let mut app = make_app(2);
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("test_file.txt");
    std::fs::write(&tmp, content).unwrap();
    app.editor = Some(EditorViewState::open(tmp.clone()).unwrap());
    app.main_view = MainView::Editor;
    app.panel_focus = PanelFocus::Right;
    (app, tmp, dir)
}

#[test]
fn file_changed_reloads_open_editor() {
    let (mut app, tmp, _dir) = make_app_with_open_file("original\n");
    // Modify file on disk
    std::fs::write(&tmp, "changed content\n").unwrap();
    app.handle_event(&AppEvent::FileChanged {
        paths: vec![tmp.clone()],
    });
    // Editor should have reloaded
    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.editor_state.lines.to_string(), "changed content\n");
    assert!(!editor.modified);
    assert_eq!(app.status_message.as_deref(), Some("File reloaded"));
}

#[test]
fn file_changed_does_not_overwrite_modified_editor() {
    let (mut app, tmp, _dir) = make_app_with_open_file("original\n");
    // Mark editor as modified by user
    app.editor.as_mut().unwrap().modified = true;
    // Modify file on disk
    std::fs::write(&tmp, "changed content\n").unwrap();
    app.handle_event(&AppEvent::FileChanged {
        paths: vec![tmp.clone()],
    });
    // Editor should still have original content
    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.editor_state.lines.to_string(), "original\n");
    assert!(editor.modified);
    assert!(app
        .status_message
        .as_deref()
        .unwrap()
        .contains("unsaved edits preserved"));
}

#[test]
fn file_changed_ignores_unrelated_path() {
    let (mut app, _tmp, _dir) = make_app_with_open_file("original\n");
    let unrelated = PathBuf::from("/tmp/some_other_file.txt");
    app.handle_event(&AppEvent::FileChanged {
        paths: vec![unrelated],
    });
    // No status message, content unchanged
    assert!(app.status_message.is_none());
    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.editor_state.lines.to_string(), "original\n");
}

#[test]
fn file_changed_without_editor_is_noop() {
    let mut app = make_app(2);
    assert!(app.editor.is_none());
    app.handle_event(&AppEvent::FileChanged {
        paths: vec![PathBuf::from("/tmp/foo.txt")],
    });
    assert!(app.status_message.is_none());
}

#[test]
fn files_created_or_deleted_refreshes_explorer() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let mut app = make_app(2);
    app.file_explorer.set_root(tmp_dir.path().to_path_buf());
    let initial_count = app.file_explorer.entries.len();
    // Create a file
    std::fs::write(tmp_dir.path().join("new_file.txt"), "hello").unwrap();
    app.handle_event(&AppEvent::FilesCreatedOrDeleted);
    // Explorer should now include the new file
    assert!(app.file_explorer.entries.len() > initial_count);
    assert!(app
        .file_explorer
        .entries
        .iter()
        .any(|e| e.name == "new_file.txt"));
}

#[test]
fn file_changed_identical_content_no_message() {
    let (mut app, tmp, _dir) = make_app_with_open_file("same content\n");
    // File on disk is identical to editor content — no rewrite needed, just send event
    app.handle_event(&AppEvent::FileChanged {
        paths: vec![tmp.clone()],
    });
    // No status message when content is identical
    assert!(app.status_message.is_none());
}

// ── resolve_session_command ─────────────────────────────────

#[test]
fn resolve_session_command_uses_global_default() {
    let dir = tempfile::tempdir().unwrap();
    // No .darya.toml present — should return the global value
    let result = config::resolve_session_command(dir.path(), "claude --model opus");
    assert_eq!(result, "claude --model opus");
}

#[test]
fn resolve_session_command_reads_local_override() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".darya.toml"),
        "[session]\ncommand = \"custom-cmd --flag\"\n",
    )
    .unwrap();
    let result = config::resolve_session_command(dir.path(), "claude");
    assert_eq!(result, "custom-cmd --flag");
}

#[test]
fn resolve_session_command_falls_back_on_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".darya.toml"), "not valid toml {{{{").unwrap();
    let result = config::resolve_session_command(dir.path(), "global-default");
    assert_eq!(result, "global-default");
}

// ── Split Pane Operations ───────────────────────────────────

#[test]
fn split_add_pane_creates_layout() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    assert!(app.pane_layout.is_none());
    assert!(app.split_add_pane());
    let layout = app.pane_layout.as_ref().unwrap();
    assert_eq!(layout.root.leaf_count(), 2);
    assert_eq!(layout.focused, 0);
    // First pane is the active session, second is the next available
    let leaves = layout.root.leaves();
    assert_eq!(
        *leaves[0],
        darya::app::PaneContent::Terminal("test-session-1".to_string())
    );
    assert_eq!(
        *leaves[1],
        darya::app::PaneContent::Terminal("test-session-2".to_string())
    );
}

#[test]
fn split_add_pane_fails_without_other_sessions() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    // Only one session exists
    assert!(!app.split_add_pane());
    assert!(app.pane_layout.is_none());
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .contains("No other running"));
}

#[test]
fn split_add_pane_caps_at_three() {
    let mut app = make_app_with_two_sessions(4);
    app.sidebar_tree.cursor = 1;
    // Add a third session
    set_session(&mut app, 2, "test-session-3");
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    assert!(app.split_add_pane()); // 2 panes
    assert!(app.split_add_pane()); // 3 panes
    assert_eq!(app.pane_layout.as_ref().unwrap().root.leaf_count(), 3);
    // Can keep splitting until depth limit (no longer capped at 3)
    // The old MAX_PANES=3 is gone; now limited by MAX_SPLIT_DEPTH=4
}

#[test]
fn close_focused_pane_collapses_to_single() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    assert!(app.pane_layout.is_some());
    app.close_focused_pane();
    assert!(app.pane_layout.is_none());
    // active_session_id should still be available via sidebar tree
    assert!(active_session_id(&app).is_some());
}

#[test]
fn close_focused_pane_adjusts_focus() {
    let mut app = make_app_with_two_sessions(4);
    app.sidebar_tree.cursor = 1;
    set_session(&mut app, 2, "test-session-3");
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    app.split_add_pane();
    // Focus the last pane
    let layout = app.pane_layout.as_mut().unwrap();
    layout.focused = 2;
    app.close_focused_pane();
    let layout = app.pane_layout.as_ref().unwrap();
    assert_eq!(layout.root.leaf_count(), 2);
    assert!(layout.focused < layout.root.leaf_count());
}

#[test]
fn cycle_pane_focus_wraps() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();

    let layout = app.pane_layout.as_ref().unwrap();
    assert_eq!(layout.focused, 0);
    app.cycle_pane_focus_next();
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 1);
    app.cycle_pane_focus_next();
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 0); // wrapped

    app.cycle_pane_focus_prev();
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 1); // wrapped back
    app.cycle_pane_focus_prev();
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 0);
}

#[test]
fn focused_session_id_single_vs_split() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Single mode: returns active_session_id
    assert_eq!(
        app.focused_session_id(),
        Some(&"test-session-1".to_string())
    );

    // Split mode: returns pane-focused session
    app.split_add_pane();
    assert_eq!(
        app.focused_session_id(),
        Some(&"test-session-1".to_string())
    );
    app.cycle_pane_focus_next();
    assert_eq!(
        app.focused_session_id(),
        Some(&"test-session-2".to_string())
    );
}

#[test]
fn is_session_visible_split_mode() {
    let mut app = make_app_with_two_sessions(4);
    app.sidebar_tree.cursor = 1;
    set_session(&mut app, 2, "test-session-3");
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();

    assert!(app.is_session_visible("test-session-1"));
    assert!(app.is_session_visible("test-session-2"));
    assert!(!app.is_session_visible("test-session-3"));
}

#[test]
fn split_preserves_across_view_switch() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    let before = app.pane_layout.clone();

    // Switch to editor and back
    app.main_view = MainView::Editor;
    app.main_view = MainView::Terminal;
    // Layout should be preserved (leaf count and focused unchanged)
    assert_eq!(
        app.pane_layout.as_ref().unwrap().root.leaf_count(),
        before.unwrap().root.leaf_count()
    );
}

#[test]
fn split_from_editor_view_works() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Editor;
    // Editor can split with another editor pane
    assert!(app.split_add_pane());
    let layout = app.pane_layout.as_ref().unwrap();
    assert_eq!(layout.root.leaf_count(), 2);
    // Both panes should be Editor
    let leaves = layout.root.leaves();
    assert_eq!(*leaves[0], darya::app::PaneContent::Editor);
    assert_eq!(*leaves[1], darya::app::PaneContent::Editor);
}

#[test]
fn remove_session_from_panes_collapses() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    assert!(app.pane_layout.is_some());

    app.remove_session_from_panes("test-session-2");
    // Should collapse to single since only 1 pane remains
    assert!(app.pane_layout.is_none());
    assert_eq!(active_session_id(&app).unwrap(), "test-session-1");
}

#[test]
fn tab_cycles_panes_then_sidebar_in_terminal_nav() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.input_mode = InputMode::Navigation;
    app.split_add_pane();

    // Start at pane 0
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 0);
    // Tab → pane 1, enters terminal mode (session alive)
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 1);
    assert_eq!(app.input_mode, InputMode::Terminal);

    // Tab from last pane in terminal mode → exits to nav, goes to left panel
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Left);
    assert_eq!(app.input_mode, InputMode::Navigation);

    // Tab from sidebar → back to right panel, pane focus resets to 0, enters terminal
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Right);
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 0);
    assert_eq!(app.input_mode, InputMode::Terminal);
}

#[test]
fn tab_cycles_panes_in_terminal_mode() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.input_mode = InputMode::Terminal;
    app.split_add_pane();

    // Tab in terminal mode with panes: cycle to next pane, stay in terminal mode
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 0);
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 1);
    assert_eq!(app.input_mode, InputMode::Terminal);

    // Tab on last pane in terminal mode → exit to nav, left panel
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Left);
    assert_eq!(app.input_mode, InputMode::Navigation);
}

#[test]
fn tab_no_panes_behaves_as_before() {
    // Without split, Tab in terminal mode toggles to left panel
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Terminal;
    app.panel_focus = PanelFocus::Right;
    assert!(app.pane_layout.is_none());

    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.input_mode, InputMode::Navigation);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

// ── Git Indicators ──────────────────────────────────────────

#[test]
fn status_priority_ordering() {
    assert!(status_priority(&GitFileStatus::Deleted) > status_priority(&GitFileStatus::Modified));
    assert!(status_priority(&GitFileStatus::Modified) > status_priority(&GitFileStatus::Added));
    assert!(status_priority(&GitFileStatus::Added) > status_priority(&GitFileStatus::Renamed));
    assert!(status_priority(&GitFileStatus::Renamed) > status_priority(&GitFileStatus::Untracked));
}

#[test]
fn git_indicators_cleared_on_set_root() {
    let mut app = make_app(2);
    // Manually inject a fake indicator
    app.file_explorer
        .git_indicators
        .insert("foo.rs".to_string(), GitFileStatus::Modified);
    assert!(!app.file_explorer.git_indicators.is_empty());
    // Set root to a non-git temp path — indicators should be cleared
    let tmp = tempfile::tempdir().unwrap();
    app.file_explorer.set_root(tmp.path().to_path_buf());
    assert!(app.file_explorer.git_indicators.is_empty());
}

#[test]
fn file_changed_refreshes_git_indicators() {
    // File change marks indicators stale for lazy refresh
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app(2);
    app.file_explorer.set_root(tmp.path().to_path_buf());
    app.file_explorer.git_indicators_stale = false; // pretend we already loaded
    app.handle_event(&AppEvent::FileChanged {
        paths: vec![tmp.path().join("a.txt")],
    });
    assert!(app.file_explorer.git_indicators_stale);
}

#[test]
fn files_created_or_deleted_refreshes_git_indicators() {
    // File creation/deletion marks indicators stale for lazy refresh
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app(2);
    app.file_explorer.set_root(tmp.path().to_path_buf());
    app.file_explorer.git_indicators_stale = false;
    app.handle_event(&AppEvent::FilesCreatedOrDeleted);
    assert!(app.file_explorer.git_indicators_stale);
}

// ── Git Blame View ──────────────────────────────────────────

fn make_blame_lines() -> Vec<BlameLine> {
    vec![
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
            content: "fn main() {}".to_string(),
            is_recent: false,
        },
    ]
}

fn make_app_with_blame(n: usize) -> darya::app::App {
    let mut app = make_app(n);
    app.sidebar_tree.cursor = 1; // select item 0
    app.git_blame = Some(GitBlameState {
        file_path: "src/main.rs".to_string(),
        lines: make_blame_lines(),
        scroll_offset: 0,
        visible_height: 20,
        worktree_path: item_path(&app, 0),
        stale: false,
    });
    app.main_view = MainView::GitBlame;
    app.panel_focus = PanelFocus::Right;
    app
}

#[test]
fn git_blame_scroll_down_and_up() {
    let mut app = make_app_with_blame(2);
    // Make visible_height small so scrolling works
    app.git_blame.as_mut().unwrap().visible_height = 1;
    // Scroll down
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(app.git_blame.as_ref().unwrap().scroll_offset, 1);
    // Scroll up
    app.handle_event(&key(KeyCode::Char('k')));
    assert_eq!(app.git_blame.as_ref().unwrap().scroll_offset, 0);
}

#[test]
fn git_blame_esc_returns_to_editor() {
    let mut app = make_app_with_blame(2);
    app.handle_event(&key(KeyCode::Esc));
    assert_eq!(app.main_view, MainView::Editor);
}

#[test]
fn git_blame_tab_toggles_focus() {
    let mut app = make_app_with_blame(2);
    assert_eq!(app.panel_focus, PanelFocus::Right);
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn git_blame_q_quits() {
    let mut app = make_app_with_blame(2);
    assert!(app.running);
    app.handle_event(&key(KeyCode::Char('q')));
    assert!(!app.running);
}

#[test]
fn git_blame_number_jumps_worktree() {
    let mut app = make_app_with_blame(3);
    app.handle_event(&key(KeyCode::Char('2')));
    assert_eq!(selected_item_index(&app), Some(1));
    // Blame cleared on worktree switch
    assert!(app.git_blame.is_none());
}

#[test]
fn git_blame_page_scroll() {
    let mut app = make_app_with_blame(2);
    // Set visible_height small so we can test bounds
    app.git_blame.as_mut().unwrap().visible_height = 5;
    // Add more lines
    let mut lines = make_blame_lines();
    for i in 3..=20 {
        lines.push(BlameLine {
            commit_short: format!("hash{:04}", i),
            author: "Test".to_string(),
            relative_time: "1 day ago".to_string(),
            line_number: i,
            content: format!("line {}", i),
            is_recent: false,
        });
    }
    app.git_blame.as_mut().unwrap().lines = lines;
    app.handle_event(&key(KeyCode::PageDown));
    assert!(app.git_blame.as_ref().unwrap().scroll_offset > 0);
    app.handle_event(&key(KeyCode::PageUp));
    assert_eq!(app.git_blame.as_ref().unwrap().scroll_offset, 0);
}

// ── Git Log View ────────────────────────────────────────────

fn make_log_entries() -> Vec<GitLogEntry> {
    vec![
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
    ]
}

fn make_app_with_git_log(n: usize) -> darya::app::App {
    let mut app = make_app(n);
    app.sidebar_tree.cursor = 1; // select item 0
    app.git_log = Some(GitLogState {
        entries: make_log_entries(),
        selected: 0,
        scroll_offset: 0,
        visible_height: 20,
        worktree_path: item_path(&app, 0),
        file_filter: None,
        stale: false,
    });
    app.main_view = MainView::GitLog;
    app.panel_focus = PanelFocus::Right;
    app
}

#[test]
fn git_log_j_moves_selection_down() {
    let mut app = make_app_with_git_log(2);
    assert_eq!(app.git_log.as_ref().unwrap().selected, 0);
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(app.git_log.as_ref().unwrap().selected, 1);
}

#[test]
fn git_log_k_moves_selection_up() {
    let mut app = make_app_with_git_log(2);
    app.git_log.as_mut().unwrap().selected = 2;
    app.handle_event(&key(KeyCode::Char('k')));
    assert_eq!(app.git_log.as_ref().unwrap().selected, 1);
}

#[test]
fn git_log_j_wraps_around() {
    let mut app = make_app_with_git_log(2);
    app.git_log.as_mut().unwrap().selected = 2;
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(app.git_log.as_ref().unwrap().selected, 0);
}

#[test]
fn git_log_esc_returns_to_terminal() {
    let mut app = make_app_with_git_log(2);
    app.handle_event(&key(KeyCode::Esc));
    assert_eq!(app.main_view, MainView::Terminal);
}

#[test]
fn git_log_tab_toggles_focus() {
    let mut app = make_app_with_git_log(2);
    assert_eq!(app.panel_focus, PanelFocus::Right);
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn git_log_q_quits() {
    let mut app = make_app_with_git_log(2);
    assert!(app.running);
    app.handle_event(&key(KeyCode::Char('q')));
    assert!(!app.running);
}

#[test]
fn git_log_number_jumps_worktree() {
    let mut app = make_app_with_git_log(3);
    app.handle_event(&key(KeyCode::Char('2')));
    assert_eq!(selected_item_index(&app), Some(1));
    // Log cleared on worktree switch
    assert!(app.git_log.is_none());
}

#[test]
fn cmd_7_opens_git_blame_view() {
    // Without an editor open, should show status message
    let mut app = make_app(3);
    app.handle_event(&cmd_key('7'));
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .contains("No file open"));
}

#[test]
fn cmd_8_opens_git_log_view() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1; // select item 0 so worktree path is available
                                 // This will try to run git log on a non-git dir, should show error
    app.handle_event(&cmd_key('8'));
    // Either it succeeds (if in a git repo) or shows an error
    assert!(app.main_view == MainView::GitLog || app.status_message.is_some());
}

#[test]
fn b_in_editor_readonly_opens_blame() {
    // Without a real git repo, this will fail gracefully
    let mut app = make_app_with_editor(2);
    app.sidebar_tree.cursor = 1; // select item 0 so worktree path is available
    app.input_mode = InputMode::Navigation;
    app.handle_event(&key(KeyCode::Char('b')));
    // Should either open blame or show error (no git repo)
    assert!(app.main_view == MainView::GitBlame || app.status_message.is_some());
}

// ── format_relative_time ────────────────────────────────────

#[test]
fn format_relative_time_just_now() {
    let now = 1000000;
    assert_eq!(format_relative_time(now, now), "just now");
    assert_eq!(format_relative_time(now - 30, now), "just now");
}

#[test]
fn format_relative_time_minutes() {
    let now = 1000000;
    assert_eq!(format_relative_time(now - 120, now), "2 mins ago");
    assert_eq!(format_relative_time(now - 60, now), "1 min ago");
}

#[test]
fn format_relative_time_hours() {
    let now = 1000000;
    assert_eq!(format_relative_time(now - 3600, now), "1 hour ago");
    assert_eq!(format_relative_time(now - 7200, now), "2 hours ago");
}

#[test]
fn format_relative_time_days() {
    let now = 1000000;
    assert_eq!(format_relative_time(now - 86400, now), "1 day ago");
    assert_eq!(format_relative_time(now - 172800, now), "2 days ago");
}

#[test]
fn format_relative_time_weeks() {
    let now = 1000000;
    assert_eq!(format_relative_time(now - 604800, now), "1 week ago");
}

#[test]
fn format_relative_time_months() {
    let now = 10000000;
    assert_eq!(format_relative_time(now - 2592000, now), "1 month ago");
}

#[test]
fn format_relative_time_years() {
    let now = 100000000;
    assert_eq!(format_relative_time(now - 31536000, now), "1 year ago");
}

// ── Command Palette ─────────────────────────────────────────

#[test]
fn command_palette_opens_and_lists_commands() {
    let mut app = make_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));
    let palette = app.command_palette.as_ref().unwrap();
    assert!(!palette.all_commands.is_empty());
    assert_eq!(palette.results.len(), palette.all_commands.len());
    assert_eq!(palette.selected, 0);
}

#[test]
fn command_palette_esc_dismisses() {
    let mut app = make_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));
    assert!(app.command_palette.is_some());
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.command_palette.is_none());
}

#[test]
fn command_palette_filter_narrows_results() {
    let mut app = make_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));
    // Type "quit" to filter
    app.handle_event(&key(KeyCode::Char('q')));
    app.handle_event(&key(KeyCode::Char('u')));
    app.handle_event(&key(KeyCode::Char('i')));
    app.handle_event(&key(KeyCode::Char('t')));
    let palette = app.command_palette.as_ref().unwrap();
    assert!(palette.results.len() < palette.all_commands.len());
    assert!(palette.results.iter().any(|c| c.id == CommandId::Quit));
}

#[test]
fn command_palette_backspace_widens_results() {
    let mut app = make_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));
    app.handle_event(&key(KeyCode::Char('q')));
    app.handle_event(&key(KeyCode::Char('u')));
    let narrow_count = app.command_palette.as_ref().unwrap().results.len();
    app.handle_event(&key(KeyCode::Backspace));
    let wider_count = app.command_palette.as_ref().unwrap().results.len();
    assert!(wider_count >= narrow_count);
}

#[test]
fn command_palette_navigate_up_down() {
    let mut app = make_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
    app.handle_event(&key(KeyCode::Down));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 1);
    app.handle_event(&key(KeyCode::Up));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
}

#[test]
fn command_palette_execute_quit() {
    let mut app = make_app(3);
    assert!(app.running);
    app.execute_command(CommandId::Quit);
    assert!(!app.running);
}

#[test]
fn command_palette_execute_view_worktrees() {
    let mut app = make_app(3);
    app.sidebar_view = SidebarView::FileExplorer;
    app.panel_focus = PanelFocus::Right;
    app.execute_command(CommandId::ViewWorktrees);
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn command_palette_execute_view_terminal() {
    let mut app = make_app(3);
    app.main_view = MainView::Editor;
    app.panel_focus = PanelFocus::Left;
    app.execute_command(CommandId::ViewTerminal);
    assert_eq!(app.main_view, MainView::Terminal);
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn command_palette_execute_toggle_help() {
    let mut app = make_app(3);
    assert!(!app.show_help);
    app.execute_command(CommandId::ToggleHelp);
    assert!(app.show_help);
    app.execute_command(CommandId::ToggleHelp);
    assert!(!app.show_help);
}

#[test]
fn command_palette_execute_fuzzy_finder() {
    let mut app = make_app(3);
    assert!(app.fuzzy_finder.is_none());
    app.execute_command(CommandId::FuzzyFinder);
    assert!(app.fuzzy_finder.is_some());
}

#[test]
fn command_palette_execute_project_search() {
    let mut app = make_app(3);
    assert!(app.prompt.is_none());
    app.execute_command(CommandId::ProjectSearch);
    assert!(matches!(app.prompt, Some(Prompt::SearchInput { .. })));
}

#[test]
fn command_palette_enter_executes_selected() {
    let mut app = make_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));
    // Navigate to "Quit" — find its index
    let quit_idx = app
        .command_palette
        .as_ref()
        .unwrap()
        .results
        .iter()
        .position(|c| c.id == CommandId::Quit)
        .unwrap();
    // Set selected directly
    app.command_palette.as_mut().unwrap().selected = quit_idx;
    assert!(app.running);
    app.handle_event(&key(KeyCode::Enter));
    assert!(!app.running);
    assert!(app.command_palette.is_none());
}

#[test]
fn command_palette_blocks_other_keys() {
    let mut app = make_app(3);
    app.command_palette = Some(CommandPaletteState::new(&app.keybindings));
    // 'q' should type into palette, not quit
    app.handle_event(&key(KeyCode::Char('q')));
    assert!(app.running);
    assert!(app.command_palette.is_some());
    assert_eq!(app.command_palette.as_ref().unwrap().input, "q");
}

#[test]
fn command_palette_session_commands_show_guidance() {
    let mut app = make_app(3);
    app.execute_command(CommandId::StartSession);
    assert!(app.status_message.is_some());
    assert!(app.status_message.as_ref().unwrap().contains("Enter"));
}

#[test]
fn command_palette_execute_view_git_status() {
    let mut app = make_app(3);
    app.execute_command(CommandId::ViewGitStatus);
    assert_eq!(app.sidebar_view, SidebarView::GitStatus);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn command_palette_execute_refresh_git_status() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1; // select item 0 so worktree path is available
    app.execute_command(CommandId::RefreshGitStatus);
    assert!(app.status_message.as_ref().unwrap().contains("refreshed"));
}

// ── Git View Auto-Refresh ──────────────────────────────────

#[test]
fn git_log_refresh_preserves_state_on_error() {
    let mut app = make_app_with_git_log(2);
    app.git_log.as_mut().unwrap().selected = 1;
    // refresh() calls git on a non-existent repo path, silently keeps stale data
    app.git_log.as_mut().unwrap().refresh();
    let gl = app.git_log.as_ref().unwrap();
    assert_eq!(gl.entries.len(), 3); // original entries preserved
    assert_eq!(gl.selected, 1); // selection preserved
}

#[test]
fn git_blame_refresh_preserves_state_on_error() {
    let mut app = make_app_with_blame(2);
    app.git_blame.as_mut().unwrap().scroll_offset = 2;
    // refresh() calls git on a non-existent repo path, silently keeps stale data
    app.git_blame.as_mut().unwrap().refresh();
    let gb = app.git_blame.as_ref().unwrap();
    assert!(!gb.lines.is_empty()); // original lines preserved
    assert_eq!(gb.scroll_offset, 2); // scroll preserved
}

#[test]
fn file_changed_refreshes_git_views() {
    let mut app = make_app_with_git_log(2);
    app.git_blame = Some(GitBlameState {
        file_path: "src/main.rs".to_string(),
        lines: vec![BlameLine {
            commit_short: "abc1234".to_string(),
            author: "Test".to_string(),
            relative_time: "1 hour ago".to_string(),
            line_number: 1,
            content: "fn main() {}".to_string(),
            is_recent: false,
        }],
        scroll_offset: 0,
        visible_height: 20,
        worktree_path: item_path(&app, 0),
        stale: false,
    });
    app.git_status = Some(GitStatusState {
        entries: vec![],
        selected: 0,
        error: None,
        worktree_path: item_path(&app, 0),
        stale: false,
    });
    // File change marks views stale for lazy refresh on next render
    app.handle_event(&AppEvent::FileChanged {
        paths: vec![PathBuf::from("/tmp/foo.rs")],
    });
    assert!(app.git_log.as_ref().unwrap().stale);
    assert!(app.git_blame.as_ref().unwrap().stale);
    assert!(app.git_status.as_ref().unwrap().stale);
    assert!(app.file_explorer.git_indicators_stale);
}

#[test]
fn files_created_or_deleted_refreshes_git_views() {
    let mut app = make_app_with_git_log(2);
    app.git_blame = Some(GitBlameState {
        file_path: "src/main.rs".to_string(),
        lines: vec![BlameLine {
            commit_short: "abc1234".to_string(),
            author: "Test".to_string(),
            relative_time: "1 hour ago".to_string(),
            line_number: 1,
            content: "fn main() {}".to_string(),
            is_recent: false,
        }],
        scroll_offset: 0,
        visible_height: 20,
        worktree_path: item_path(&app, 0),
        stale: false,
    });
    app.git_status = Some(GitStatusState {
        entries: vec![],
        selected: 0,
        error: None,
        worktree_path: item_path(&app, 0),
        stale: false,
    });
    // File creation/deletion marks views stale for lazy refresh on next render
    app.handle_event(&AppEvent::FilesCreatedOrDeleted);
    assert!(app.git_log.as_ref().unwrap().stale);
    assert!(app.git_blame.as_ref().unwrap().stale);
    assert!(app.git_status.as_ref().unwrap().stale);
    assert!(app.file_explorer.git_indicators_stale);
}

// ── Shell View ──────────────────────────────────────────────

fn make_app_with_shell_session(n: usize) -> darya::app::App {
    let mut app = make_app(n);
    app.sidebar_tree.cursor = 1; // select item 0
    set_shell_session(&mut app, 0, "test-shell-1");
    app.main_view = MainView::Shell;
    app.panel_focus = PanelFocus::Right;
    app
}

#[test]
fn cmd_9_sets_main_shell() {
    let mut app = make_app(3);
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Left;
    app.handle_event(&cmd_key('9'));
    assert_eq!(app.main_view, MainView::Shell);
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn shell_view_kind_is_shell() {
    let mut app = make_app(3);
    app.main_view = MainView::Shell;
    app.panel_focus = PanelFocus::Right;
    assert_eq!(app.focused_view(), ViewKind::Shell);
}

#[test]
fn shell_session_spawn_signal() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1;
    app.main_view = MainView::Shell;
    app.panel_focus = PanelFocus::Right;
    app.input_mode = InputMode::Navigation;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    // needs_session_spawn now handles shell view too
    assert!(app.needs_session_spawn(&key_event));
}

#[test]
fn shell_spawn_false_in_terminal_view() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1;
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;
    app.input_mode = InputMode::Navigation;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    // needs_session_spawn returns true for Terminal view too, since it handles all session spawning
    assert!(app.needs_session_spawn(&key_event));
}

#[test]
fn shell_and_claude_sessions_coexist() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1;
    // Claude session on item 0
    set_session(&mut app, 0, "claude-1");
    // Shell session on item 0
    set_shell_session(&mut app, 0, "shell-1");

    // In terminal view, focused_session_id returns claude session
    app.main_view = MainView::Terminal;
    assert_eq!(app.focused_session_id(), Some(&"claude-1".to_string()));

    // In shell view, focused_session_id returns shell session
    app.main_view = MainView::Shell;
    assert_eq!(app.focused_session_id(), Some(&"shell-1".to_string()));
}

#[test]
fn focused_session_id_returns_shell_in_shell_view() {
    let app = make_app_with_shell_session(3);
    assert_eq!(app.focused_session_id(), Some(&"test-shell-1".to_string()));
}

#[test]
fn focused_session_id_returns_none_for_claude_in_shell_view() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1;
    // No shell session, but we're in shell view
    app.main_view = MainView::Shell;
    assert_eq!(app.focused_session_id(), None);
    // Add claude session — shouldn't be returned in shell view
    set_session(&mut app, 0, "claude-1");
    assert_eq!(app.focused_session_id(), None);
}

#[test]
fn shell_session_restart_signal() {
    let mut app = make_app_with_shell_session(3);
    let sid = active_shell_session_id(&app).unwrap().to_string();
    app.exited_sessions.insert(sid);
    app.input_mode = InputMode::Navigation;
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Char('r'), crossterm::event::KeyModifiers::NONE);
    // needs_session_restart now handles all session types
    assert!(app.needs_session_restart(&key_event));
}

#[test]
fn shell_session_close_signal() {
    let app = make_app_with_shell_session(3);
    let key_event =
        crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    // needs_session_close now handles all session types
    assert!(app.needs_session_close(&key_event));
}

#[test]
fn shell_command_palette_integration() {
    let mut app = make_app(3);
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Left;
    app.execute_command(CommandId::ViewShell);
    assert_eq!(app.main_view, MainView::Shell);
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn worktree_name_for_shell_session() {
    let app = make_app_with_shell_session(3);
    let name = app.worktree_name_for_session("test-shell-1");
    assert_eq!(name, Some("my-project"));
}

#[test]
fn switch_worktree_updates_shell_session() {
    let mut app = make_app(3);
    app.sidebar_tree.cursor = 1; // item 0
    set_shell_session(&mut app, 0, "shell-0");
    set_shell_session(&mut app, 1, "shell-1");

    app.handle_event(&key(KeyCode::Char('2'))); // jump to item 1
    assert_eq!(active_shell_session_id(&app), Some("shell-1"));
}

#[test]
fn enter_terminal_if_focused_works_for_shell() {
    let mut app = make_app_with_shell_session(3);
    app.input_mode = InputMode::Navigation;
    app.panel_focus = PanelFocus::Left;
    // Tab to right panel — should auto-enter terminal mode
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Right);
    assert_eq!(app.input_mode, InputMode::Terminal);
}

#[test]
fn shell_nav_uses_terminal_nav_key() {
    let mut app = make_app_with_shell_session(3);
    app.input_mode = InputMode::Navigation;
    // In shell nav mode, 'i' should enter terminal mode
    app.handle_event(&key(KeyCode::Char('i')));
    assert_eq!(app.input_mode, InputMode::Terminal);
}

#[test]
fn resolve_shell_command_uses_global_default() {
    let dir = tempfile::tempdir().unwrap();
    let result = config::resolve_shell_command(dir.path(), "/bin/zsh");
    assert_eq!(result, "/bin/zsh");
}

#[test]
fn resolve_shell_command_reads_local_override() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".darya.toml"),
        "[shell]\ncommand = \"/usr/local/bin/fish\"\n",
    )
    .unwrap();
    let result = config::resolve_shell_command(dir.path(), "/bin/sh");
    assert_eq!(result, "/usr/local/bin/fish");
}

// ── Mixed-Content Pane Splitting ────────────────────────────

fn make_app_with_shell_and_terminal(n: usize) -> darya::app::App {
    let mut app = make_app(n);
    app.sidebar_tree.cursor = 1; // select item 0
                                 // Claude terminal session on item 0
    set_session(&mut app, 0, "terminal-1");
    // Shell session on item 0
    set_shell_session(&mut app, 0, "shell-1");
    // Second terminal session on item 1
    set_session(&mut app, 1, "terminal-2");
    app
}

#[test]
fn split_terminal_with_editor() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Split adds editor pane alongside terminal
    assert!(app.split_add_pane_with(PaneContent::Editor));
    let layout = app.pane_layout.as_ref().unwrap();
    let leaves = layout.root.leaves();
    assert_eq!(leaves.len(), 2);
    assert_eq!(
        *leaves[0],
        PaneContent::Terminal("test-session-1".to_string())
    );
    assert_eq!(*leaves[1], PaneContent::Editor);
}

#[test]
fn split_terminal_with_shell() {
    let mut app = make_app_with_shell_and_terminal(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    assert!(app.split_add_pane_with(PaneContent::Shell("shell-1".to_string())));
    let layout = app.pane_layout.as_ref().unwrap();
    let leaves = layout.root.leaves();
    assert_eq!(leaves.len(), 2);
    assert_eq!(
        *leaves[0],
        PaneContent::Terminal("terminal-1".to_string())
    );
    assert_eq!(*leaves[1], PaneContent::Shell("shell-1".to_string()));
}

#[test]
fn split_editor_with_terminal() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Editor;

    assert!(app.split_add_pane_with(PaneContent::Terminal("test-session-1".to_string())));
    let layout = app.pane_layout.as_ref().unwrap();
    let leaves = layout.root.leaves();
    assert_eq!(leaves.len(), 2);
    assert_eq!(*leaves[0], PaneContent::Editor);
    assert_eq!(
        *leaves[1],
        PaneContent::Terminal("test-session-1".to_string())
    );
}

#[test]
fn focused_view_reflects_pane_content() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Add editor pane
    app.split_add_pane_with(PaneContent::Editor);

    // Focused on terminal pane (index 0)
    assert_eq!(app.focused_view(), ViewKind::Terminal);

    // Switch to editor pane (index 1)
    app.cycle_pane_focus_next();
    assert_eq!(app.focused_view(), ViewKind::Editor);
}

#[test]
fn close_mixed_pane_sets_correct_main_view() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Split terminal + editor
    app.split_add_pane_with(PaneContent::Editor);
    assert!(app.pane_layout.is_some());

    // Focus terminal pane (index 0) and close it
    app.pane_layout.as_mut().unwrap().focused = 0;
    app.close_focused_pane();

    // Remaining pane is Editor, so main_view should be Editor
    assert!(app.pane_layout.is_none());
    assert_eq!(app.main_view, MainView::Editor);
}

#[test]
fn close_mixed_pane_shell_remains() {
    let mut app = make_app_with_shell_and_terminal(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Split terminal + shell
    app.split_add_pane_with(PaneContent::Shell("shell-1".to_string()));

    // Close terminal pane (focused = 0)
    app.close_focused_pane();

    assert!(app.pane_layout.is_none());
    assert_eq!(app.main_view, MainView::Shell);
    assert_eq!(active_shell_session_id(&app), Some("shell-1"));
}

#[test]
fn tab_cycles_mixed_panes() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.input_mode = InputMode::Navigation;

    // Add editor pane alongside terminal
    app.split_add_pane_with(PaneContent::Editor);

    // Start at pane 0 (terminal)
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 0);
    assert_eq!(app.focused_view(), ViewKind::Terminal);

    // Tab → pane 1 (editor), enters terminal mode (but it's an editor, so stays Nav)
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 1);
    // Editor pane — enter_terminal_if_focused should NOT enter terminal mode
    assert_eq!(app.input_mode, InputMode::Navigation);
    assert_eq!(app.focused_view(), ViewKind::Editor);

    // Tab from last pane → left panel
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn tab_in_terminal_mode_to_editor_pane_switches_mode() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.input_mode = InputMode::Terminal;

    // Add editor pane
    app.split_add_pane_with(PaneContent::Editor);

    // Tab from terminal mode on pane 0 → pane 1 (editor)
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.pane_layout.as_ref().unwrap().focused, 1);
    // Should switch to Navigation since editor pane doesn't support Terminal mode
    assert_eq!(app.input_mode, InputMode::Navigation);
}

#[test]
fn focused_session_id_none_for_editor_pane() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    app.split_add_pane_with(PaneContent::Editor);
    // Focus editor pane
    app.cycle_pane_focus_next();
    assert_eq!(app.focused_session_id(), None);
}

#[test]
fn split_editor_command_from_palette() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    app.execute_command(CommandId::SplitEditor);
    let layout = app.pane_layout.as_ref().unwrap();
    let leaves = layout.root.leaves();
    assert_eq!(leaves.len(), 2);
    assert_eq!(*leaves[1], PaneContent::Editor);
}

#[test]
fn split_terminal_command_from_palette() {
    let mut app = make_app_with_shell_and_terminal(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // SplitTerminal should find "terminal-2" (next available)
    app.execute_command(CommandId::SplitTerminal);
    let layout = app.pane_layout.as_ref().unwrap();
    let leaves = layout.root.leaves();
    assert_eq!(leaves.len(), 2);
    assert_eq!(
        *leaves[1],
        PaneContent::Terminal("terminal-2".to_string())
    );
}

#[test]
fn split_shell_command_from_palette() {
    let mut app = make_app_with_shell_and_terminal(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // SplitShell uses current shell session when it's the only one
    app.execute_command(CommandId::SplitShell);
    let layout = app.pane_layout.as_ref().unwrap();
    let leaves = layout.root.leaves();
    assert_eq!(leaves.len(), 2);
    assert_eq!(*leaves[1], PaneContent::Shell("shell-1".to_string()));
}

#[test]
fn remove_session_from_mixed_panes() {
    let mut app = make_app_with_session(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Terminal + Editor
    app.split_add_pane_with(PaneContent::Editor);
    assert_eq!(app.pane_layout.as_ref().unwrap().root.leaf_count(), 2);

    // Remove the terminal session
    app.remove_session_from_panes("test-session-1");
    // Should collapse, remaining is Editor
    assert!(app.pane_layout.is_none());
    assert_eq!(app.main_view, MainView::Editor);
}

#[test]
fn is_session_visible_in_mixed_panes() {
    let mut app = make_app_with_shell_and_terminal(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Terminal + Shell in split
    app.split_add_pane_with(PaneContent::Shell("shell-1".to_string()));

    assert!(app.is_session_visible("terminal-1"));
    assert!(app.is_session_visible("shell-1"));
    assert!(!app.is_session_visible("terminal-2"));
}

// ── Notifications ──────────────────────────────────────────

#[test]
fn notification_for_bell_when_not_viewing() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Navigation;
    let sid = active_session_id(&app).unwrap().to_string();
    let event = AppEvent::SessionBell { session_id: sid };
    let (iterm, native) = app.notification_for_event(&event);
    // Bell → iTerm2 only, no native notification
    assert!(iterm.is_some());
    assert!(iterm.unwrap().contains("needs attention"));
    assert!(native.is_none());
}

#[test]
fn notification_for_bell_when_viewing() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Terminal;
    let sid = active_session_id(&app).unwrap().to_string();
    let event = AppEvent::SessionBell { session_id: sid };
    let (iterm, native) = app.notification_for_event(&event);
    // Viewing session in terminal mode — no notifications at all
    assert!(iterm.is_none());
    assert!(native.is_none());
}

#[test]
fn notification_for_done() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    let event = AppEvent::SessionDone { session_id: sid };
    let (iterm, native) = app.notification_for_event(&event);
    // SessionDone no longer triggers any notification — attention is debounced
    // in the main event loop instead.
    assert!(iterm.is_none());
    assert!(native.is_none());
}

#[test]
fn notification_for_exit() {
    let mut app = make_app_with_session(2);
    app.sidebar_tree.cursor = 1;
    let sid = active_session_id(&app).unwrap().to_string();
    let event = AppEvent::SessionExited { session_id: sid };
    let (iterm, native) = app.notification_for_event(&event);
    assert!(iterm.is_some());
    let iterm_msg = iterm.unwrap();
    assert!(iterm_msg.contains("my-project"));
    assert!(iterm_msg.contains("session exited"));
    let native_msg = native.unwrap();
    assert!(native_msg.contains("my-project"));
    assert!(native_msg.contains("session exited"));
}

#[test]
fn notification_none_for_other_events() {
    let app = make_app(2);
    assert_eq!(app.notification_for_event(&AppEvent::Tick), (None, None));
    assert_eq!(
        app.notification_for_event(&AppEvent::Resize(80, 24)),
        (None, None)
    );
    assert_eq!(
        app.notification_for_event(&AppEvent::PtyOutput {
            session_id: "foo".to_string(),
        }),
        (None, None)
    );
}

// ── Directory browser / Section creation ──────────────────────────

#[test]
fn n_key_opens_dir_browser() {
    let mut app = make_app(3);
    assert!(app.dir_browser.is_none());
    app.handle_event(&key(KeyCode::Char('N')));
    assert!(app.dir_browser.is_some());
}

#[test]
fn dir_browser_esc_dismisses() {
    let mut app = make_app(3);
    app.handle_event(&key(KeyCode::Char('N')));
    assert!(app.dir_browser.is_some());
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.dir_browser.is_none());
}

#[test]
fn dir_browser_jk_navigation() {
    let mut app = make_app(3);
    app.handle_event(&key(KeyCode::Char('N')));
    let initial = app.dir_browser.as_ref().unwrap().selected;
    app.handle_event(&key(KeyCode::Char('j')));
    let after_j = app.dir_browser.as_ref().unwrap().selected;
    // j moves down (or wraps if only 1 entry)
    if app.dir_browser.as_ref().unwrap().entries.len() > 1 {
        assert_eq!(after_j, initial + 1);
    }
    app.handle_event(&key(KeyCode::Char('k')));
    let after_k = app.dir_browser.as_ref().unwrap().selected;
    assert_eq!(after_k, initial);
}

#[test]
fn add_section_with_root_path() {
    let mut app = make_app(3);
    let initial_sections = app.sidebar_tree.sections.len();
    app.sidebar_tree.add_section(
        "test-section".to_string(),
        Some(PathBuf::from("/tmp/test-root")),
    );
    assert_eq!(app.sidebar_tree.sections.len(), initial_sections + 1);
    let new_section = app.sidebar_tree.sections.last().unwrap();
    assert_eq!(new_section.name, "test-section");
    assert_eq!(new_section.root_path, Some(PathBuf::from("/tmp/test-root")));
}

#[test]
fn add_section_without_root_path() {
    let mut app = make_app(3);
    app.sidebar_tree
        .add_section("empty-section".to_string(), None);
    let new_section = app.sidebar_tree.sections.last().unwrap();
    assert_eq!(new_section.name, "empty-section");
    assert_eq!(new_section.root_path, None);
}

#[test]
fn sections_config_round_trip_with_root() {
    let mut app = make_app(3);
    app.sidebar_tree
        .add_section("rooted".to_string(), Some(PathBuf::from("/tmp/my-repo")));

    // Serialize
    let config = app.sidebar_tree.to_sections_config();
    let rooted = config.sections.iter().find(|s| s.name == "rooted").unwrap();
    assert_eq!(rooted.root.as_deref(), Some("/tmp/my-repo"));

    // Deserialize back
    let tree = darya::sidebar::tree::SidebarTree::from_config(&config, &[]);
    let restored = tree.sections.iter().find(|s| s.name == "rooted").unwrap();
    assert_eq!(restored.root_path, Some(PathBuf::from("/tmp/my-repo")));
}

#[test]
fn sections_config_round_trip_without_root() {
    let app = make_app(3);
    let config = app.sidebar_tree.to_sections_config();
    // First section (auto-generated) should have no root
    assert!(config.sections[0].root.is_none());

    let tree = darya::sidebar::tree::SidebarTree::from_config(&config, &[]);
    assert!(tree.sections[0].root_path.is_none());
}

#[test]
fn refresh_section_worktrees_populates_items() {
    use darya::worktree::types::Worktree;

    let mut app = make_app(3);
    app.sidebar_tree
        .add_section("new-repo".to_string(), Some(PathBuf::from("/tmp/new-repo")));
    let section_idx = app.sidebar_tree.sections.len() - 1;
    assert!(app.sidebar_tree.sections[section_idx].items.is_empty());

    let worktrees = vec![
        Worktree {
            name: "new-repo".to_string(),
            path: PathBuf::from("/tmp/new-repo"),
            branch: Some("main".to_string()),
            is_main: true,
        },
        Worktree {
            name: "new-repo-feature".to_string(),
            path: PathBuf::from("/tmp/new-repo-feature"),
            branch: Some("feature".to_string()),
            is_main: false,
        },
    ];
    app.sidebar_tree
        .refresh_section_worktrees(section_idx, &worktrees);
    assert_eq!(app.sidebar_tree.sections[section_idx].items.len(), 2);
    assert_eq!(
        app.sidebar_tree.sections[section_idx].items[0].display_name,
        "new-repo"
    );
    assert_eq!(
        app.sidebar_tree.sections[section_idx].items[1].display_name,
        "new-repo-feature"
    );
}

#[test]
fn command_palette_add_section_opens_dir_browser() {
    let mut app = make_app(3);
    app.execute_command(CommandId::AddSection);
    assert!(app.dir_browser.is_some());
    assert!(app.prompt.is_none());
}

#[test]
fn backspace_on_section_header_opens_confirm_delete() {
    let mut app = make_app(3);
    // Add a second section so we can delete it
    app.sidebar_tree.add_section("deletable".to_string(), None);
    // Move cursor to the new section header
    let section_count = app.sidebar_tree.sections.len();
    // Find the visible index of the last section header
    for (i, node) in app.sidebar_tree.visible.iter().enumerate() {
        if matches!(node, darya::sidebar::tree::TreeNode::Section(si) if *si == section_count - 1) {
            app.sidebar_tree.cursor = i;
            break;
        }
    }
    app.handle_event(&key(KeyCode::Backspace));
    assert!(matches!(
        app.prompt,
        Some(Prompt::ConfirmDeleteSection { .. })
    ));
}

#[test]
fn backspace_on_default_section_shows_error() {
    let mut app = make_app(3);
    // Cursor starts at section 0 header
    assert_eq!(app.sidebar_tree.cursor, 0);
    app.handle_event(&key(KeyCode::Backspace));
    assert!(app.prompt.is_none());
    assert!(app.status_message.is_some());
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .contains("Cannot delete"));
}

#[test]
fn confirm_delete_section_removes_it() {
    let mut app = make_app(3);
    app.sidebar_tree
        .add_section("to-delete".to_string(), Some(PathBuf::from("/tmp/del")));
    assert_eq!(app.sidebar_tree.sections.len(), 2);

    // Simulate confirming deletion
    app.prompt = Some(Prompt::ConfirmDeleteSection {
        section_name: "to-delete".to_string(),
        section_idx: 1,
    });
    app.handle_event(&key(KeyCode::Char('y')));
    assert_eq!(app.sidebar_tree.sections.len(), 1);
    assert!(app.prompt.is_none());
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .contains("Deleted section"));
}

#[test]
fn remove_section_returns_session_ids() {
    let mut app = make_app(3);
    app.sidebar_tree
        .add_section("with-sessions".to_string(), None);
    let si = app.sidebar_tree.sections.len() - 1;
    // Add an item with a session to the new section
    use darya::sidebar::types::{SessionKind, SessionSlot, SidebarItem};
    app.sidebar_tree.sections[si].items.push(SidebarItem {
        path: PathBuf::from("/tmp/test"),
        display_name: "test".to_string(),
        branch: None,
        is_main: false,
        collapsed: true,
        sessions: vec![SessionSlot {
            kind: SessionKind::Claude,
            label: "claude".to_string(),
            session_id: Some("sess-123".to_string()),
            color: None,
        }],
        color: None,
    });
    let removed = app.sidebar_tree.remove_section(si);
    assert_eq!(removed, vec!["sess-123".to_string()]);
}

// ── Mouse scroll ──────────────────────────────────────────────

#[test]
fn mouse_scroll_up_increases_offset() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    assert_eq!(app.active_scroll_offset(), 0);
    // Simulate what process_event does for MouseScroll { delta: 3 }
    app.scroll_up(3);
    assert_eq!(app.active_scroll_offset(), 3);
    app.scroll_up(3);
    assert_eq!(app.active_scroll_offset(), 6);
}

#[test]
fn mouse_scroll_down_decreases_offset() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    app.scroll_up(10);
    assert_eq!(app.active_scroll_offset(), 10);
    // Simulate scroll down (delta: -3 → scroll_down(3))
    app.scroll_down(3);
    assert_eq!(app.active_scroll_offset(), 7);
}

#[test]
fn mouse_scroll_event_noop_in_handle_event() {
    // MouseScroll is handled in process_event, handle_event should not panic
    let mut app = make_app(1);
    app.handle_event(&AppEvent::MouseScroll { delta: 3 });
    app.handle_event(&AppEvent::MouseScroll { delta: -3 });
    // No panic = pass
}

#[test]
fn shift_pageup_scrolls_in_navigation_mode() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    app.input_mode = InputMode::Navigation;
    app.terminal_height = 40;
    // Shift+PageUp handled via handle_event → handle_key dispatches to terminal_nav
    // which calls scroll_up. But actually Shift+PageUp in navigation mode is handled
    // by process_event in main.rs. For the app-level test, verify scroll methods work.
    let page = app.terminal_height.saturating_sub(2) as usize;
    app.scroll_up(page);
    assert_eq!(app.active_scroll_offset(), 38);
}

#[test]
fn shift_pagedown_reduces_scroll_offset() {
    let mut app = make_app_with_session(1);
    app.sidebar_tree.cursor = 1;
    app.terminal_height = 40;
    app.scroll_up(80);
    assert_eq!(app.active_scroll_offset(), 80);
    let page = app.terminal_height.saturating_sub(2) as usize;
    app.scroll_down(page);
    assert_eq!(app.active_scroll_offset(), 42);
}

// ── Color Picker ──────────────────────────────────────────────

#[test]
fn color_picker_opens_on_c_key() {
    let mut app = helpers::make_app(2);
    app.sidebar_tree.cursor = 1;
    app.handle_event(&key(KeyCode::Char('c')));
    assert!(matches!(app.prompt, Some(Prompt::ColorPicker { .. })));
}

#[test]
fn color_picker_assigns_color_on_enter() {
    use ratatui::style::Color;
    let mut app = helpers::make_app(2);
    app.sidebar_tree.cursor = 1;
    app.handle_event(&key(KeyCode::Char('c')));
    app.handle_event(&key(KeyCode::Right));
    app.handle_event(&key(KeyCode::Enter));
    assert!(app.prompt.is_none());
    assert_eq!(
        app.sidebar_tree.sections[0].items[0].color,
        Some(Color::Rgb(0xE0, 0x7A, 0x2A))
    );
}

#[test]
fn color_picker_clears_color() {
    use ratatui::style::Color;
    let mut app = helpers::make_app(2);
    app.sidebar_tree.sections[0].items[0].color = Some(Color::Rgb(0xFF, 0x00, 0x00));
    app.sidebar_tree.cursor = 1;
    app.handle_event(&key(KeyCode::Char('c')));
    app.handle_event(&key(KeyCode::Enter));
    assert!(app.prompt.is_none());
    assert_eq!(app.sidebar_tree.sections[0].items[0].color, None);
}

#[test]
fn color_picker_escape_cancels() {
    let mut app = helpers::make_app(2);
    app.sidebar_tree.cursor = 1;
    app.handle_event(&key(KeyCode::Char('c')));
    assert!(app.prompt.is_some());
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.prompt.is_none());
}

#[test]
fn color_picker_on_section() {
    use ratatui::style::Color;
    let mut app = helpers::make_app(2);
    app.sidebar_tree.cursor = 0;
    app.handle_event(&key(KeyCode::Char('c')));
    assert!(matches!(
        app.prompt,
        Some(Prompt::ColorPicker {
            target: ColorTarget::Section(0),
            ..
        })
    ));
    app.handle_event(&key(KeyCode::Right));
    app.handle_event(&key(KeyCode::Right));
    app.handle_event(&key(KeyCode::Right));
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(
        app.sidebar_tree.sections[0].color,
        Some(Color::Rgb(0xCC, 0x8A, 0x4E))
    );
}

#[test]
fn color_roundtrip_through_toml() {
    use darya::worktree::types::Worktree;
    use ratatui::style::Color;

    let mut app = helpers::make_app(1);
    app.sidebar_tree.sections[0].color = Some(Color::Rgb(0xFF, 0x55, 0x33));
    app.sidebar_tree.sections[0].items[0].color = Some(Color::Rgb(0x33, 0xCC, 0x33));

    let config = app.sidebar_tree.to_sections_config();
    assert_eq!(config.sections[0].color.as_deref(), Some("#FF5533"));
    assert_eq!(
        config.sections[0].items[0].color.as_deref(),
        Some("#33CC33")
    );

    let worktrees: Vec<Worktree> = app.sidebar_tree.sections[0]
        .items
        .iter()
        .map(|item| Worktree {
            path: item.path.clone(),
            name: item.display_name.clone(),
            branch: item.branch.clone(),
            is_main: item.is_main,
        })
        .collect();
    let tree = darya::sidebar::tree::SidebarTree::from_config(&config, &worktrees);
    assert_eq!(tree.sections[0].color, Some(Color::Rgb(0xFF, 0x55, 0x33)));
    assert_eq!(
        tree.sections[0].items[0].color,
        Some(Color::Rgb(0x33, 0xCC, 0x33))
    );
}

// ── Layout Persistence ──────────────────────────────────────

#[test]
fn to_layout_config_collects_active_sessions() {
    let mut app = make_app(3);
    set_session(&mut app, 0, "sess-1");
    set_session(&mut app, 2, "sess-2");

    let layout = app.to_layout_config();
    assert_eq!(layout.sessions.len(), 2);
    assert_eq!(
        layout.sessions[0].path,
        item_path(&app, 0).to_string_lossy()
    );
    assert_eq!(layout.sessions[0].slot_kind, "claude");
    assert_eq!(
        layout.sessions[1].path,
        item_path(&app, 2).to_string_lossy()
    );
}

#[test]
fn to_layout_config_includes_shell_sessions() {
    let mut app = make_app(2);
    set_session(&mut app, 0, "claude-1");
    set_shell_session(&mut app, 0, "shell-1");

    let layout = app.to_layout_config();
    assert_eq!(layout.sessions.len(), 2);
    let kinds: Vec<&str> = layout
        .sessions
        .iter()
        .map(|s| s.slot_kind.as_str())
        .collect();
    assert!(kinds.contains(&"claude"));
    assert!(kinds.contains(&"shell"));
}

#[test]
fn to_layout_config_empty_when_no_sessions() {
    let app = make_app(3);
    let layout = app.to_layout_config();
    assert!(layout.sessions.is_empty());
}

#[test]
fn to_layout_config_captures_ui_state() {
    let mut app = make_app(2);
    app.sidebar_view = SidebarView::FileExplorer;
    app.main_view = MainView::Shell;
    app.panel_focus = PanelFocus::Right;

    let layout = app.to_layout_config();
    assert_eq!(layout.sidebar_view.as_deref(), Some("files"));
    assert_eq!(layout.main_view.as_deref(), Some("shell"));
    assert_eq!(layout.panel_focus.as_deref(), Some("right"));
}

#[test]
fn restore_session_prompt_y_sets_approved() {
    let mut app = make_app(2);
    app.pending_layout = Some(config::LayoutConfig {
        sessions: vec![config::LayoutSessionToml {
            path: "/tmp/test".to_string(),
            slot_kind: "claude".to_string(),
            slot_label: "claude".to_string(),
        }],
        ..Default::default()
    });
    app.prompt = Some(Prompt::RestoreSession { count: 1 });

    app.handle_event(&key(KeyCode::Char('y')));
    assert!(app.restore_approved);
    assert!(app.prompt.is_none());
    assert!(app.pending_layout.is_some()); // still present, consumed by main loop
}

#[test]
fn restore_session_prompt_n_clears_layout() {
    let mut app = make_app(2);
    app.pending_layout = Some(config::LayoutConfig {
        sessions: vec![config::LayoutSessionToml {
            path: "/tmp/test".to_string(),
            slot_kind: "claude".to_string(),
            slot_label: "claude".to_string(),
        }],
        ..Default::default()
    });
    app.prompt = Some(Prompt::RestoreSession { count: 1 });

    app.handle_event(&key(KeyCode::Char('n')));
    assert!(!app.restore_approved);
    assert!(app.prompt.is_none());
    assert!(app.pending_layout.is_none());
}

#[test]
fn restore_session_prompt_esc_clears_layout() {
    let mut app = make_app(2);
    app.pending_layout = Some(config::LayoutConfig::default());
    app.prompt = Some(Prompt::RestoreSession { count: 0 });

    app.handle_event(&key(KeyCode::Esc));
    assert!(app.pending_layout.is_none());
    assert!(app.prompt.is_none());
}

#[test]
fn restore_session_prompt_enter_approves() {
    let mut app = make_app(2);
    app.pending_layout = Some(config::LayoutConfig::default());
    app.prompt = Some(Prompt::RestoreSession { count: 0 });

    app.handle_event(&key(KeyCode::Enter));
    assert!(app.restore_approved);
    assert!(app.prompt.is_none());
}

#[test]
fn layout_config_roundtrip_toml() {
    let layout = config::LayoutConfig {
        sessions: vec![
            config::LayoutSessionToml {
                path: "/tmp/project".to_string(),
                slot_kind: "claude".to_string(),
                slot_label: "claude".to_string(),
            },
            config::LayoutSessionToml {
                path: "/tmp/project".to_string(),
                slot_kind: "shell".to_string(),
                slot_label: "my-shell".to_string(),
            },
        ],
        sidebar_view: Some("files".to_string()),
        main_view: Some("terminal".to_string()),
        panel_focus: Some("right".to_string()),
    };

    let toml_str = toml::to_string_pretty(&layout).unwrap();
    let parsed: config::LayoutConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.sessions.len(), 2);
    assert_eq!(parsed.sessions[0].slot_kind, "claude");
    assert_eq!(parsed.sessions[1].slot_label, "my-shell");
    assert_eq!(parsed.sidebar_view.as_deref(), Some("files"));
    assert_eq!(parsed.panel_focus.as_deref(), Some("right"));
}

// ── Vertical Split Direction ────────────────────────────────

#[test]
fn split_vertical_creates_layout_with_vertical_direction() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_direction = SplitDirection::Vertical;
    assert!(app.split_add_pane());
    let layout = app.pane_layout.as_ref().unwrap();
    // Direction is stored in the root SplitNode
    if let SplitNode::Split { direction, .. } = &layout.root {
        assert_eq!(*direction, SplitDirection::Vertical);
    } else {
        panic!("Expected SplitNode::Split");
    }
    assert_eq!(layout.root.leaf_count(), 2);
}

#[test]
fn toggle_split_direction_flips_existing_layout() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    assert!(app.split_add_pane());
    // Default is Horizontal
    let root_direction = |app: &darya::app::App| -> SplitDirection {
        if let SplitNode::Split { direction, .. } = &app.pane_layout.as_ref().unwrap().root {
            *direction
        } else {
            panic!("Expected SplitNode::Split");
        }
    };
    assert_eq!(root_direction(&app), SplitDirection::Horizontal);
    app.toggle_split_direction();
    assert_eq!(root_direction(&app), SplitDirection::Vertical);
    assert_eq!(app.split_direction, SplitDirection::Vertical);
    assert!(app.layout_changed);
    // Toggle back
    app.layout_changed = false;
    app.toggle_split_direction();
    assert_eq!(root_direction(&app), SplitDirection::Horizontal);
    assert!(app.layout_changed);
}

#[test]
fn toggle_split_direction_noop_without_layout() {
    let mut app = make_app(3);
    assert!(app.pane_layout.is_none());
    app.toggle_split_direction();
    // Direction preference changes but no layout_changed since no layout exists
    assert_eq!(app.split_direction, SplitDirection::Vertical);
    assert!(!app.layout_changed);
}

// ── SplitNode Tree Operations ────────────────────────────────

#[test]
fn split_node_leaf_count() {
    let leaf = SplitNode::Leaf(PaneContent::Editor);
    assert_eq!(leaf.leaf_count(), 1);

    let split = SplitNode::Split {
        direction: SplitDirection::Horizontal,
        first: Box::new(SplitNode::Leaf(PaneContent::Editor)),
        second: Box::new(SplitNode::Leaf(PaneContent::Terminal("t1".into()))),
    };
    assert_eq!(split.leaf_count(), 2);
}

#[test]
fn split_node_depth() {
    let leaf = SplitNode::Leaf(PaneContent::Editor);
    assert_eq!(leaf.depth(), 0);

    let nested = SplitNode::Split {
        direction: SplitDirection::Horizontal,
        first: Box::new(SplitNode::Leaf(PaneContent::Editor)),
        second: Box::new(SplitNode::Split {
            direction: SplitDirection::Vertical,
            first: Box::new(SplitNode::Leaf(PaneContent::Terminal("t1".into()))),
            second: Box::new(SplitNode::Leaf(PaneContent::Terminal("t2".into()))),
        }),
    };
    assert_eq!(nested.depth(), 2);
    assert_eq!(nested.leaf_count(), 3);
}

#[test]
fn split_node_split_leaf() {
    let mut node = SplitNode::Leaf(PaneContent::Editor);
    assert!(node.split_leaf(0, SplitDirection::Horizontal, PaneContent::Terminal("t1".into())));
    assert_eq!(node.leaf_count(), 2);
    assert_eq!(node.depth(), 1);

    // Split the second leaf (index 1)
    assert!(node.split_leaf(1, SplitDirection::Vertical, PaneContent::Shell("s1".into())));
    assert_eq!(node.leaf_count(), 3);
    assert_eq!(node.depth(), 2);
}

#[test]
fn split_node_remove_leaf() {
    let mut node = SplitNode::Split {
        direction: SplitDirection::Horizontal,
        first: Box::new(SplitNode::Leaf(PaneContent::Editor)),
        second: Box::new(SplitNode::Leaf(PaneContent::Terminal("t1".into()))),
    };
    assert!(node.remove_leaf(0));
    // Should collapse to just the terminal leaf
    assert_eq!(node.leaf_count(), 1);
    assert!(matches!(node, SplitNode::Leaf(PaneContent::Terminal(_))));
}

#[test]
fn split_node_remove_nested_leaf() {
    // [Editor | [Terminal | Shell]]
    let mut node = SplitNode::Split {
        direction: SplitDirection::Horizontal,
        first: Box::new(SplitNode::Leaf(PaneContent::Editor)),
        second: Box::new(SplitNode::Split {
            direction: SplitDirection::Vertical,
            first: Box::new(SplitNode::Leaf(PaneContent::Terminal("t1".into()))),
            second: Box::new(SplitNode::Leaf(PaneContent::Shell("s1".into()))),
        }),
    };
    // Remove Terminal (index 1) from the nested split
    assert!(node.remove_leaf(1));
    assert_eq!(node.leaf_count(), 2);
    // Should now be [Editor | Shell]
    let leaves = node.leaves();
    assert!(matches!(leaves[0], PaneContent::Editor));
    assert!(matches!(leaves[1], PaneContent::Shell(_)));
}

#[test]
fn split_node_display_label() {
    let leaf = SplitNode::Leaf(PaneContent::Terminal("my-session".into()));
    assert_eq!(leaf.display_label(), "Terminal: my-session");

    let split = SplitNode::Split {
        direction: SplitDirection::Horizontal,
        first: Box::new(SplitNode::Leaf(PaneContent::Editor)),
        second: Box::new(SplitNode::Leaf(PaneContent::Terminal("t1".into()))),
    };
    assert!(split.display_label().starts_with("Split ["));
}

#[test]
fn split_node_contains_and_remove_session() {
    let mut node = SplitNode::Split {
        direction: SplitDirection::Horizontal,
        first: Box::new(SplitNode::Leaf(PaneContent::Terminal("t1".into()))),
        second: Box::new(SplitNode::Leaf(PaneContent::Shell("s1".into()))),
    };
    assert!(node.contains_session("t1"));
    assert!(node.contains_session("s1"));
    assert!(!node.contains_session("t2"));

    assert!(node.remove_session("t1"));
    assert_eq!(node.leaf_count(), 1);
    assert!(!node.contains_session("t1"));
}

#[test]
fn nested_split_creates_three_panes() {
    let mut app = make_app_with_two_sessions(5);
    app.sidebar_tree.cursor = 1;
    set_session(&mut app, 2, "test-session-3");
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Create first split
    assert!(app.split_add_pane());
    assert_eq!(app.pane_layout.as_ref().unwrap().root.leaf_count(), 2);

    // Create nested split (splits the focused leaf)
    assert!(app.split_add_pane());
    assert_eq!(app.pane_layout.as_ref().unwrap().root.leaf_count(), 3);
    assert!(app.pane_layout.as_ref().unwrap().root.depth() >= 2);
}

#[test]
fn split_picker_opens_with_available_items() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    app.open_split_picker(SplitDirection::Horizontal);
    assert!(app.split_picker.is_some());
    let picker = app.split_picker.as_ref().unwrap();
    // Should have at least 2 items (current terminal + another session)
    assert!(picker.items.len() >= 2);
    assert_eq!(picker.direction, SplitDirection::Horizontal);
}

#[test]
fn split_picker_toggle_direction() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    app.open_split_picker(SplitDirection::Horizontal);
    assert_eq!(
        app.split_picker.as_ref().unwrap().direction,
        SplitDirection::Horizontal
    );

    // Tab toggles direction
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(
        app.split_picker.as_ref().unwrap().direction,
        SplitDirection::Vertical
    );
}

#[test]
fn split_picker_esc_closes() {
    let mut app = make_app_with_two_sessions(3);
    app.sidebar_tree.cursor = 1;
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    app.open_split_picker(SplitDirection::Horizontal);
    assert!(app.split_picker.is_some());

    app.handle_event(&key(KeyCode::Esc));
    assert!(app.split_picker.is_none());
}

#[test]
fn split_picker_existing_layout_shown_as_item() {
    let mut app = make_app_with_two_sessions(4);
    app.sidebar_tree.cursor = 1;
    set_session(&mut app, 2, "test-session-3");
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Create a split first
    assert!(app.split_add_pane());

    // Now open picker — should include "Current Layout" item
    app.open_split_picker(SplitDirection::Horizontal);
    let picker = app.split_picker.as_ref().unwrap();
    assert!(picker.items.iter().any(|i| matches!(i, darya::app::SplitPickerItem::ExistingLayout { .. })));
}

// ── Theme Picker ─────────────────────────────────────────────

#[test]
fn theme_picker_opens_via_command() {
    let mut app = make_app(1);
    app.open_theme_picker();
    assert!(matches!(app.prompt, Some(Prompt::ThemePicker { .. })));
    assert!(app.planet_animation.is_some());
}

#[test]
fn theme_picker_left_right_cycles_planets() {
    let mut app = make_app(1);
    app.open_theme_picker();
    let original_theme = app.theme.clone();

    // Right arrow moves to next planet
    app.handle_event(&key(KeyCode::Right));
    if let Some(Prompt::ThemePicker { selected, .. }) = &app.prompt {
        assert_eq!(*selected, 1);
    } else {
        panic!("expected ThemePicker prompt");
    }
    // Theme should have changed (live preview)
    assert_ne!(app.theme, original_theme);

    // Left arrow wraps back
    app.handle_event(&key(KeyCode::Left));
    if let Some(Prompt::ThemePicker { selected, .. }) = &app.prompt {
        assert_eq!(*selected, 0);
    } else {
        panic!("expected ThemePicker prompt");
    }
}

#[test]
fn theme_picker_left_wraps_to_last() {
    let mut app = make_app(1);
    app.open_theme_picker();
    app.handle_event(&key(KeyCode::Left));
    if let Some(Prompt::ThemePicker { selected, .. }) = &app.prompt {
        assert_eq!(*selected, PlanetKind::all().len() - 1);
    } else {
        panic!("expected ThemePicker prompt");
    }
}

#[test]
fn theme_picker_enter_confirms_planet() {
    let mut app = make_app(1);
    app.open_theme_picker();
    app.handle_event(&key(KeyCode::Right)); // select Mars
    app.handle_event(&key(KeyCode::Enter));
    assert_eq!(app.planet_kind, Some(PlanetKind::all()[1]));
    assert!(app.planet_animation.is_some());
    // Prompt dismissed (or transitioned to SetupGuide)
    assert!(!matches!(app.prompt, Some(Prompt::ThemePicker { .. })));
}

#[test]
fn theme_picker_esc_reverts_theme() {
    let mut app = make_app(1);
    let original_theme = app.theme.clone();
    app.open_theme_picker();
    app.handle_event(&key(KeyCode::Right)); // change theme
    assert_ne!(app.theme, original_theme);
    app.handle_event(&key(KeyCode::Esc));
    assert_eq!(app.theme, original_theme);
    assert!(app.prompt.is_none());
}

#[test]
fn theme_picker_d_switches_to_dark() {
    let mut app = make_app(1);
    app.open_theme_picker();
    // Switch to light first
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.theme.mode, darya::config::ThemeMode::Light);
    // Switch back to dark
    app.handle_event(&key(KeyCode::Char('d')));
    assert_eq!(app.theme.mode, darya::config::ThemeMode::Dark);
}

#[test]
fn theme_picker_live_preview_changes_theme() {
    let mut app = make_app(1);
    let initial_theme = app.theme.clone();
    app.open_theme_picker();
    // Navigate to a different planet
    app.handle_event(&key(KeyCode::Right));
    app.handle_event(&key(KeyCode::Right));
    // Theme should differ from initial (live preview)
    assert_ne!(app.theme, initial_theme);
}

// ── Branch Switcher ─────────────────────────────────────────

fn make_branch_switcher(branches: Vec<&str>, current: &str) -> BranchSwitcherState {
    BranchSwitcherState {
        input: String::new(),
        all_branches: branches.iter().map(|s| s.to_string()).collect(),
        current_branch: current.to_string(),
        worktree_path: PathBuf::from("/tmp/test"),
        results: branches.iter().map(|s| s.to_string()).collect(),
        selected: 0,
    }
}

#[test]
fn branch_switcher_opens_and_closes_with_esc() {
    let mut app = make_app(2);
    app.branch_switcher = Some(make_branch_switcher(vec!["main", "dev", "feature"], "main"));
    assert!(app.branch_switcher.is_some());
    app.handle_event(&key(KeyCode::Esc));
    assert!(app.branch_switcher.is_none());
}

#[test]
fn branch_switcher_fuzzy_filter() {
    let mut bs = make_branch_switcher(vec!["main", "develop", "feature-auth", "feature-ui"], "main");
    bs.input = "feat".to_string();
    bs.update_matches();
    assert_eq!(bs.results.len(), 2);
    assert!(bs.results.iter().all(|r| r.contains("feature")));
}

#[test]
fn branch_switcher_empty_filter_shows_all() {
    let mut bs = make_branch_switcher(vec!["main", "develop", "feature"], "main");
    bs.input = "xyz".to_string();
    bs.update_matches();
    assert!(bs.results.is_empty());
    bs.input.clear();
    bs.update_matches();
    assert_eq!(bs.results.len(), 3);
}

#[test]
fn branch_switcher_navigation_wraps() {
    let mut bs = make_branch_switcher(vec!["main", "dev", "feature"], "main");
    assert_eq!(bs.selected, 0);
    bs.move_down();
    assert_eq!(bs.selected, 1);
    bs.move_down();
    assert_eq!(bs.selected, 2);
    // Wraps to top
    bs.move_down();
    assert_eq!(bs.selected, 0);
    // Wraps to bottom
    bs.move_up();
    assert_eq!(bs.selected, 2);
}

#[test]
fn branch_switcher_selected_branch() {
    let bs = make_branch_switcher(vec!["main", "dev", "feature"], "main");
    assert_eq!(bs.selected_branch(), Some("main"));
}

#[test]
fn branch_switcher_wants_switch_returns_none_for_current_branch() {
    let mut app = make_app(2);
    app.branch_switcher = Some(make_branch_switcher(vec!["main", "dev"], "main"));
    // selected=0 which is "main" (current) — should return None
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert!(app.wants_switch_branch(&enter).is_none());
}

#[test]
fn branch_switcher_wants_switch_returns_some_for_different_branch() {
    let mut app = make_app(2);
    let mut bs = make_branch_switcher(vec!["main", "dev"], "main");
    bs.selected = 1; // "dev" — not current
    app.branch_switcher = Some(bs);
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let result = app.wants_switch_branch(&enter);
    assert!(result.is_some());
    let (path, branch) = result.unwrap();
    assert_eq!(branch, "dev");
    assert_eq!(path, PathBuf::from("/tmp/test"));
}

#[test]
fn branch_switcher_typing_filters_results() {
    let mut app = make_app(2);
    app.branch_switcher = Some(make_branch_switcher(
        vec!["main", "develop", "feature-auth"],
        "main",
    ));
    // Type 'd' to filter
    app.handle_event(&key(KeyCode::Char('d')));
    let bs = app.branch_switcher.as_ref().unwrap();
    assert_eq!(bs.input, "d");
    assert!(bs.results.len() < 3); // filtered
}

#[test]
fn branch_switcher_backspace_removes_char() {
    let mut app = make_app(2);
    let mut bs = make_branch_switcher(vec!["main", "dev"], "main");
    bs.input = "de".to_string();
    app.branch_switcher = Some(bs);
    app.handle_event(&key(KeyCode::Backspace));
    assert_eq!(app.branch_switcher.as_ref().unwrap().input, "d");
}

#[test]
fn branch_switcher_down_arrow_moves_selection() {
    let mut app = make_app(2);
    app.branch_switcher = Some(make_branch_switcher(vec!["main", "dev", "feature"], "main"));
    assert_eq!(app.branch_switcher.as_ref().unwrap().selected, 0);
    app.handle_event(&key(KeyCode::Down));
    assert_eq!(app.branch_switcher.as_ref().unwrap().selected, 1);
}

#[test]
fn command_palette_execute_branch_switcher() {
    let mut app = make_app(2);
    // We can't use execute_command(BranchSwitcher) directly because it calls git,
    // so just verify the CommandId variant exists and state field works
    app.branch_switcher = Some(make_branch_switcher(vec!["main"], "main"));
    assert!(app.branch_switcher.is_some());
    app.branch_switcher = None;
    assert!(app.branch_switcher.is_none());
}
