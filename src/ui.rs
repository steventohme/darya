use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, InputMode, PanelFocus, Prompt, ViewKind};
use crate::session::manager::SessionManager;
use crate::widgets;

/// Compute the right panel Rect (before any pane splitting).
fn right_panel_rect(size: Rect) -> Rect {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(outer[1])[1]
}

/// Compute inner Rects for each pane by splitting the right panel horizontally.
pub fn compute_pane_rects(size: Rect, pane_count: usize) -> Vec<Rect> {
    let panel = right_panel_rect(size);
    if pane_count <= 1 {
        return vec![panel];
    }
    let constraints: Vec<Constraint> = (0..pane_count)
        .map(|_| Constraint::Ratio(1, pane_count as u32))
        .collect();
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(panel)
        .to_vec()
}

/// Compute the inner Rect where the terminal PTY is rendered (single pane).
pub fn compute_pty_rect(size: Rect) -> Rect {
    let panel = right_panel_rect(size);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick);
    block.inner(panel)
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
        ViewKind::GitStatus => widgets::git_status::render(frame, area, app, is_focused),
        ViewKind::DiffView => widgets::diff_view::render(frame, area, app, is_focused),
        ViewKind::GitBlame => widgets::git_blame::render(frame, area, app, is_focused),
        ViewKind::GitLog => widgets::git_log::render(frame, area, app, is_focused),
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

    // Left panel (sidebar)
    let left_focused = app.panel_focus == PanelFocus::Left;
    render_view(frame, main_chunks[0], app.sidebar_view.to_view_kind(), app, session_manager, left_focused);

    // Right panel (main) — split panes for terminal, full panel for everything else
    let right_focused = app.panel_focus == PanelFocus::Right;
    if app.main_view == crate::app::MainView::Terminal {
        if let Some(ref layout) = app.pane_layout {
            if layout.panes.len() > 1 {
                // Split rendering
                let pane_rects = compute_pane_rects(size, layout.panes.len());
                for (i, pane_rect) in pane_rects.iter().enumerate() {
                    if let Some(session_id) = layout.panes.get(i) {
                        let pane_focused = right_focused && i == layout.focused;
                        widgets::terminal_panel::render_session(
                            frame, *pane_rect, app, session_manager, session_id, pane_focused,
                        );
                    }
                }
            } else {
                render_view(frame, main_chunks[1], app.main_view.to_view_kind(), app, session_manager, right_focused);
            }
        } else {
            render_view(frame, main_chunks[1], app.main_view.to_view_kind(), app, session_manager, right_focused);
        }
    } else {
        render_view(frame, main_chunks[1], app.main_view.to_view_kind(), app, session_manager, right_focused);
    }

    // Status bar
    if let Some(ref msg) = app.status_message {
        let status = Paragraph::new(format!(" {} ", msg))
            .style(Style::default().fg(app.theme.warning).bg(app.theme.status_bar_bg));
        frame.render_widget(status, outer[2]);
    } else {
        let bar_width = outer[2].width as usize;

        // Left: mode + view
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
            ViewKind::GitStatus => "git",
            ViewKind::DiffView => "diff",
            ViewKind::GitBlame => "blame",
            ViewKind::GitLog => "log",
        };
        let left = format!(" [{}] {}", mode_str, view_str);

        // Center: branch info
        let center = match app.selected_branch_info() {
            Some((branch, u, m)) => {
                let mut s = branch;
                if m > 0 {
                    s += &format!(" ~{}", m);
                }
                if u > 0 {
                    s += &format!(" +{}", u);
                }
                s
            }
            None => String::new(),
        };

        // Right: session counts
        let running = app.running_session_count();
        let exited = app.exited_session_count();
        let mut right_parts = Vec::new();
        if running > 0 {
            right_parts.push(format!("{} running", running));
        }
        if exited > 0 {
            right_parts.push(format!("{} exited", exited));
        }
        let right = if right_parts.is_empty() {
            String::new()
        } else {
            format!("{} ", right_parts.join(" · "))
        };

        // Build a single line with left-aligned left, centered center, right-aligned right
        let left_len = left.len();
        let right_len = right.len();
        let center_len = center.len();
        let available = bar_width.saturating_sub(left_len + right_len);
        let center_pad_left = available.saturating_sub(center_len) / 2;
        let center_pad_right = available.saturating_sub(center_len + center_pad_left);

        let line = format!(
            "{}{}{}{}{}",
            left,
            " ".repeat(center_pad_left),
            center,
            " ".repeat(center_pad_right),
            right,
        );

        let status = Paragraph::new(line)
            .style(Style::default().fg(app.theme.status_bar_fg).bg(app.theme.status_bar_bg));
        frame.render_widget(status, outer[2]);
    }

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
