use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, InputMode, PanelFocus, Prompt, ViewKind};
use crate::session::manager::SessionManager;
use crate::widgets;

/// Compute the inner Rect where the terminal PTY is rendered.
/// Finds whichever panel (left or right) has the Terminal view and returns its inner rect.
/// If neither panel has Terminal, returns a fallback based on the right panel.
pub fn compute_pty_rect(size: Rect, left_view: ViewKind, right_view: ViewKind) -> Rect {
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

    let chunk = if left_view == ViewKind::Terminal {
        main_chunks[0]
    } else if right_view == ViewKind::Terminal {
        main_chunks[1]
    } else {
        // Terminal not visible — use right panel as fallback for sizing
        main_chunks[1]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick);
    block.inner(chunk)
}

fn render_view(
    frame: &mut Frame,
    area: Rect,
    view: ViewKind,
    app: &mut App,
    session_manager: &SessionManager,
    is_focused: bool,
) {
    match view {
        ViewKind::Worktrees => widgets::worktree_list::render(frame, area, app, is_focused),
        ViewKind::Terminal => widgets::terminal_panel::render(frame, area, app, session_manager, is_focused),
        ViewKind::FileExplorer => widgets::file_explorer::render(frame, area, app, is_focused),
        ViewKind::Editor => widgets::editor::render(frame, area, app, is_focused),
        ViewKind::Search => widgets::search_results::render(frame, area, app, is_focused),
    }
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

    // Main layout: left panel | right panel
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(outer[1]);

    // Left panel
    let left_focused = app.panel_focus == PanelFocus::Left;
    render_view(frame, main_chunks[0], app.left_panel.view, app, session_manager, left_focused);

    // Right panel
    let right_focused = app.panel_focus == PanelFocus::Right;
    render_view(frame, main_chunks[1], app.right_panel.view, app, session_manager, right_focused);

    // Status bar
    let status_text = if let Some(ref msg) = app.status_message {
        format!(" {} ", msg)
    } else {
        let mode_str = match app.input_mode {
            InputMode::Navigation => "NAV",
            InputMode::Terminal => "TERM",
            InputMode::Editor => "EDIT",
        };
        let view_str = match app.focused_view() {
            ViewKind::Worktrees => "worktrees",
            ViewKind::Terminal => "terminal",
            ViewKind::FileExplorer => "files",
            ViewKind::Editor => "editor",
            ViewKind::Search => "search",
        };
        let has_exited_selected = app
            .selected_worktree_path()
            .and_then(|p| app.session_ids.get(p))
            .map(|id| app.exited_sessions.contains(id))
            .unwrap_or(false);
        let restart_hint = if has_exited_selected { "  r:restart" } else { "" };
        let scroll_hint = if app.active_scroll_offset() > 0 {
            "  [scrolled] PgUp/PgDn:scroll"
        } else {
            ""
        };
        format!(
            " [{}] [{}]  q:quit  Tab:switch  Ctrl+1..5:view  Ctrl+P:find  Ctrl+F:search  ?:help{}{}",
            mode_str, view_str, restart_hint, scroll_hint
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

    // Render help overlay if active
    if app.show_help {
        widgets::help_overlay::render(frame, size, app);
    }

    // Render fuzzy finder overlay if active
    if app.fuzzy_finder.is_some() {
        widgets::fuzzy_finder::render(frame, size, app);
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
        Prompt::SearchInput { input } => {
            let block = Block::default()
                .title(" Search project (ripgrep) ")
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
    }
}
