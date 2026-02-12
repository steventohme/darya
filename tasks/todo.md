# Phase 11: Split Terminal Panes

## Steps
- [x] Step 1: Add PaneLayout struct + pane methods to src/app.rs
- [x] Step 2: Add split_pane/close_pane keybinding config to src/config.rs
- [x] Step 3: Extract render_session() in src/widgets/terminal_panel.rs
- [x] Step 4: Add compute_pane_rects() to src/ui.rs + update draw() for split rendering
- [x] Step 5: Wire keybindings + update input routing in src/main.rs
- [x] Step 6: Add resize_all_except() to src/session/manager.rs
- [x] Step 7: Update help overlay with split pane keybindings
- [x] Step 8: Add 12 state machine tests + 2 snapshot tests
- [x] Step 9: Verify all tests pass + accept snapshots

## Review Notes
- All 12 new state machine tests pass (split_add_pane_creates_layout, split_add_pane_fails_without_other_sessions, split_add_pane_caps_at_three, close_focused_pane_collapses_to_single, close_focused_pane_adjusts_focus, cycle_pane_focus_wraps, focused_session_id_single_vs_split, is_session_visible_split_mode, split_preserves_across_view_switch, split_requires_terminal_view, remove_session_from_panes_collapses, alt_h_l_cycle_pane_focus_in_terminal_nav)
- 2 new snapshot tests accepted: split_two_panes, split_focused_pane_highlighted
- 1 existing snapshot updated: help_overlay_terminal (added pane keybindings)
- 2 existing snapshots updated: worktree_list_3_items, status_bar_with_sessions (render_session now shows worktree name as title)
- Pre-existing test race in file_changed tests (shared temp file) — not caused by this change
- `cargo check` clean with zero warnings
