use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::app::App;
use crate::session::manager::SessionManager;

/// Render a single terminal session into the given area with its own border.
pub fn render_session(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    session_manager: &SessionManager,
    session_id: &str,
    is_focused: bool,
) {
    let border_style = if is_focused {
        Style::default().fg(app.theme.border_active)
    } else {
        Style::default().fg(app.theme.border_inactive)
    };

    let title = app
        .worktree_name_for_session(session_id)
        .unwrap_or("terminal");

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(session) = session_manager.get(session_id) {
        if let Ok(mut parser) = session.parser.write() {
            let offset = app.scroll_offset_for(session_id);
            parser.screen_mut().set_scrollback(offset);
            let pseudo_term = PseudoTerminal::new(parser.screen());
            frame.render_widget(pseudo_term, inner);

            // Post-process: replace Color::Reset with theme colors.
            let buf = frame.buffer_mut();
            for y in inner.y..inner.y + inner.height {
                for x in inner.x..inner.x + inner.width {
                    let cell = &mut buf[(x, y)];
                    if cell.bg == Color::Reset {
                        cell.bg = app.theme.bg;
                    }
                    if cell.fg == Color::Reset {
                        cell.fg = app.theme.fg;
                    }
                }
            }

            // Bottom-align: shift content rows to the bottom of the widget
            if app.terminal_start_bottom && inner.height > 0 {
                let bottom = inner.y + inner.height - 1;
                let mut last_content_row = inner.y;
                for y in (inner.y..=bottom).rev() {
                    let mut has_content = false;
                    for x in inner.x..inner.x + inner.width {
                        let sym = buf[(x, y)].symbol();
                        if sym != " " && !sym.is_empty() {
                            has_content = true;
                            break;
                        }
                    }
                    if has_content {
                        last_content_row = y;
                        break;
                    }
                }

                let shift = bottom - last_content_row;
                if shift > 0 {
                    for y in (inner.y..=last_content_row).rev() {
                        let dst_y = y + shift;
                        for x in inner.x..inner.x + inner.width {
                            let cell = buf[(x, y)].clone();
                            buf[(x, dst_y)] = cell;
                        }
                    }
                    for y in inner.y..inner.y + shift {
                        for x in inner.x..inner.x + inner.width {
                            let cell = &mut buf[(x, y)];
                            cell.reset();
                            cell.fg = app.theme.fg;
                            cell.bg = app.theme.bg;
                        }
                    }
                }
            }

            // Show scroll indicator when scrolled back
            if offset > 0 && inner.height > 0 {
                let indicator_area = Rect::new(inner.x, inner.y, inner.width, 1);
                let indicator = Paragraph::new(" \u{2191} scrollback (PgDn to return) ")
                    .alignment(Alignment::Right)
                    .style(
                        Style::default()
                            .fg(app.theme.fg_dim)
                            .bg(app.theme.highlight_bg),
                    );
                frame.render_widget(indicator, indicator_area);
            }

            // Show overlay bar if session has exited
            if app.exited_sessions.contains(session_id) && inner.height > 0 {
                let overlay_area = Rect::new(
                    inner.x,
                    inner.y + inner.height - 1,
                    inner.width,
                    1,
                );
                let overlay = Paragraph::new(" [exited] press r to restart ")
                    .alignment(Alignment::Center)
                    .style(
                        Style::default()
                            .fg(app.theme.bg)
                            .bg(app.theme.session_exited),
                    );
                frame.render_widget(overlay, overlay_area);
            }

            return;
        }
    }

    // No session data available — show placeholder
    let placeholder = Paragraph::new("  No session data")
        .style(Style::default().fg(app.theme.fg_dim));
    frame.render_widget(placeholder, inner);
}

/// Render the single-pane terminal panel (backward compatible entry point).
pub fn render(frame: &mut Frame, area: Rect, app: &App, session_manager: &SessionManager, is_focused: bool) {
    if let Some(ref session_id) = app.active_session_id {
        render_session(frame, area, app, session_manager, session_id, is_focused);
    } else {
        // No active session — show placeholder with border
        let border_style = if is_focused {
            Style::default().fg(app.theme.border_active)
        } else {
            Style::default().fg(app.theme.border_inactive)
        };

        let block = Block::default()
            .title(" Claude Code ")
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let placeholder = Paragraph::new("  Press Enter on a worktree to start a Claude Code session")
            .style(Style::default().fg(app.theme.fg_dim));
        frame.render_widget(placeholder, inner);
    }
}
