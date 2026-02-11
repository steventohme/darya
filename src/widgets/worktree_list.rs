use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .worktrees
        .iter()
        .enumerate()
        .map(|(i, wt)| {
            let session_id = app.session_ids.get(&wt.path);
            let has_session = session_id.is_some();
            let needs_attention = session_id
                .map(|id| app.attention_sessions.contains(id))
                .unwrap_or(false);
            let indicator = if has_session { "●" } else { "○" };

            let branch_str = wt
                .branch
                .as_deref()
                .unwrap_or("detached");

            let main_marker = if wt.is_main { " [main]" } else { "" };

            // Hotkey label: 1-9 for first 9, 0 for 10th
            let hotkey = if i < 9 {
                format!("{}", i + 1)
            } else if i == 9 {
                "0".to_string()
            } else {
                " ".to_string()
            };

            let indicator_color = if needs_attention {
                app.theme.session_attention
            } else if has_session {
                app.theme.session_active
            } else {
                app.theme.session_inactive
            };

            let line = if needs_attention {
                let attn = app.theme.session_attention;
                Line::from(vec![
                    Span::styled(
                        format!("{} {} ", hotkey, indicator),
                        Style::default().fg(attn).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{}{}", wt.name, main_marker),
                        Style::default().fg(attn).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {}", branch_str),
                        Style::default().fg(attn),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("{} {} ", hotkey, indicator),
                        Style::default().fg(indicator_color),
                    ),
                    Span::styled(
                        format!("{}{}", wt.name, main_marker),
                        Style::default().fg(app.theme.fg),
                    ),
                    Span::styled(
                        format!("  {}", branch_str),
                        Style::default().fg(app.theme.fg_dim),
                    ),
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let border_style = if app.active_panel == crate::app::Panel::Sidebar {
        Style::default().fg(app.theme.border_active)
    } else {
        Style::default().fg(app.theme.border_inactive)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Worktrees ")
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.selected_worktree));
    frame.render_stateful_widget(list, area, &mut state);
}
