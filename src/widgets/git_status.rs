use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::{App, GitFileStatus, GitStatusCategory};
use crate::icons;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    if let Some(ref mut gs) = app.git_status {
        gs.ensure_fresh();
    }

    let border_color = if is_focused { app.theme.border_active } else { app.theme.border_inactive };

    let gs = app.git_status.as_ref();
    let (title, items, selected) = match gs {
        Some(s) => {
            let title = if let Some(ref err) = s.error {
                format!(" Git Status: {} ", err)
            } else {
                format!(" Git Status — {} changes ", s.entries.len())
            };

            let items: Vec<ListItem> = s
                .entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let is_selected = i == s.selected;

                    let status_char = match entry.status {
                        GitFileStatus::Added => "A",
                        GitFileStatus::Modified => "M",
                        GitFileStatus::Deleted => "D",
                        GitFileStatus::Renamed => "R",
                        GitFileStatus::Untracked => "?",
                    };

                    let status_color = match entry.status {
                        GitFileStatus::Added => ratatui::style::Color::Green,
                        GitFileStatus::Modified => ratatui::style::Color::Yellow,
                        GitFileStatus::Deleted => ratatui::style::Color::Red,
                        GitFileStatus::Renamed => ratatui::style::Color::Blue,
                        GitFileStatus::Untracked => app.theme.fg_dim,
                    };

                    let prefix = match entry.category {
                        GitStatusCategory::Staged => "[staged] ",
                        GitStatusCategory::Unstaged => "",
                        GitStatusCategory::Untracked => "",
                    };

                    let filename = entry.path.rsplit('/').next().unwrap_or(&entry.path);
                    let fi = icons::file_icon(filename, false);

                    let line = Line::from(vec![
                        Span::styled(
                            format!("{} ", status_char),
                            Style::default()
                                .fg(status_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{} ", fi.icon),
                            Style::default().fg(fi.color),
                        ),
                        Span::styled(
                            prefix.to_string(),
                            Style::default().fg(app.theme.fg_dim),
                        ),
                        Span::styled(
                            entry.path.clone(),
                            Style::default().fg(if is_selected {
                                app.theme.border_active
                            } else {
                                app.theme.fg
                            }),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            (title, items, s.selected)
        }
        None => (" Git Status — no data ".to_string(), Vec::new(), 0),
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
