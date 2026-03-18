use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, DiffLineKind};

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    let border_style = app.theme.border_style(is_focused);

    let Some(ref mut dv) = app.diff_view else {
        // No diff loaded — render placeholder
        let block = Block::default()
            .title(" Diff ")
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(border_style)
            .style(Style::default().bg(app.theme.bg));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let hint = Paragraph::new("  Select a file from Git Status to view diff")
            .style(Style::default().fg(app.theme.fg_dim));
        frame.render_widget(hint, inner);
        return;
    };

    // Update visible height for scroll calculations
    let block = Block::default()
        .title(format!(" Diff: {} ", dv.file_path))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(border_style)
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(area);
    dv.visible_height = inner.height as usize;
    frame.render_widget(block, area);

    let lines: Vec<Line> = dv
        .lines
        .iter()
        .skip(dv.scroll_offset)
        .take(inner.height as usize)
        .map(|dl| {
            let style = match dl.kind {
                DiffLineKind::Header => Style::default()
                    .fg(app.theme.border_active)
                    .add_modifier(Modifier::BOLD),
                DiffLineKind::Addition => Style::default().fg(ratatui::style::Color::Green),
                DiffLineKind::Deletion => Style::default().fg(ratatui::style::Color::Red),
                DiffLineKind::Context => Style::default().fg(app.theme.fg_dim),
            };
            Line::from(Span::styled(dl.content.clone(), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines).style(Style::default().bg(app.theme.bg));
    frame.render_widget(paragraph, inner);
}
