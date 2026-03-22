use edtui::{EditorTheme, EditorView, LineNumbers, SyntaxHighlighter};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::config::ThemeMode;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    let border_style = app.theme.border_style(is_focused);

    // Check if we're in edit mode (need mutable access to note)
    let is_editing = app.note.as_ref().map_or(false, |n| !n.read_only);

    if is_editing {
        render_editor(frame, area, app, border_style);
    } else {
        render_preview(frame, area, app, border_style);
    }
}

fn render_editor(frame: &mut Frame, area: Rect, app: &mut App, border_style: Style) {
    let Some(ref mut note) = app.note else {
        return;
    };

    let modified_indicator = if note.modified { " [+]" } else { "" };
    let title = format!(" Notes — EDIT{} ", modified_indicator);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let syntax_hl = {
        let theme_name = match app.theme.mode {
            ThemeMode::Dark => "base16-ocean-dark",
            ThemeMode::Light => "base16-ocean-light",
        };
        SyntaxHighlighter::new(theme_name, "md").ok()
    };

    let editor_theme = EditorTheme::default()
        .base(Style::default().fg(app.theme.fg).bg(app.theme.bg))
        .cursor_style(
            Style::default()
                .fg(app.theme.bg)
                .bg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        )
        .selection_style(
            Style::default()
                .fg(app.theme.bg)
                .bg(app.theme.border_active),
        )
        .line_numbers_style(Style::default().fg(app.theme.fg_dim))
        .hide_status_line();

    let view = EditorView::new(&mut note.editor_state)
        .theme(editor_theme)
        .syntax_highlighter(syntax_hl)
        .line_numbers(LineNumbers::Absolute);

    frame.render_widget(view, inner);
}

fn render_preview(frame: &mut Frame, area: Rect, app: &App, border_style: Style) {
    let block = Block::default()
        .title(" Notes ")
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let Some(ref note) = app.note else {
        let hint = Paragraph::new("  select a worktree")
            .style(Style::default().fg(app.theme.fg_dim));
        frame.render_widget(hint, inner);
        return;
    };

    let content = note.content_string();
    if content.trim().is_empty() {
        let keybinding =
            crate::config::KeybindingsConfig::format(&app.keybindings.notes_toggle);
        let hint_line = Line::from(vec![
            Span::styled("  ", Style::default().fg(app.theme.fg_dim)),
            Span::styled(
                keybinding,
                Style::default().fg(app.theme.border_active),
            ),
            Span::styled(
                " to edit",
                Style::default().fg(app.theme.fg_dim),
            ),
        ]);
        let hint = Paragraph::new(hint_line);
        frame.render_widget(hint, inner);
        return;
    }

    // Show first N lines of note content
    let max_lines = inner.height as usize;
    let lines: Vec<Line> = content
        .lines()
        .take(max_lines)
        .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(app.theme.fg_dim))))
        .collect();

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
