use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, InputMode, Panel, Prompt};
use crate::session::manager::SessionManager;
use crate::widgets;

/// Compute the inner Rect where the terminal PTY is rendered.
/// Replicates the layout (header/main/status + sidebar/terminal split + border).
pub fn compute_pty_rect(size: Rect) -> Rect {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(outer[1]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick);
    block.inner(main_chunks[1])
}

pub fn draw(frame: &mut Frame, app: &mut App, session_manager: &SessionManager) {
    let size = frame.area();

    // Fill background with theme color
    let bg_block = Block::default().style(Style::default().bg(app.theme.bg));
    frame.render_widget(bg_block, size);

    // Full layout: header + content area + status bar
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    // Header
    let header = Paragraph::new(" darya")
        .style(
            Style::default()
                .fg(app.theme.border_active)
                .bg(app.theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(header, outer[0]);

    // Main layout: sidebar | terminal
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(outer[1]);

    // Sidebar
    widgets::worktree_list::render(frame, main_chunks[0], app);

    // Terminal panel
    widgets::terminal_panel::render(frame, main_chunks[1], app, session_manager);

    // Status bar
    let status_text = if let Some(ref msg) = app.status_message {
        format!(" {} ", msg)
    } else {
        let mode_str = match app.input_mode {
            InputMode::Navigation => "NAV",
            InputMode::Terminal => "TERM",
        };
        let panel_str = match app.active_panel {
            Panel::Sidebar => "sidebar",
            Panel::Terminal => "terminal",
        };
        format!(
            " [{}] [{}]  q:quit  Tab:switch  j/k:navigate  a:add  d:delete  Enter:session  Esc:back",
            mode_str, panel_str
        )
    };
    let status_style = if app.status_message.is_some() {
        Style::default().fg(app.theme.warning).bg(app.theme.status_bar_bg)
    } else {
        Style::default().fg(app.theme.status_bar_fg).bg(app.theme.status_bar_bg)
    };
    let status = Paragraph::new(status_text).style(status_style);
    frame.render_widget(status, outer[2]);

    // Render prompt overlay if active
    if let Some(ref prompt) = app.prompt {
        render_prompt(frame, size, prompt, &app.theme);
    }
}

fn render_prompt(frame: &mut Frame, area: Rect, prompt: &Prompt, theme: &crate::config::Theme) {
    let width = 50u16.min(area.width.saturating_sub(4));
    let height = 3u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    match prompt {
        Prompt::CreateWorktree { input } => {
            let block = Block::default()
                .title(" New worktree (branch name) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(theme.prompt_border))
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = Paragraph::new(format!("{}█", input))
                .style(Style::default().fg(theme.fg).bg(theme.bg).add_modifier(Modifier::BOLD));
            frame.render_widget(text, inner);
        }
        Prompt::ConfirmDelete { worktree_name } => {
            let block = Block::default()
                .title(" Confirm Delete ")
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(theme.prompt_delete_border))
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = Paragraph::new(format!("Delete '{}'? (y/N)", worktree_name))
                .style(Style::default().fg(theme.fg).bg(theme.bg));
            frame.render_widget(text, inner);
        }
    }
}
