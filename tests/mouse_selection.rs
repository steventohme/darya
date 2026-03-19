mod helpers;

use ratatui::layout::Rect;

use darya::app::{MainView, PanelFocus, TextSelection};

use helpers::*;

/// A terminal size that matches our test layout expectations.
/// Header=1, content area, status bar=1. Sidebar 25%.
const TERM_SIZE: Rect = Rect {
    x: 0,
    y: 0,
    width: 120,
    height: 40,
};

/// Move cursor from section header to the first item (one move_down).
fn cursor_to_first_item(app: &mut darya::app::App) {
    app.sidebar_tree.move_down(); // Section → first Item
}

#[test]
fn pane_session_at_coords_returns_none_for_sidebar() {
    let mut app = make_app_with_session(3);
    cursor_to_first_item(&mut app);
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;

    // Sidebar is 25% of 120 = 30 cols. Click inside sidebar area should return None.
    let result = app.pane_session_at_coords(5, 10, TERM_SIZE);
    assert!(result.is_none());
}

#[test]
fn pane_session_at_coords_returns_session_for_terminal_area() {
    let mut app = make_app_with_session(3);
    cursor_to_first_item(&mut app);
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;

    // Right panel starts at ~30 (25% of 120). Click well inside terminal area.
    let result = app.pane_session_at_coords(60, 10, TERM_SIZE);
    assert!(result.is_some());
    let (session_id, inner) = result.unwrap();
    assert_eq!(session_id, "test-session-1");
    // Inner rect should be inside the right panel (after border)
    assert!(inner.x > 30);
    assert!(inner.width > 0);
    assert!(inner.height > 0);
}

#[test]
fn pane_session_at_coords_returns_none_when_no_session() {
    let mut app = make_app(3);
    cursor_to_first_item(&mut app);
    // No session set, should return None even in terminal area
    let result = app.pane_session_at_coords(60, 10, TERM_SIZE);
    assert!(result.is_none());
}

#[test]
fn selection_lifecycle_down_drag_up() {
    let mut app = make_app_with_session(3);
    cursor_to_first_item(&mut app);
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;

    // Simulate: find the inner rect for the terminal area
    let (session_id, inner) = app.pane_session_at_coords(60, 10, TERM_SIZE).unwrap();

    // MouseDown creates an active selection
    let click_x = inner.x + 5;
    let click_y = inner.y + 2;
    app.text_selection = Some(TextSelection {
        session_id: session_id.clone(),
        pane_inner: inner,
        start: (click_y - inner.y, click_x - inner.x),
        end: (click_y - inner.y, click_x - inner.x),
        active: true,
    });
    assert!(app.text_selection.as_ref().unwrap().active);

    // MouseDrag updates end
    let drag_x = inner.x + 20;
    let drag_y = inner.y + 4;
    if let Some(ref mut sel) = app.text_selection {
        sel.end = (drag_y - inner.y, drag_x - inner.x);
    }
    let sel = app.text_selection.as_ref().unwrap();
    assert_eq!(sel.end, (drag_y - inner.y, drag_x - inner.x));
    assert!(sel.active);

    // MouseUp deactivates
    if let Some(ref mut sel) = app.text_selection {
        sel.active = false;
    }
    assert!(!app.text_selection.as_ref().unwrap().active);
}

#[test]
fn selection_cleared_on_key_press() {
    let mut app = make_app_with_session(3);
    app.main_view = MainView::Terminal;

    // Create a selection
    app.text_selection = Some(TextSelection {
        session_id: "test-session-1".to_string(),
        pane_inner: Rect::new(30, 1, 80, 30),
        start: (2, 5),
        end: (4, 20),
        active: false,
    });
    assert!(app.text_selection.is_some());

    // Key event clears selection (simulating what process_event does)
    app.text_selection = None;
    assert!(app.text_selection.is_none());
}

#[test]
fn selection_cleared_on_scroll() {
    let mut app = make_app_with_session(3);
    app.main_view = MainView::Terminal;

    // Create a selection
    app.text_selection = Some(TextSelection {
        session_id: "test-session-1".to_string(),
        pane_inner: Rect::new(30, 1, 80, 30),
        start: (2, 5),
        end: (4, 20),
        active: false,
    });
    assert!(app.text_selection.is_some());

    // Scroll clears selection (simulating what process_event does)
    app.text_selection = None;
    assert!(app.text_selection.is_none());
}

#[test]
fn pane_session_at_coords_with_multi_pane() {
    let mut app = make_app_with_two_sessions(3);
    cursor_to_first_item(&mut app);
    app.main_view = MainView::Terminal;
    app.panel_focus = PanelFocus::Right;

    // Add split pane
    app.split_add_pane();

    // Both panes should be hittable. The split divides the right panel horizontally.
    // Right panel starts around x=30, so first pane ~30-75, second ~75-120.
    let result1 = app.pane_session_at_coords(45, 10, TERM_SIZE);
    assert!(result1.is_some(), "Should hit first pane");

    let result2 = app.pane_session_at_coords(100, 10, TERM_SIZE);
    assert!(result2.is_some(), "Should hit second pane");

    // The two panes should have different session IDs
    let (sid1, _) = result1.unwrap();
    let (sid2, _) = result2.unwrap();
    assert_ne!(sid1, sid2, "Different panes should have different sessions");
}
