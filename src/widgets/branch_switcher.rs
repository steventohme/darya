use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref switcher) = app.branch_switcher else {
        return;
    };

    // Centered overlay: ~60% width, ~60% height
    let width = (area.width * 3 / 5)
        .max(40)
        .min(area.width.saturating_sub(4));
    let height = (area.height * 3 / 5)
        .max(10)
        .min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(
            " Switch Branch ({} branches) ",
            switcher.all_branches.len()
        ))
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
    let input_text = Paragraph::new(format!("> {}\u{2588}", switcher.input)).style(
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

    let items: Vec<ListItem> = switcher
        .results
        .iter()
        .enumerate()
        .map(|(i, branch)| {
            let is_current = branch == &switcher.current_branch;
            let display = if is_current {
                format!("{} (current)", branch)
            } else {
                branch.clone()
            };
            let style = if i == switcher.selected {
                Style::default()
                    .fg(app.theme.border_active)
                    .bg(app.theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default().fg(app.theme.fg_dim)
            } else {
                Style::default().fg(app.theme.fg)
            };
            ListItem::new(Line::from(Span::styled(display, style)))
        })
        .collect();

    let list = List::new(items).style(Style::default().bg(app.theme.bg));
    let mut state = ListState::default().with_selected(Some(switcher.selected));
    frame.render_stateful_widget(list, results_area, &mut state);
}
