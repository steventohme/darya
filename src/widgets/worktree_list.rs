use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .worktrees
        .iter()
        .enumerate()
        .map(|(_i, wt)| {
            let has_session = app.session_ids.contains_key(&wt.path);
            let indicator = if has_session { "●" } else { "○" };

            let branch_str = wt
                .branch
                .as_deref()
                .unwrap_or("detached");

            let main_marker = if wt.is_main { " [main]" } else { "" };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", indicator),
                    Style::default().fg(if has_session {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{}{}", wt.name, main_marker),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("  {}", branch_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let border_style = if app.active_panel == crate::app::Panel::Sidebar {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Worktrees ")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.selected_worktree));
    frame.render_stateful_widget(list, area, &mut state);
}
