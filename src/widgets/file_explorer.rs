use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::{App, GitFileStatus};
use crate::icons;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    // Lazily refresh git indicators only when the file explorer is visible
    app.file_explorer.ensure_git_indicators();

    let root_display = app
        .file_explorer
        .root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| app.file_explorer.root.display().to_string());

    let root = &app.file_explorer.root;
    let items: Vec<ListItem> = app
        .file_explorer
        .entries
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.depth);
            let expanded = entry.is_dir && app.file_explorer.expanded.contains(&entry.path);
            let fi = if entry.is_dir {
                if expanded { icons::dir_icon_open() } else { icons::file_icon(&entry.name, true) }
            } else {
                icons::file_icon(&entry.name, false)
            };
            let name_style = if entry.is_dir {
                Style::default().fg(app.theme.session_active)
            } else {
                Style::default().fg(app.theme.fg)
            };

            let marker = if entry.is_dir {
                // O(1) lookup in pre-computed dirty dirs set
                let dir_rel = entry.path
                    .strip_prefix(&root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                if app.file_explorer.dirty_dirs.contains(&dir_rel) {
                    Span::styled(" ●", Style::default().fg(Color::DarkGray))
                } else {
                    Span::raw("")
                }
            } else {
                let rel = entry.path
                    .strip_prefix(&root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                match app.file_explorer.git_indicators.get(&rel) {
                    Some(GitFileStatus::Added) => Span::styled(" A", Style::default().fg(Color::Green)),
                    Some(GitFileStatus::Modified) => Span::styled(" M", Style::default().fg(Color::Yellow)),
                    Some(GitFileStatus::Deleted) => Span::styled(" D", Style::default().fg(Color::Red)),
                    Some(GitFileStatus::Renamed) => Span::styled(" R", Style::default().fg(Color::Blue)),
                    Some(GitFileStatus::Untracked) => Span::styled(" ?", Style::default().fg(Color::DarkGray)),
                    None => Span::raw(""),
                }
            };

            let line = if entry.is_dir {
                let arrow = if expanded { "▾" } else { "▸" };
                Line::from(vec![
                    Span::styled(format!("{}{} ", indent, arrow), name_style),
                    Span::styled(format!("{} ", fi.icon), Style::default().fg(fi.color)),
                    Span::styled(&entry.name, name_style),
                    marker,
                ])
            } else {
                Line::from(vec![
                    Span::styled(indent, name_style),
                    Span::styled(format!("{} ", fi.icon), Style::default().fg(fi.color)),
                    Span::styled(&entry.name, name_style),
                    marker,
                ])
            };
            ListItem::new(line)
        })
        .collect();

    let border_style = app.theme.border_style(is_focused);

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
