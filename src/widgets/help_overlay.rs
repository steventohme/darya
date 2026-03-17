use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, ViewKind};
use crate::config::KeybindingsConfig;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let kb = &app.keybindings;
    let kb_worktrees = KeybindingsConfig::format(&kb.worktrees);
    let kb_terminal = KeybindingsConfig::format(&kb.terminal);
    let kb_files = KeybindingsConfig::format(&kb.files);
    let kb_editor = KeybindingsConfig::format(&kb.editor);
    let kb_search = KeybindingsConfig::format(&kb.search);
    let kb_fuzzy = KeybindingsConfig::format(&kb.fuzzy_finder);
    let kb_proj_search = KeybindingsConfig::format(&kb.project_search);

    let kb_git = KeybindingsConfig::format(&kb.git_status);
    let kb_split = KeybindingsConfig::format(&kb.split_pane);
    let kb_close_pane = KeybindingsConfig::format(&kb.close_pane);
    let kb_blame = KeybindingsConfig::format(&kb.git_blame);
    let kb_log = KeybindingsConfig::format(&kb.git_log);
    let kb_cmd_palette = KeybindingsConfig::format(&kb.command_palette);
    let kb_shell = KeybindingsConfig::format(&kb.shell);

    let view_bindings = format!(
        "{}: Worktrees  {}: Terminal  {}: Files  {}: Editor  {}: Search  {}: Git  {}: Blame  {}: Log  {}: Shell  {}: Palette",
        kb_worktrees, kb_terminal, kb_files, kb_editor, kb_search, kb_git, kb_blame, kb_log, kb_shell, kb_cmd_palette
    );

    let (title, bindings): (&str, Vec<(&str, &str)>) = match app.focused_view() {
        ViewKind::Worktrees => (
            "Navigation — Worktrees",
            vec![
                ("j/k, ↑/↓", "Navigate tree"),
                ("1-9, 0", "Jump to item"),
                ("Enter", "Toggle collapse / start session"),
                ("l/→", "Expand / enter"),
                ("h/←", "Collapse / parent"),
                ("a", "Add worktree"),
                ("d", "Delete worktree"),
                ("r", "Restart exited session"),
                ("Shift+R", "Force-restart session"),
                ("Shift+S", "Add shell slot"),
                ("c", "Assign color"),
                ("Shift+N", "Create section"),
                ("Backspace", "Close/delete session/section"),
                ("Tab", "Switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
                ("Ctrl+C", "Close session / Quit"),
            ],
        ),
        ViewKind::Terminal => (
            "Navigation — Terminal",
            vec![
                ("i, Enter", "Enter terminal mode"),
                ("PgUp/PgDn", "Scroll output"),
                (&kb_split, "Split: same type"),
                (&kb_close_pane, "Close pane"),
                ("", "Palette: Split Terminal/Shell/Editor"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Cycle panes / switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
                ("Ctrl+C", "Close session / Quit"),
            ],
        ),
        ViewKind::FileExplorer => (
            "Navigation — Files",
            vec![
                ("j/k, ↑/↓", "Navigate files"),
                ("Enter, l", "Expand dir / open file"),
                ("h", "Collapse dir / go to parent"),
                ("Backspace", "Go up one directory"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
            ],
        ),
        ViewKind::Editor => (
            "Navigation — Editor",
            vec![
                ("e", "Enter edit mode"),
                ("Esc", "Exit edit mode"),
                ("Ctrl+S", "Save file"),
                ("j/k", "Navigate lines"),
                ("", "Palette: Split Terminal/Shell/Editor"),
                ("Tab", "Cycle panes / switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
            ],
        ),
        ViewKind::Search => (
            "Navigation — Search",
            vec![
                ("j/k, ↑/↓", "Navigate results"),
                ("Enter", "Open file at match"),
                ("Tab", "Switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "New search"),
                ("q", "Quit"),
                ("Ctrl+C", "Close session / Quit"),
            ],
        ),
        ViewKind::GitStatus => (
            "Navigation — Git Status",
            vec![
                ("j/k, ↑/↓", "Navigate files"),
                ("Enter, d", "View diff"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
            ],
        ),
        ViewKind::DiffView => (
            "Navigation — Diff View",
            vec![
                ("j/k, ↑/↓", "Scroll line"),
                ("PgUp/PgDn", "Scroll page"),
                ("Esc", "Back to terminal"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
            ],
        ),
        ViewKind::GitBlame => (
            "Navigation — Git Blame",
            vec![
                ("j/k, ↑/↓", "Scroll line"),
                ("PgUp/PgDn", "Scroll page"),
                ("Esc", "Back to editor"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
            ],
        ),
        ViewKind::GitLog => (
            "Navigation — Git Log",
            vec![
                ("j/k, ↑/↓", "Navigate commits"),
                ("Enter", "View commit diff"),
                ("Esc", "Back to terminal"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
            ],
        ),
        ViewKind::Shell => (
            "Navigation — Shell",
            vec![
                ("i, Enter", "Enter terminal mode"),
                ("PgUp/PgDn", "Scroll output"),
                (&kb_split, "Split: same type"),
                (&kb_close_pane, "Close pane"),
                ("", "Palette: Split Terminal/Shell/Editor"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Cycle panes / switch panel"),
                (&kb_fuzzy, "Fuzzy file finder"),
                (&kb_proj_search, "Project search"),
                ("q", "Quit"),
                ("Ctrl+C", "Close session / Quit"),
            ],
        ),
    };

    let key_width = 12;
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(" {}", title),
        Style::default()
            .fg(app.theme.border_active)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        " ─────────────────────────────────────",
        Style::default().fg(app.theme.fg_dim),
    )));

    lines.push(Line::from(Span::styled(
        format!(" {}", view_bindings),
        Style::default().fg(app.theme.fg_dim),
    )));
    lines.push(Line::from(""));

    for (key, desc) in &bindings {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:width$}", key, width = key_width),
                Style::default()
                    .fg(app.theme.border_active)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(*desc, Style::default().fg(app.theme.fg)),
        ]));
    }

    let content_height = lines.len() as u16;
    let width = 60u16.min(area.width.saturating_sub(4));
    let height = (content_height + 2).min(area.height.saturating_sub(2)); // +2 for border
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Help (? to close) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(app.theme.prompt_border))
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let paragraph = Paragraph::new(lines).style(Style::default().bg(app.theme.bg));
    frame.render_widget(paragraph, inner);
}
