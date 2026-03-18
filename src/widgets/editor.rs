use edtui::{EditorTheme, EditorView, LineNumbers, SyntaxHighlighter};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::config::ThemeMode;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    let border_style = app.theme.border_style(is_focused);

    let Some(ref mut editor) = app.editor else {
        // No file open — render placeholder
        let block = Block::default()
            .title(" Editor ")
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let hint = Paragraph::new("  Open a file from the file explorer (Enter)")
            .style(Style::default().fg(app.theme.fg_dim));
        frame.render_widget(hint, inner);
        return;
    };

    // Build title
    let modified_indicator = if editor.modified { " [+]" } else { "" };
    let mode_label = if editor.read_only { "VIEW" } else { "EDIT" };
    let title = format!(" {} {}{} ", editor.file_name(), mode_label, modified_indicator);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build syntax highlighter
    let syntax_hl = if !editor.file_extension.is_empty() {
        let theme_name = match app.theme.mode {
            ThemeMode::Dark => "base16-ocean-dark",
            ThemeMode::Light => "base16-ocean-light",
        };
        SyntaxHighlighter::new(theme_name, &editor.file_extension).ok()
    } else {
        None
    };

    // Build editor theme
    let editor_theme = EditorTheme::default()
        .base(Style::default().fg(app.theme.fg).bg(app.theme.bg))
        .cursor_style(
            Style::default()
                .fg(app.theme.bg)
                .bg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        )
        .selection_style(Style::default().fg(app.theme.bg).bg(app.theme.border_active))
        .line_numbers_style(Style::default().fg(app.theme.fg_dim))
        .hide_status_line();

    let view = EditorView::new(&mut editor.editor_state)
        .theme(editor_theme)
        .syntax_highlighter(syntax_hl)
        .line_numbers(LineNumbers::Absolute);

    frame.render_widget(view, inner);
}
