use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    if let Some(ref mut gl) = app.git_log {
        gl.ensure_fresh();
    }

    let border_color = if is_focused {
        app.theme.border_active
    } else {
        app.theme.border_inactive
    };

    let gl = app.git_log.as_mut();
    let (title, items, selected, visible_height) = match gl {
        Some(s) => {
            let title = if let Some(ref file) = s.file_filter {
                format!(" Git Log — {} ({} commits) ", file, s.entries.len())
            } else {
                format!(" Git Log — {} commits ", s.entries.len())
            };

            let block_tmp = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Thick);
            let inner_height = block_tmp.inner(area).height as usize;

            let items: Vec<ListItem> = s
                .entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let is_selected = i == s.selected;
                    let hash_style = Style::default()
                        .fg(app.theme.border_active)
                        .add_modifier(Modifier::BOLD);
                    let subject_color = if is_selected {
                        app.theme.border_active
                    } else {
                        app.theme.fg
                    };

                    let line = Line::from(vec![
                        Span::styled(format!("{} ", entry.hash_short), hash_style),
                        Span::styled(entry.subject.clone(), Style::default().fg(subject_color)),
                        Span::styled(
                            format!("  {} ", entry.author),
                            Style::default().fg(app.theme.fg_dim),
                        ),
                        Span::styled(
                            entry.relative_date.clone(),
                            Style::default().fg(app.theme.fg_dim),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            let selected = s.selected;
            (title, items, selected, inner_height)
        }
        None => (" Git Log — no data ".to_string(), Vec::new(), 0, 0),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg));

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(app.theme.highlight_bg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);

    // Update visible_height for scroll tracking
    if let Some(ref mut gl) = app.git_log {
        gl.visible_height = visible_height;
    }
}
