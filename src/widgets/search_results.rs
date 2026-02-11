use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    let border_color = if is_focused {
        app.theme.border_active
    } else {
        app.theme.border_inactive
    };

    let search = app.search.as_ref();
    let (title, items, selected) = match search {
        Some(s) => {
            let title = if let Some(ref err) = s.error {
                format!(" Search: {} ", err)
            } else {
                format!(" Search '{}' — {} results ", s.query, s.results.len())
            };

            let items: Vec<ListItem> = s
                .results
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let is_selected = i == s.selected;
                    let line = Line::from(vec![
                        Span::styled(
                            format!("{}:{}", r.file_relative, r.line_number),
                            Style::default()
                                .fg(if is_selected {
                                    app.theme.border_active
                                } else {
                                    app.theme.fg_dim
                                })
                                .add_modifier(if is_selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                        Span::styled(
                            format!(" {}", r.line_text.trim()),
                            Style::default().fg(app.theme.fg),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            (title, items, s.selected)
        }
        None => (" Search — no query ".to_string(), Vec::new(), 0),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg));

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}
