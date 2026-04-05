use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Priority, TodoColumn, TodoEditMode};

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    // Outer border around the whole board
    let outer_block = Block::default()
        .title(" Todo Board ")
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(app.theme.border_style(is_focused));

    let board = match app.todo_board.as_mut() {
        Some(b) => b,
        None => {
            frame.render_widget(outer_block, area);
            return;
        }
    };

    board.clamp_selections();

    let inner = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    // Check if selected item has notes to show
    let selected_notes = {
        let col = board.focused_column;
        let items = board.column_items(col);
        let sel = board.selected[col.index()];
        items.get(sel).and_then(|item| {
            if item.notes.is_empty() {
                None
            } else {
                Some(item.notes.clone())
            }
        })
    };

    // Split: columns on top, optional notes preview at bottom
    let (columns_area, notes_area) = if selected_notes.is_some() {
        let chunks = Layout::vertical([
            Constraint::Min(5),
            Constraint::Length(4),
        ])
        .split(inner);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner, None)
    };

    // Split into 3 columns
    let columns = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .split(columns_area);

    for col in TodoColumn::ALL {
        let col_idx = col.index();
        let col_area = columns[col_idx];
        let col_focused = is_focused && board.focused_column == col;

        let items = board.column_items(col);
        let count = items.len();

        let title = format!(
            " {} ({}) ",
            col.title(),
            count
        );

        let border_style = if col_focused {
            Style::default().fg(app.theme.border_active)
        } else {
            Style::default().fg(app.theme.border_inactive)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(border_style);

        let list_items: Vec<ListItem> = items
            .iter()
            .map(|item| {
                let priority_color = match item.priority {
                    Priority::High => Color::Rgb(0xE0, 0x5A, 0x5A),
                    Priority::Medium => Color::Rgb(0xE0, 0xB0, 0x4A),
                    Priority::Low => app.theme.fg_dim,
                };

                let mut spans = vec![
                    Span::styled(
                        format!("{} ", item.priority.symbol()),
                        Style::default().fg(priority_color),
                    ),
                    Span::styled(&item.title, Style::default().fg(app.theme.fg)),
                ];

                if !item.notes.is_empty() {
                    spans.push(Span::styled(
                        " [+]",
                        Style::default().fg(app.theme.fg_dim),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(list_items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(app.theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        let mut state = ListState::default();
        if count > 0 {
            state.select(Some(board.selected[col_idx]));
        }
        frame.render_stateful_widget(list, col_area, &mut state);
    }

    // Render notes preview
    if let (Some(notes_area), Some(notes)) = (notes_area, selected_notes) {
        let notes_block = Block::default()
            .title(" Notes ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(app.theme.fg_dim));

        let para = Paragraph::new(notes)
            .style(Style::default().fg(app.theme.fg))
            .wrap(Wrap { trim: false })
            .block(notes_block);

        frame.render_widget(para, notes_area);
    }

    // Render editing overlay if active
    if let Some(ref editing) = board.editing {
        render_edit_overlay(frame, area, editing, &app.theme);
    }
}

fn render_edit_overlay(
    frame: &mut Frame,
    area: Rect,
    editing: &TodoEditMode,
    theme: &crate::config::Theme,
) {
    let (label, input) = match editing {
        TodoEditMode::NewTitle { input, .. } => ("New task", input.as_str()),
        TodoEditMode::EditTitle { input, .. } => ("Edit title", input.as_str()),
        TodoEditMode::EditNotes { input, .. } => ("Edit notes", input.as_str()),
    };

    let width = (area.width * 3 / 5).max(30).min(area.width.saturating_sub(4));
    let height = 3;
    let x = (area.width.saturating_sub(width)) / 2 + area.x;
    let y = (area.height.saturating_sub(height)) / 2 + area.y;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {} ", label))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(theme.prompt_border));

    let display = format!("{}█", input);
    let para = Paragraph::new(display)
        .style(Style::default().fg(theme.fg))
        .block(block);

    frame.render_widget(para, popup);
}
