use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    let border_style = if is_focused {
        Style::default().fg(app.theme.border_active)
    } else {
        Style::default().fg(app.theme.border_inactive)
    };

    let Some(ref mut gb) = app.git_blame else {
        let block = Block::default()
            .title(" Blame ")
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(border_style)
            .style(Style::default().bg(app.theme.bg));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let hint = Paragraph::new("  Open a file in editor, then press b or Ctrl+7")
            .style(Style::default().fg(app.theme.fg_dim));
        frame.render_widget(hint, inner);
        return;
    };

    let block = Block::default()
        .title(format!(" Blame: {} ", gb.file_path))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(border_style)
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(area);
    gb.visible_height = inner.height as usize;
    frame.render_widget(block, area);

    let lines: Vec<Line> = gb
        .lines
        .iter()
        .skip(gb.scroll_offset)
        .take(inner.height as usize)
        .map(|bl| {
            let hash_style = if bl.is_recent {
                Style::default().fg(app.theme.border_active)
            } else {
                Style::default().fg(app.theme.fg_dim)
            };
            let author_truncated = if bl.author.len() > 12 {
                format!("{:.12}", bl.author)
            } else {
                format!("{:<12}", bl.author)
            };
            let date_truncated = if bl.relative_time.len() > 12 {
                format!("{:.12}", bl.relative_time)
            } else {
                format!("{:<12}", bl.relative_time)
            };

            Line::from(vec![
                Span::styled(format!("{} ", bl.commit_short), hash_style),
                Span::styled(
                    format!("{} ", author_truncated),
                    Style::default().fg(app.theme.fg_dim),
                ),
                Span::styled(
                    format!("{} ", date_truncated),
                    hash_style,
                ),
                Span::styled(
                    format!("{:>4} ", bl.line_number),
                    Style::default()
                        .fg(app.theme.fg_dim)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(bl.content.clone(), Style::default().fg(app.theme.fg)),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines).style(Style::default().bg(app.theme.bg));
    frame.render_widget(paragraph, inner);
}
