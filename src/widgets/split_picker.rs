use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, SplitPickerStep};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref picker) = app.split_picker else {
        return;
    };

    // Centered overlay: ~50% width, ~50% height
    let width = (area.width / 2).max(40).min(area.width.saturating_sub(4));
    let height = (area.height / 2).max(10).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let step_label = match picker.step {
        SplitPickerStep::PickFirst => "pick first window",
        SplitPickerStep::PickSecond => "pick second window",
    };
    let title = format!(" Split ({}): {} ", picker.direction_name(), step_label);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(app.theme.prompt_border))
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 3 {
        return;
    }

    // Footer hint (1 line at bottom)
    let footer_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "Tab",
            Style::default()
                .fg(app.theme.border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" toggle direction  ", Style::default().fg(app.theme.fg_dim)),
        Span::styled(
            "Enter",
            Style::default()
                .fg(app.theme.border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" select  ", Style::default().fg(app.theme.fg_dim)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(app.theme.border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(app.theme.fg_dim)),
    ]))
    .style(Style::default().bg(app.theme.bg));
    frame.render_widget(footer, footer_area);

    // Results list (rest of the space, minus footer)
    let results_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(1),
    );

    let items: Vec<ListItem> = picker
        .visible
        .iter()
        .enumerate()
        .map(|(i, &item_idx)| {
            let item = &picker.items[item_idx];
            let label = item.label();
            let style = if i == picker.selected {
                Style::default()
                    .fg(app.theme.border_active)
                    .bg(app.theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg)
            };
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let list = List::new(items).style(Style::default().bg(app.theme.bg));
    let mut state = ListState::default().with_selected(Some(picker.selected));
    frame.render_stateful_widget(list, results_area, &mut state);
}
