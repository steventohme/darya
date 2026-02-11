use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Panel};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let (title, bindings) = match app.active_panel {
        Panel::Sidebar => (
            "Navigation — Sidebar",
            vec![
                ("j/k, ↑/↓", "Navigate worktrees"),
                ("1-9, 0", "Jump to worktree"),
                ("Enter", "Start / focus session"),
                ("a", "Add worktree"),
                ("d", "Delete worktree"),
                ("r", "Restart exited session"),
                ("Tab", "Switch to terminal"),
                ("q", "Quit"),
                ("Ctrl+C", "Close session / Quit"),
            ],
        ),
        Panel::Terminal => (
            "Navigation — Terminal",
            vec![
                ("i, Enter", "Enter terminal mode"),
                ("PgUp/PgDn", "Scroll output"),
                ("1-9, 0", "Jump to worktree"),
                ("Tab", "Switch to sidebar"),
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
    let width = 40u16.min(area.width.saturating_sub(4));
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
