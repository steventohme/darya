mod helpers;

use std::path::PathBuf;

use crossterm::event::KeyCode;

use darya::app::{
    is_edtui_compatible, status_priority, EditorViewState,
    GitStatusCategory, GitStatusEntry, GitStatusState, GitFileStatus,
    InputMode, MainView, PanelFocus, Prompt, SidebarView,
};
use darya::config;
use darya::event::AppEvent;

use helpers::{key, ctrl_key, make_app, make_app_with_session, make_app_with_two_sessions};

// ── Navigation ──────────────────────────────────────────────

#[test]
fn nav_j_moves_worktree_selection_down() {
    let mut app = make_app(3);
    assert_eq!(app.selected_worktree, 0);
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(app.selected_worktree, 1);
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(app.selected_worktree, 2);
}

#[test]
fn nav_k_moves_worktree_selection_up() {
    let mut app = make_app(3);
    app.selected_worktree = 2;
    app.handle_event(&key(KeyCode::Char('k')));
    assert_eq!(app.selected_worktree, 1);
}

#[test]
fn nav_j_wraps_around() {
    let mut app = make_app(3);
    app.selected_worktree = 2;
    app.handle_event(&key(KeyCode::Char('j')));
    assert_eq!(app.selected_worktree, 0);
}

#[test]
fn nav_k_wraps_around() {
    let mut app = make_app(3);
    assert_eq!(app.selected_worktree, 0);
    app.handle_event(&key(KeyCode::Char('k')));
    assert_eq!(app.selected_worktree, 2);
}

#[test]
fn nav_down_arrow_works_like_j() {
    let mut app = make_app(3);
    app.handle_event(&key(KeyCode::Down));
    assert_eq!(app.selected_worktree, 1);
}

#[test]
fn nav_number_keys_jump_to_worktree() {
    let mut app = make_app(5);
    app.handle_event(&key(KeyCode::Char('3')));
    assert_eq!(app.selected_worktree, 2);
    app.handle_event(&key(KeyCode::Char('1')));
    assert_eq!(app.selected_worktree, 0);
}

#[test]
fn nav_zero_jumps_to_tenth_worktree() {
    let mut app = make_app(11);
    app.handle_event(&key(KeyCode::Char('0')));
    assert_eq!(app.selected_worktree, 9);
}

#[test]
fn nav_number_beyond_count_is_noop() {
    let mut app = make_app(2);
    app.handle_event(&key(KeyCode::Char('5')));
    assert_eq!(app.selected_worktree, 0); // unchanged
}

// ── Mode transitions ────────────────────────────────────────

#[test]
fn terminal_mode_tab_returns_to_nav() {
    let mut app = make_app_with_session(3);
    app.input_mode = InputMode::Terminal;
    app.panel_focus = PanelFocus::Right;
    app.handle_event(&key(KeyCode::Tab));
    assert_eq!(app.input_mode, InputMode::Navigation);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn enter_terminal_mode_from_terminal_nav() {
    let mut app = make_app_with_session(3);
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
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    // No active session
    assert!(app.active_session_id.is_none());
    app.handle_event(&key(KeyCode::Char('i')));
    assert_eq!(app.input_mode, InputMode::Navigation);
}

#[test]
fn cannot_enter_terminal_on_exited_session() {
    let mut app = make_app_with_session(3);
    let sid = app.active_session_id.clone().unwrap();
    app.exited_sessions.insert(sid);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.handle_event(&key(KeyCode::Char('i')));
    assert_eq!(app.input_mode, InputMode::Navigation);
}

// ── Panel switching ─────────────────────────────────────────

#[test]
fn ctrl_1_sets_sidebar_worktrees() {
    let mut app = make_app(3);
    app.sidebar_view = SidebarView::FileExplorer;
    app.panel_focus = PanelFocus::Right;
    app.handle_event(&ctrl_key('1'));
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn ctrl_2_sets_main_terminal() {
    let mut app = make_app(3);
    app.main_view = MainView::Editor;
    app.panel_focus = PanelFocus::Left;
    app.handle_event(&ctrl_key('2'));
    assert_eq!(app.main_view, MainView::Terminal);
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn ctrl_3_sets_sidebar_files() {
    let mut app = make_app(3);
    app.handle_event(&ctrl_key('3'));
    assert_eq!(app.sidebar_view, SidebarView::FileExplorer);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

#[test]
fn ctrl_4_sets_main_editor() {
    let mut app = make_app(3);
    app.handle_event(&ctrl_key('4'));
    assert_eq!(app.main_view, MainView::Editor);
    assert_eq!(app.panel_focus, PanelFocus::Right);
}

#[test]
fn ctrl_5_sets_sidebar_search() {
    let mut app = make_app(3);
    app.handle_event(&ctrl_key('5'));
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
    let sid = app.active_session_id.clone().unwrap();
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
    app.input_mode = InputMode::Terminal;
    app.handle_event(&AppEvent::SessionExited {
        session_id: "other-session".to_string(),
    });
    assert_eq!(app.input_mode, InputMode::Terminal);
}

#[test]
fn session_bell_marks_attention_unless_active_and_terminal() {
    let mut app = make_app_with_session(2);
    let sid = app.active_session_id.clone().unwrap();

    // In terminal mode viewing the session — no attention
    app.input_mode = InputMode::Terminal;
    app.handle_event(&AppEvent::SessionBell {
        session_id: sid.clone(),
    });
    assert!(!app.attention_sessions.contains(&sid));

    // In nav mode — should mark attention
    app.input_mode = InputMode::Navigation;
    app.handle_event(&AppEvent::SessionBell {
        session_id: sid.clone(),
    });
    assert!(app.attention_sessions.contains(&sid));
}

#[test]
fn session_bell_other_session_always_marks_attention() {
    let mut app = make_app_with_session(2);
    app.input_mode = InputMode::Terminal;
    let other = "other-session".to_string();
    app.handle_event(&AppEvent::SessionBell {
        session_id: other.clone(),
    });
    assert!(app.attention_sessions.contains(&other));
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
    app.selected_worktree = 1; // non-main
    app.handle_event(&key(KeyCode::Char('d')));
    assert!(matches!(app.prompt, Some(Prompt::ConfirmDelete { .. })));
}

#[test]
fn d_on_main_worktree_shows_error() {
    let mut app = make_app(3);
    app.selected_worktree = 0; // main
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
    let before = app.selected_worktree;
    app.handle_event(&AppEvent::Tick);
    assert_eq!(app.selected_worktree, before);
}

// ── Scroll ──────────────────────────────────────────────────

#[test]
fn scroll_up_and_down() {
    let mut app = make_app_with_session(1);
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
    app.scroll_up(2000);
    assert_eq!(app.active_scroll_offset(), 1000);
}

#[test]
fn reset_scroll_clears_offset() {
    let mut app = make_app_with_session(1);
    app.scroll_up(50);
    assert_eq!(app.active_scroll_offset(), 50);
    app.reset_scroll();
    assert_eq!(app.active_scroll_offset(), 0);
}

// ── Tab auto-enters terminal ────────────────────────────────

#[test]
fn tab_from_worktrees_with_active_session_enters_terminal_mode() {
    let mut app = make_app_with_session(2);
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
    let app = make_app(2);
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert!(app.needs_session_spawn(&key_event));
}

#[test]
fn needs_session_spawn_false_when_prompt_active() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::CreateWorktree { input: String::new() });
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_spawn(&key_event));
}

#[test]
fn needs_session_restart_on_r_with_exited_session() {
    let mut app = make_app_with_session(2);
    let sid = app.active_session_id.clone().unwrap();
    app.exited_sessions.insert(sid);
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Char('r'), crossterm::event::KeyModifiers::NONE);
    assert!(app.needs_session_restart(&key_event));
}

#[test]
fn needs_session_restart_false_without_exited() {
    let app = make_app_with_session(2);
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Char('r'), crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_restart(&key_event));
}

// ── needs_session_close ─────────────────────────────────────

#[test]
fn needs_session_close_on_backspace_with_session() {
    let app = make_app_with_session(2);
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(app.needs_session_close(&key_event));
}

#[test]
fn needs_session_close_false_without_session() {
    let app = make_app(2);
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_close(&key_event));
}

#[test]
fn needs_session_close_false_in_terminal_mode() {
    let mut app = make_app_with_session(2);
    app.input_mode = InputMode::Terminal;
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_close(&key_event));
}

#[test]
fn needs_session_close_false_when_prompt_active() {
    let mut app = make_app_with_session(2);
    app.prompt = Some(Prompt::CreateWorktree { input: String::new() });
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
    assert!(!app.needs_session_close(&key_event));
}

// ── wants_create/delete worktree ────────────────────────────

#[test]
fn wants_create_worktree_returns_input_on_enter() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::CreateWorktree {
        input: "my-branch".to_string(),
    });
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert_eq!(app.wants_create_worktree(&key_event), Some("my-branch".to_string()));
}

#[test]
fn wants_create_worktree_none_on_empty_input() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::CreateWorktree {
        input: String::new(),
    });
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
    assert_eq!(app.wants_create_worktree(&key_event), None);
}

#[test]
fn wants_delete_worktree_on_y() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::ConfirmDelete {
        worktree_name: "test".to_string(),
    });
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Char('y'), crossterm::event::KeyModifiers::NONE);
    assert!(app.wants_delete_worktree(&key_event));
}

#[test]
fn wants_delete_worktree_false_on_n() {
    let mut app = make_app(2);
    app.prompt = Some(Prompt::ConfirmDelete {
        worktree_name: "test".to_string(),
    });
    let key_event = crossterm::event::KeyEvent::new(KeyCode::Char('n'), crossterm::event::KeyModifiers::NONE);
    assert!(!app.wants_delete_worktree(&key_event));
}

// ── Git Status view switching ───────────────────────────────

#[test]
fn ctrl_6_sets_sidebar_git_status() {
    let mut app = make_app(3);
    app.sidebar_view = SidebarView::Worktrees;
    app.panel_focus = PanelFocus::Right;
    app.handle_event(&ctrl_key('6'));
    assert_eq!(app.sidebar_view, SidebarView::GitStatus);
    assert_eq!(app.panel_focus, PanelFocus::Left);
}

fn make_app_with_git_status(n: usize) -> darya::app::App {
    let mut app = make_app(n);
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
        worktree_path: app.worktrees[0].path.clone(),
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
    assert_eq!(app.selected_worktree, 1);
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
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.sidebar_view, SidebarView::FileExplorer);
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.sidebar_view, SidebarView::Search);
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.sidebar_view, SidebarView::GitStatus);
    app.handle_event(&key(KeyCode::Char('l')));
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
}

#[test]
fn h_cycles_sidebar_backward() {
    let mut app = make_app(3);
    assert_eq!(app.sidebar_view, SidebarView::Worktrees);
    app.handle_event(&key(KeyCode::Char('h')));
    assert_eq!(app.sidebar_view, SidebarView::GitStatus);
    app.handle_event(&key(KeyCode::Char('h')));
    assert_eq!(app.sidebar_view, SidebarView::Search);
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
    let sid = app.active_session_id.clone().unwrap();
    assert!(!app.activity.is_active(&sid));
    // Output + tick (no recent input) → active
    app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
    app.handle_event(&AppEvent::Tick);
    assert!(app.activity.is_active(&sid));
}

#[test]
fn output_suppressed_after_user_input() {
    let mut app = make_app_with_session(2);
    let sid = app.active_session_id.clone().unwrap();
    // Simulate user typing: mark_input then echo arrives as PtyOutput
    app.activity.mark_input(&sid);
    app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
    app.handle_event(&AppEvent::Tick);
    // Should NOT activate — the output was just an echo
    assert!(!app.activity.is_active(&sid));
}

#[test]
fn tick_advances_animation_position() {
    let mut app = make_app_with_session(2);
    let sid = app.active_session_id.clone().unwrap();
    app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
    app.handle_event(&AppEvent::Tick);
    let pos_before = app.activity.position(&sid);
    // Two ticks needed to advance (100ms per frame via parity skip)
    app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
    app.handle_event(&AppEvent::Tick);
    app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
    app.handle_event(&AppEvent::Tick);
    let pos_after = app.activity.position(&sid);
    assert_ne!(pos_before, pos_after);
}

#[test]
fn animation_bounce_cycle() {
    let mut app = make_app_with_session(2);
    let sid = app.active_session_id.clone().unwrap();
    // Initial output to start animation
    app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
    app.handle_event(&AppEvent::Tick);

    // Collect positions over a full 8-frame bounce cycle
    // Each frame takes 2 ticks (100ms) due to parity skip
    let mut positions = Vec::new();
    for _ in 0..8 {
        positions.push(app.activity.position(&sid));
        // Two ticks per frame advance
        app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
        app.handle_event(&AppEvent::Tick);
        app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
        app.handle_event(&AppEvent::Tick);
    }
    assert_eq!(positions, vec![0, 1, 2, 3, 4, 3, 2, 1]);
}

#[test]
fn session_exited_cleans_up_animation() {
    let mut app = make_app_with_session(2);
    let sid = app.active_session_id.clone().unwrap();
    app.handle_event(&AppEvent::PtyOutput { session_id: sid.clone() });
    app.handle_event(&AppEvent::Tick);
    assert!(app.activity.is_active(&sid));
    app.handle_event(&AppEvent::SessionExited { session_id: sid.clone() });
    assert!(!app.activity.is_active(&sid));
}

#[test]
fn animation_independent_per_session() {
    let mut app = make_app_with_session(3);
    // Add a second session for the second worktree
    let wt2_path = app.worktrees[1].path.clone();
    let sid2 = "test-session-2".to_string();
    app.session_ids.insert(wt2_path, sid2.clone());

    let sid1 = app.active_session_id.clone().unwrap();

    // Only activate session 1
    app.handle_event(&AppEvent::PtyOutput { session_id: sid1.clone() });
    app.handle_event(&AppEvent::Tick);
    assert!(app.activity.is_active(&sid1));
    assert!(!app.activity.is_active(&sid2));

    // Now activate session 2 too
    app.handle_event(&AppEvent::PtyOutput { session_id: sid2.clone() });
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
    // BackTab should be silently ignored, not panic
    app.handle_event(&key(KeyCode::BackTab));
    assert_eq!(app.input_mode, InputMode::Editor);
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
    // Compatible keys
    assert!(is_edtui_compatible(&KeyCode::Char('a')));
    assert!(is_edtui_compatible(&KeyCode::Enter));
    assert!(is_edtui_compatible(&KeyCode::Backspace));
    assert!(is_edtui_compatible(&KeyCode::Tab));
    assert!(is_edtui_compatible(&KeyCode::Esc));
    assert!(is_edtui_compatible(&KeyCode::Left));
    assert!(is_edtui_compatible(&KeyCode::Right));
    assert!(is_edtui_compatible(&KeyCode::Up));
    assert!(is_edtui_compatible(&KeyCode::Down));
    assert!(is_edtui_compatible(&KeyCode::Home));
    assert!(is_edtui_compatible(&KeyCode::End));
    assert!(is_edtui_compatible(&KeyCode::Delete));
    assert!(is_edtui_compatible(&KeyCode::PageUp));
    assert!(is_edtui_compatible(&KeyCode::PageDown));
    assert!(is_edtui_compatible(&KeyCode::F(1)));
    assert!(is_edtui_compatible(&KeyCode::F(12)));

    // Incompatible keys that would cause edtui to panic
    assert!(!is_edtui_compatible(&KeyCode::BackTab));
    assert!(!is_edtui_compatible(&KeyCode::Null));
    assert!(!is_edtui_compatible(&KeyCode::Insert));
    assert!(!is_edtui_compatible(&KeyCode::F(13)));
    assert!(!is_edtui_compatible(&KeyCode::CapsLock));
}

// ── Session counts / branch info helpers ─────────────────────

#[test]
fn running_session_count_excludes_exited() {
    let mut app = make_app(3);
    let p0 = app.worktrees[0].path.clone();
    let p1 = app.worktrees[1].path.clone();
    let p2 = app.worktrees[2].path.clone();
    app.session_ids.insert(p0, "s0".to_string());
    app.session_ids.insert(p1, "s1".to_string());
    app.session_ids.insert(p2, "s2".to_string());
    app.exited_sessions.insert("s1".to_string());
    assert_eq!(app.running_session_count(), 2);
}

#[test]
fn exited_session_count_only_counts_exited() {
    let mut app = make_app(3);
    let p0 = app.worktrees[0].path.clone();
    let p1 = app.worktrees[1].path.clone();
    let p2 = app.worktrees[2].path.clone();
    app.session_ids.insert(p0, "s0".to_string());
    app.session_ids.insert(p1, "s1".to_string());
    app.session_ids.insert(p2, "s2".to_string());
    app.exited_sessions.insert("s1".to_string());
    assert_eq!(app.exited_session_count(), 1);
}

#[test]
fn selected_branch_info_returns_branch_and_counts() {
    let mut app = make_app(2);
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
        worktree_path: app.worktrees[0].path.clone(),
    });
    let (branch, untracked, modified) = app.selected_branch_info().unwrap();
    assert_eq!(branch, "main");
    assert_eq!(untracked, 1);
    assert_eq!(modified, 2); // staged + unstaged
}

#[test]
fn selected_branch_info_without_git_status_returns_zeros() {
    let app = make_app(2);
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
    app.handle_event(&AppEvent::FileChanged { paths: vec![tmp.clone()] });
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
    app.handle_event(&AppEvent::FileChanged { paths: vec![tmp.clone()] });
    // Editor should still have original content
    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.editor_state.lines.to_string(), "original\n");
    assert!(editor.modified);
    assert!(app.status_message.as_deref().unwrap().contains("unsaved edits preserved"));
}

#[test]
fn file_changed_ignores_unrelated_path() {
    let (mut app, _tmp, _dir) = make_app_with_open_file("original\n");
    let unrelated = PathBuf::from("/tmp/some_other_file.txt");
    app.handle_event(&AppEvent::FileChanged { paths: vec![unrelated] });
    // No status message, content unchanged
    assert!(app.status_message.is_none());
    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.editor_state.lines.to_string(), "original\n");
}

#[test]
fn file_changed_without_editor_is_noop() {
    let mut app = make_app(2);
    assert!(app.editor.is_none());
    app.handle_event(&AppEvent::FileChanged { paths: vec![PathBuf::from("/tmp/foo.txt")] });
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
    assert!(app.file_explorer.entries.iter().any(|e| e.name == "new_file.txt"));
}

#[test]
fn file_changed_identical_content_no_message() {
    let (mut app, tmp, _dir) = make_app_with_open_file("same content\n");
    // File on disk is identical to editor content — no rewrite needed, just send event
    app.handle_event(&AppEvent::FileChanged { paths: vec![tmp.clone()] });
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
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    assert!(app.pane_layout.is_none());
    assert!(app.split_add_pane());
    let layout = app.pane_layout.as_ref().unwrap();
    assert_eq!(layout.panes.len(), 2);
    assert_eq!(layout.focused, 0);
    // First pane is the active session, second is the next available
    assert_eq!(layout.panes[0], "test-session-1");
    assert_eq!(layout.panes[1], "test-session-2");
}

#[test]
fn split_add_pane_fails_without_other_sessions() {
    let mut app = make_app_with_session(2);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    // Only one session exists
    assert!(!app.split_add_pane());
    assert!(app.pane_layout.is_none());
    assert!(app.status_message.as_ref().unwrap().contains("No other running"));
}

#[test]
fn split_add_pane_caps_at_three() {
    let mut app = make_app_with_two_sessions(4);
    // Add a third session
    let wt2 = app.worktrees[2].path.clone();
    app.session_ids.insert(wt2, "test-session-3".to_string());
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    assert!(app.split_add_pane()); // 2 panes
    assert!(app.split_add_pane()); // 3 panes
    assert_eq!(app.pane_layout.as_ref().unwrap().panes.len(), 3);
    assert!(!app.split_add_pane()); // max reached
    assert!(app.status_message.as_ref().unwrap().contains("Maximum 3"));
}

#[test]
fn close_focused_pane_collapses_to_single() {
    let mut app = make_app_with_two_sessions(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    assert!(app.pane_layout.is_some());
    app.close_focused_pane();
    assert!(app.pane_layout.is_none());
    // active_session_id should be set to remaining session
    assert!(app.active_session_id.is_some());
}

#[test]
fn close_focused_pane_adjusts_focus() {
    let mut app = make_app_with_two_sessions(4);
    let wt2 = app.worktrees[2].path.clone();
    app.session_ids.insert(wt2, "test-session-3".to_string());
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    app.split_add_pane();
    // Focus the last pane
    let layout = app.pane_layout.as_mut().unwrap();
    layout.focused = 2;
    app.close_focused_pane();
    let layout = app.pane_layout.as_ref().unwrap();
    assert_eq!(layout.panes.len(), 2);
    assert!(layout.focused < layout.panes.len());
}

#[test]
fn cycle_pane_focus_wraps() {
    let mut app = make_app_with_two_sessions(3);
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
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;

    // Single mode: returns active_session_id
    assert_eq!(app.focused_session_id(), Some(&"test-session-1".to_string()));

    // Split mode: returns pane-focused session
    app.split_add_pane();
    assert_eq!(app.focused_session_id(), Some(&"test-session-1".to_string()));
    app.cycle_pane_focus_next();
    assert_eq!(app.focused_session_id(), Some(&"test-session-2".to_string()));
}

#[test]
fn is_session_visible_split_mode() {
    let mut app = make_app_with_two_sessions(4);
    let wt2 = app.worktrees[2].path.clone();
    app.session_ids.insert(wt2, "test-session-3".to_string());
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
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    let before = app.pane_layout.clone();

    // Switch to editor and back
    app.main_view = MainView::Editor;
    app.main_view = MainView::Terminal;
    assert_eq!(app.pane_layout.as_ref().unwrap().panes, before.unwrap().panes);
}

#[test]
fn split_requires_terminal_view() {
    let mut app = make_app_with_two_sessions(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Editor;
    assert!(!app.split_add_pane());
    assert!(app.status_message.as_ref().unwrap().contains("terminal view"));
}

#[test]
fn remove_session_from_panes_collapses() {
    let mut app = make_app_with_two_sessions(3);
    app.panel_focus = PanelFocus::Right;
    app.main_view = MainView::Terminal;
    app.split_add_pane();
    assert!(app.pane_layout.is_some());

    app.remove_session_from_panes("test-session-2");
    // Should collapse to single since only 1 pane remains
    assert!(app.pane_layout.is_none());
    assert_eq!(app.active_session_id.as_deref(), Some("test-session-1"));
}

#[test]
fn tab_cycles_panes_then_sidebar_in_terminal_nav() {
    let mut app = make_app_with_two_sessions(3);
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
    app.file_explorer.git_indicators.insert("foo.rs".to_string(), GitFileStatus::Modified);
    assert!(!app.file_explorer.git_indicators.is_empty());
    // Set root to a non-git temp path — indicators should be cleared
    let tmp = tempfile::tempdir().unwrap();
    app.file_explorer.set_root(tmp.path().to_path_buf());
    assert!(app.file_explorer.git_indicators.is_empty());
}

#[test]
fn file_changed_refreshes_git_indicators() {
    // Ensure no panic on a non-git directory
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app(2);
    app.file_explorer.set_root(tmp.path().to_path_buf());
    app.handle_event(&AppEvent::FileChanged { paths: vec![tmp.path().join("a.txt")] });
    // Should not panic, indicators stay empty for non-git dir
    assert!(app.file_explorer.git_indicators.is_empty());
}

#[test]
fn files_created_or_deleted_refreshes_git_indicators() {
    // Ensure no panic on a non-git directory
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app(2);
    app.file_explorer.set_root(tmp.path().to_path_buf());
    app.handle_event(&AppEvent::FilesCreatedOrDeleted);
    assert!(app.file_explorer.git_indicators.is_empty());
}
