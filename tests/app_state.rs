mod helpers;

use crossterm::event::KeyCode;

use darya::app::{InputMode, MainView, PanelFocus, Prompt, SidebarView};
use darya::event::AppEvent;

use helpers::{key, ctrl_key, make_app, make_app_with_session};

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
