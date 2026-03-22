use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    let border_style = app.theme.border_style(is_focused);

    let block = Block::default()
        .title(" Notes ")
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let Some(ref note) = app.note else {
        let hint = Paragraph::new("  select a worktree")
            .style(Style::default().fg(app.theme.fg_dim));
        frame.render_widget(hint, inner);
        return;
    };

    let content = note.content_string();
    if content.trim().is_empty() {
        let keybinding =
            crate::config::KeybindingsConfig::format(&app.keybindings.notes_toggle);
        let hint_line = Line::from(vec![
            Span::styled("  ", Style::default().fg(app.theme.fg_dim)),
            Span::styled(
                keybinding,
                Style::default().fg(app.theme.border_active),
            ),
            Span::styled(
                " to edit",
                Style::default().fg(app.theme.fg_dim),
            ),
        ]);
        let hint = Paragraph::new(hint_line);
        frame.render_widget(hint, inner);
        return;
    }

    // Show first N lines of note content
    let max_lines = inner.height as usize;
    let lines: Vec<Line> = content
        .lines()
        .take(max_lines)
        .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(app.theme.fg_dim))))
        .collect();

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
