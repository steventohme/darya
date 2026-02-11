# Darya - Phase 1 Implementation

## Step 1: Project Scaffolding
- [x] cargo init + Cargo.toml with all deps
- [x] Create module directory structure with stub mod.rs files
- [x] Verify: `cargo check` passes

## Step 2: Minimal TUI Shell
- [x] main.rs: tokio runtime, terminal init/restore, run loop
- [x] event.rs: EventHandler with crossterm EventStream + tick timer
- [x] app.rs: basic App state, quit on q/Ctrl+c
- [x] ui.rs: horizontal split layout rendering empty bordered panels
- [x] Verify: app starts, shows split panes, quits cleanly

## Step 3: Worktree Sidebar
- [x] worktree/types.rs: Worktree struct
- [x] worktree/manager.rs: parse `git worktree list --porcelain`
- [x] widgets/worktree_list.rs: List widget with j/k navigation
- [x] Wire into app state and render
- [x] Verify: sidebar shows real worktrees, can navigate

## Step 4: Single PTY Session
- [x] session/pty_session.rs: spawn Claude Code via portable-pty
- [x] session/manager.rs: create session, get active
- [x] widgets/terminal_panel.rs: render tui-term PseudoTerminal
- [x] Enter on a worktree spawns Claude Code
- [x] Verify: Claude Code output appears in terminal panel

## Step 5: Terminal Input Mode
- [x] Navigation <-> Terminal mode toggle (Ctrl+\)
- [x] key_event_to_bytes(): convert KeyEvent -> raw bytes for PTY
- [x] Forward keys to PTY writer in Terminal mode
- [x] Status bar shows NAV/TERM mode (inline in ui.rs)
- [x] Verify: can type to Claude Code and see responses

## Step 6: Multi-Session Switching
- [x] SessionManager holds HashMap of sessions
- [x] Selecting different worktree switches displayed session
- [x] Session indicators in sidebar (●/○)
- [x] Verify: two worktrees, two Claude sessions, switch between them

## Step 7: Worktree Create/Delete
- [x] WorktreeManager::add() with input prompt (popup overlay)
- [x] WorktreeManager::remove() with confirmation
- [x] Session cleanup on delete
- [x] Verify: create/delete worktree from TUI

## Step 8: Resize + Polish
- [x] Handle terminal resize events → resize all PTY sessions
- [x] Handle Claude Code process exit gracefully (no unwrap)
- [x] Panic-safe terminal restoration (panic hook)
- [x] Error handling for git/PTY failures (status bar messages)
- [x] Empty worktree list edge case
- [x] Minimum PTY size bounds
- [x] Clean build with zero warnings

## Review Notes
- Upgraded from ratatui 0.29 → 0.30 for tui-term 0.3 compatibility
- Upgraded crossterm 0.28 → 0.29 to match ratatui 0.30
- vt100 0.16 API change: `set_size` moved from Parser to Screen (`parser.screen_mut().set_size()`)
- Block type mismatch workaround: render border Block separately, PseudoTerminal without block
