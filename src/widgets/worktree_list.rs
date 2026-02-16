use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;
use crate::config::Theme;

/// Linearly interpolate between two RGB colors. `t` ranges 0.0 (color a) to 1.0 (color b).
fn lerp_color(a: ratatui::style::Color, b: ratatui::style::Color, t: f32) -> ratatui::style::Color {
    use ratatui::style::Color;
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let t = t.clamp(0.0, 1.0);
            Color::Rgb(
                (ar as f32 + (br as f32 - ar as f32) * t) as u8,
                (ag as f32 + (bg as f32 - ag as f32) * t) as u8,
                (ab as f32 + (bb as f32 - ab as f32) * t) as u8,
            )
        }
        _ => if t > 0.5 { b } else { a },
    }
}

/// Build 5 styled spans for the Knight Rider scanner animation.
/// Diamond head with gradient trail that fades behind.
fn build_animation_spans(trail: [u8; 5], theme: &Theme) -> Vec<Span<'static>> {
    // 4-level gradient: dim → bright (session_active)
    let colors = [
        lerp_color(theme.fg_dim, theme.session_active, 0.0),   // level 0: dim
        lerp_color(theme.fg_dim, theme.session_active, 0.35),  // level 1
        lerp_color(theme.fg_dim, theme.session_active, 0.65),  // level 2
        theme.session_active,                                    // level 3: full bright
    ];

    trail
        .iter()
        .map(|&level| {
            let color = colors[level as usize];
            let ch = if level >= 2 { "\u{25C6}" } else { "\u{00B7}" }; // ◆ or ·
            Span::styled(ch, Style::default().fg(color))
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
            let shell_id = app.shell_session_ids.get(&wt.path);
            let has_session = session_id.is_some();
            let has_shell = shell_id.is_some();
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
            let shell_indicator = if has_shell {
                let shell_exited = shell_id
                    .map(|id| app.exited_sessions.contains(id))
                    .unwrap_or(false);
                if shell_exited { " \u{2715}$" } else { " $" }
            } else {
                ""
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
                    Span::styled(
                        shell_indicator.to_string(),
                        Style::default().fg(app.theme.fg_dim),
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
                    Span::styled(
                        shell_indicator.to_string(),
                        Style::default().fg(app.theme.fg_dim),
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
                    Span::styled(
                        shell_indicator.to_string(),
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
                let trail = app.activity.trail(session_id.unwrap());
                spans.extend(build_animation_spans(trail, &app.theme));
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
