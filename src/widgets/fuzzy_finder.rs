use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref finder) = app.fuzzy_finder else {
        return;
    };

    // Centered overlay: ~60% width, ~60% height
    let width = (area.width * 3 / 5).max(40).min(area.width.saturating_sub(4));
    let height = (area.height * 3 / 5).max(10).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(" Find file ({} files) ", finder.all_files.len()))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(app.theme.prompt_border))
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 2 {
        return;
    }

    // Input field (1 line at top)
    let input_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let input_text = Paragraph::new(format!("> {}█", finder.input))
        .style(
            Style::default()
                .fg(app.theme.fg)
                .bg(app.theme.bg)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(input_text, input_area);

    // Results list (rest of the space)
    let results_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(1),
    );

    let items: Vec<ListItem> = finder
        .results
        .iter()
        .enumerate()
        .map(|(i, (display, _))| {
            let style = if i == finder.selected {
                Style::default()
                    .fg(app.theme.border_active)
                    .bg(app.theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg)
            };
            ListItem::new(Line::from(Span::styled(display.as_str(), style)))
        })
        .collect();

    let list = List::new(items).style(Style::default().bg(app.theme.bg));
    let mut state = ListState::default().with_selected(Some(finder.selected));
    frame.render_stateful_widget(list, results_area, &mut state);
}
