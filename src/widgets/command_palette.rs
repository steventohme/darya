use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref palette) = app.command_palette else {
        return;
    };

    // Centered overlay: ~60% width, ~70% height
    let width = (area.width * 3 / 5).max(40).min(area.width.saturating_sub(4));
    let height = (area.height * 7 / 10).max(10).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Command Palette ")
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
    let input_text = Paragraph::new(format!("> {}█", palette.input))
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

    let items: Vec<ListItem> = palette
        .results
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let is_selected = i == palette.selected;
            let name_style = if is_selected {
                Style::default()
                    .fg(app.theme.border_active)
                    .bg(app.theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg)
            };

            let kb_style = if is_selected {
                Style::default()
                    .fg(app.theme.fg_dim)
                    .bg(app.theme.highlight_bg)
            } else {
                Style::default().fg(app.theme.fg_dim)
            };

            let kb_text = cmd.keybinding.as_deref().unwrap_or("");
            let name_len = cmd.name.len();
            let kb_len = kb_text.len();
            let available = inner.width as usize;
            let padding = available.saturating_sub(name_len + kb_len);

            let line = Line::from(vec![
                Span::styled(&cmd.name, name_style),
                Span::styled(" ".repeat(padding), if is_selected {
                    Style::default().bg(app.theme.highlight_bg)
                } else {
                    Style::default()
                }),
                Span::styled(kb_text, kb_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).style(Style::default().bg(app.theme.bg));
    let mut state = ListState::default().with_selected(Some(palette.selected));
    frame.render_stateful_widget(list, results_area, &mut state);
}
