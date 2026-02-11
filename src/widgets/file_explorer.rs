use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    let root_display = app
        .file_explorer
        .root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| app.file_explorer.root.display().to_string());

    let items: Vec<ListItem> = app
        .file_explorer
        .entries
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.depth);
            let (icon, style) = if entry.is_dir {
                let expanded = app.file_explorer.expanded.contains(&entry.path);
                let icon = if expanded { "▾ " } else { "▸ " };
                (icon, Style::default().fg(app.theme.session_active))
            } else {
                ("  ", Style::default().fg(app.theme.fg))
            };

            let line = Line::from(vec![
                Span::styled(format!("{}{}", indent, icon), style),
                Span::styled(&entry.name, style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let border_style = if is_focused {
        Style::default().fg(app.theme.border_active)
    } else {
        Style::default().fg(app.theme.border_inactive)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" {} ", root_display))
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
    state.select(Some(app.file_explorer.selected));
    frame.render_stateful_widget(list, area, &mut state);
}
