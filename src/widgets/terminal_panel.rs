use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::app::{App, Panel};
use crate::session::manager::SessionManager;

pub fn render(frame: &mut Frame, area: Rect, app: &App, session_manager: &SessionManager) {
    let border_style = if app.active_panel == Panel::Terminal {
        Style::default().fg(app.theme.border_active)
    } else {
        Style::default().fg(app.theme.border_inactive)
    };

    // Always render the border block
    let block = Block::default()
        .title(" Claude Code ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref session_id) = app.active_session_id {
        if let Some(session) = session_manager.get(session_id) {
            if let Ok(parser) = session.parser.read() {
                let pseudo_term = PseudoTerminal::new(parser.screen());
                frame.render_widget(pseudo_term, inner);

                // Post-process: replace Color::Reset backgrounds with theme bg.
                // tui-term maps vt100::Color::Default → Color::Reset which means
                // "use system terminal default", causing theme mismatch.
                let buf = frame.buffer_mut();
                for y in inner.y..inner.y + inner.height {
                    for x in inner.x..inner.x + inner.width {
                        let cell = &mut buf[(x, y)];
                        if cell.bg == Color::Reset {
                            cell.bg = app.theme.bg;
                        }
                    }
                }
                return;
            }
        }
    }

    // No active session — show placeholder
    let placeholder = Paragraph::new("  Press Enter on a worktree to start a Claude Code session")
        .style(Style::default().fg(app.theme.fg_dim));
    frame.render_widget(placeholder, inner);
}
