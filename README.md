# darya

A terminal workspace for developers who use git worktrees. Manage multiple branches, Claude Code sessions, shell terminals, and files — all from a single TUI.

## Features

### Workspace

- **Git worktree management** — create, delete, and jump between worktrees
- **Split panes** — up to 3 side-by-side or stacked terminal sessions (horizontal & vertical)
- **Custom sections** — organize worktrees into named groups with custom colors
- **Layout persistence** — auto-save and restore sessions on restart
- **Resizable sidebar** — adjust with `Cmd+=` / `Cmd+-` (15%–50%)

### Sessions

- **Claude Code integration** — launch and manage AI coding sessions per worktree
- **Shell sessions** — run multiple named shell terminals per worktree
- **Session status** — live status text from PTY window titles displayed in sidebar
- **Activity indicators** — animated scanner for active sessions, bell detection, exit status
- **Force-restart** — restart running or exited sessions without closing them

### File Explorer

- **Tree view** with expand/collapse directories
- **File icons** with language-specific colors (Rust, Python, Go, TypeScript, and more)
- **Git status indicators** — dirty files and directories highlighted
- **Text selection** — click-drag to select terminal text, auto-copy via OSC 52

### Git Views

- **Status** — staged, unstaged, and untracked files with color-coded indicators
- **Diff** — syntax-highlighted diffs for staged, unstaged, and untracked files
- **Blame** — commit hash, author, and relative time per line
- **Log** — last 200 commits with hash, subject, author, and date; select to view full diff

### Editor

- **Built-in editor** via edtui with syntax highlighting
- **Edit mode** (`e`) and save (`Ctrl+S`)

### Search

- **Fuzzy file finder** (`Cmd+P`) with live filtering
- **Project-wide search** (`Cmd+F`) powered by ripgrep with line numbers

### Themes

- **6 planet themes** — Earth, Mars, Venus, Neptune, Jupiter, Pluto
- **Dark and light modes** — each planet adapts to both
- **Theme picker** — browse with live planet animation preview
- **Color picker** — assign custom colors to sections, worktrees, and sessions
- **Full color customization** — override any color via hex values in config

### Other

- **Command palette** (`Cmd+K`) — searchable list of all actions with keybindings
- **Help overlay** (`?`) — quick reference for all shortcuts
- **First-launch setup guide** — helps configure iTerm2 keybindings
- **Bracketed paste** — paste multi-line content into terminal sessions
- **Mouse support** — scroll, click, and drag in terminal views
- **Kitty keyboard protocol** — Ctrl+number keys work natively

## Install

```bash
cargo install --path .
```

Requires Rust 1.70+. Designed for **macOS with iTerm2**.

## Quick Start

```bash
# Run from any git repo with worktrees
cd ~/projects/my-repo
darya
```

On first launch, a setup guide will help configure iTerm2 keybindings.

## Recommended: Remap Caps Lock

Darya uses **Caps Lock** as the primary panel-switch key. Since terminals can't capture Caps Lock directly, you need to remap it to F18 at the OS level:

1. Install [Karabiner-Elements](https://karabiner-elements.pqrs.org/)
2. Open Karabiner → **Simple Modifications**
3. Add a rule: **caps_lock → f18**

This gives you a fast, ergonomic key for switching between the sidebar and terminal without reaching for modifier combos.

## Keyboard Shortcuts

All `Cmd+` shortcuts require iTerm2 to not intercept them. See the setup guide or rebind in config.

### Views

| Key | View |
|-----|------|
| `Cmd+1` | Worktrees |
| `Cmd+2` | Terminal |
| `Cmd+3` | Files |
| `Cmd+4` | Editor |
| `Cmd+5` | Search |
| `Cmd+6` | Git Status |
| `Cmd+7` | Git Blame |
| `Cmd+8` | Git Log |
| `Cmd+9` | Shell |

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `h` / `l` | Collapse / expand (sidebar) |
| `CapsLock` | Cycle between panels and panes |
| `Shift+CapsLock` | Cycle sub-views within panel |
| `1`–`9`, `0` | Jump to worktree by number |
| `Enter` | Start session, open file, or toggle collapse |
| `?` | Toggle help overlay |
| `q` | Quit |

### Sessions

| Key | Action |
|-----|--------|
| `Enter` | Start or switch to session |
| `r` | Restart exited session |
| `Shift+R` | Force-restart session |
| `Backspace` | Close session |
| `Shift+S` | Add shell slot |

### Panes

| Key | Action |
|-----|--------|
| `Cmd+\` | Split pane (horizontal) |
| `Cmd+Shift+\` | Split pane (vertical) |
| `Cmd+W` | Close pane |
| `CapsLock` | Cycle between panes |

### Search & Commands

| Key | Action |
|-----|--------|
| `Cmd+P` | Fuzzy file finder |
| `Cmd+F` | Project search |
| `Cmd+K` | Command palette |

### Worktrees

| Key | Action |
|-----|--------|
| `a` | Add worktree |
| `d` | Delete worktree |
| `c` | Assign color |
| `Shift+N` | Create section |
| `Backspace` | Delete section |

### Sidebar

| Key | Action |
|-----|--------|
| `Cmd+=` | Grow sidebar |
| `Cmd+-` | Shrink sidebar |

### Scrolling

| Key | Action |
|-----|--------|
| `Shift+PageUp/Down` | Scroll terminal |
| `PageUp/Down` | Scroll views |
| Mouse wheel | Scroll terminal |

### Input Modes

| Key | Action |
|-----|--------|
| `i` / `Enter` | Enter terminal mode (keys → PTY) |
| `Esc` | Exit terminal mode |
| `e` | Enter editor edit mode |
| `Ctrl+S` | Save file (editor) |

## Configuration

Config lives at `~/.config/darya/config.toml`:

```toml
[theme]
mode = "dark"        # or "light"
planet = "earth"     # earth, mars, venus, neptune, jupiter, pluto
show_planet = true   # show planet animation in sidebar
# Override any color with hex values:
# border_active = "#E07A2A"
# bg = "#1A1A1A"

[terminal]
start_at_bottom = true

[session]
command = "claude"  # command to run for Claude sessions

[shell]
command = "/bin/zsh"  # defaults to $SHELL

[keybindings]
# Rebind any shortcut:
# worktrees = "cmd+1"
# terminal = "cmd+2"
# fuzzy_finder = "cmd+p"
# split_pane_vertical = "cmd+shift+\\"
# sidebar_grow = "cmd+="
# sidebar_shrink = "cmd+-"

[worktree]
dir_format = "{repo}-{branch}"

[layout]
auto_resume = false  # restore sessions on restart
```

### Per-Worktree Overrides

Create a `.darya.toml` in any worktree root to override session/shell commands:

```toml
[session]
command = "claude --dangerously-skip-permissions"

[shell]
command = "/bin/bash"
```

## Architecture

Built with:
- [ratatui](https://ratatui.rs) + crossterm for the TUI
- [tui-term](https://github.com/a-kenji/tui-term) + portable-pty for terminal emulation
- [edtui](https://github.com/preiter93/edtui) for the built-in editor
- tokio for async event handling

## License

[MIT](LICENSE)
