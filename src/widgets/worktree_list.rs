use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;
use crate::config::Theme;

/// Build 5 styled spans for the bouncing-block animation.
/// Small dim squares as a track, with a bigger bright orange square bouncing through.
fn build_animation_spans(pos: usize, theme: &Theme) -> Vec<Span<'static>> {
    let bright_style = Style::default().fg(theme.session_active);
    let dim_style = Style::default().fg(theme.fg_dim);

    (0..5)
        .map(|i| {
            if i == pos {
                Span::styled("\u{25A0}", bright_style) // ■ big square
            } else {
                Span::styled("\u{25AA}", dim_style) // ▪ small square
            }
        })
        .collect()
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    // Derive repo name from the main worktree's directory name
    let repo_name = app
        .worktrees
        .iter()
        .find(|wt| wt.is_main)
        .map(|wt| wt.name.as_str())
        .unwrap_or("repo");

    let items: Vec<ListItem> = app
        .worktrees
        .iter()
        .enumerate()
        .map(|(i, wt)| {
            let session_id = app.session_ids.get(&wt.path);
            let has_session = session_id.is_some();
            let is_exited = session_id
                .map(|id| app.exited_sessions.contains(id))
                .unwrap_or(false);
            let needs_attention = session_id
                .map(|id| app.attention_sessions.contains(id))
                .unwrap_or(false);
            let is_animating = !is_exited
                && session_id
                    .map(|id| app.activity.is_active(id))
                    .unwrap_or(false);
            let indicator = if is_exited {
                "\u{2715}"
            } else if has_session {
                "\u{25CF}"
            } else {
                "\u{25CB}"
            };

            let branch_str = wt
                .branch
                .as_deref()
                .unwrap_or("detached");

            let exited_marker = if is_exited { " [exited]" } else { "" };

            // Hotkey label: 1-9 for first 9, 0 for 10th
            let hotkey = if i < 9 {
                format!("{}", i + 1)
            } else if i == 9 {
                "0".to_string()
            } else {
                " ".to_string()
            };

            let indicator_color = if is_exited {
                app.theme.session_exited
            } else if needs_attention {
                app.theme.session_attention
            } else if has_session {
                app.theme.session_active
            } else {
                app.theme.session_inactive
            };

            let mut spans = if is_exited {
                let exited_color = app.theme.session_exited;
                vec![
                    Span::styled(
                        format!("{} {} ", hotkey, indicator),
                        Style::default().fg(exited_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        repo_name.to_string(),
                        Style::default().fg(app.theme.fg),
                    ),
                    Span::styled(
                        format!(" [{}]", branch_str),
                        Style::default().fg(app.theme.fg_dim),
                    ),
                    Span::styled(
                        exited_marker.to_string(),
                        Style::default().fg(exited_color).add_modifier(Modifier::DIM),
                    ),
                ]
            } else if needs_attention {
                let attn = app.theme.session_attention;
                vec![
                    Span::styled(
                        format!("{} {} ", hotkey, indicator),
                        Style::default().fg(attn).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        repo_name.to_string(),
                        Style::default().fg(attn).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" [{}]", branch_str),
                        Style::default().fg(attn),
                    ),
                ]
            } else {
                vec![
                    Span::styled(
                        format!("{} {} ", hotkey, indicator),
                        Style::default().fg(indicator_color),
                    ),
                    Span::styled(
                        repo_name.to_string(),
                        Style::default().fg(app.theme.fg),
                    ),
                    Span::styled(
                        format!(" [{}]", branch_str),
                        Style::default().fg(app.theme.fg_dim),
                    ),
                ]
            };

            // Right-align bouncing animation if session is actively producing output
            if is_animating {
                // Content area: total width - 2 (borders) - 2 (highlight symbol "▶ ")
                let content_width = (area.width as usize).saturating_sub(4);
                // Text width: "{hotkey} {indicator} " (4) + repo_name + " [{branch}]" (3+branch)
                //             + optional " [exited]" (9)
                let text_width = 4 + repo_name.len() + 3 + branch_str.len()
                    + if is_exited { 9 } else { 0 };
                let anim_width = 5; // 5 animation characters
                let right_margin = 1;
                let padding = content_width.saturating_sub(text_width + anim_width + right_margin);

                spans.push(Span::raw(" ".repeat(padding)));
                let pos = app.activity.position(session_id.unwrap());
                spans.extend(build_animation_spans(pos, &app.theme));
            }

            ListItem::new(Line::from(spans))
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
                .title(" Worktrees ")
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{25B6} ");

    let mut state = ListState::default();
    state.select(Some(app.selected_worktree));
    frame.render_stateful_widget(list, area, &mut state);
}
