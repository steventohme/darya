# darya

A terminal workspace for developers who use git worktrees. Manage multiple branches, Claude Code sessions, shell terminals, and files — all from a single TUI.

## Features

- **Git worktree management** — create, delete, and jump between worktrees
- **Split panes** — up to 3 side-by-side terminal sessions
- **Claude Code integration** — launch and manage AI coding sessions per worktree
- **Shell sessions** — run shell terminals alongside Claude Code
- **Built-in file explorer** with git status indicators
- **Editor** with syntax highlighting (via edtui)
- **Git views** — status, diff, blame, and log
- **Fuzzy file finder** and project-wide search (ripgrep)
- **Command palette** for discoverable actions
- **Dark/light themes** with full color customization
- **Layout persistence** — auto-save and restore sessions on restart
- **Custom sections** — organize worktrees into named groups with colors

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
| `Tab` | Cycle between panels and panes |
| `1`–`9`, `0` | Jump to worktree by number |
| `Enter` | Start session, open file, or toggle collapse |
| `?` | Toggle help overlay |
| `q` | Quit |

### Sessions

| Key | Action |
|-----|--------|
| `Enter` | Start or switch to session |
| `r` | Restart exited session |
| `R` | Force-restart session |
| `Backspace` | Close session |
| `S` | Add shell slot |

### Panes & Search

| Key | Action |
|-----|--------|
| `Cmd+\` | Split pane |
| `Cmd+W` | Close pane |
| `Cmd+P` | Fuzzy file finder |
| `Cmd+F` | Project search |
| `Cmd+K` | Command palette |

### Worktrees

| Key | Action |
|-----|--------|
| `a` | Add worktree |
| `d` | Delete worktree |
| `c` | Assign color |
| `N` | Create section |

### Scrolling

| Key | Action |
|-----|--------|
| `Shift+PageUp/Down` | Scroll terminal |
| Mouse wheel | Scroll terminal |

## Configuration

Config lives at `~/.config/darya/config.toml`:

```toml
[theme]
mode = "dark"  # or "light"
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
