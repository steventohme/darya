use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;
use crate::config::Theme;
use crate::sidebar::tree::TreeNode;
use crate::sidebar::types::SessionKind;

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
fn build_animation_spans(trail: [u8; 5], theme: &Theme) -> Vec<Span<'static>> {
    let colors = [
        lerp_color(theme.fg_dim, theme.session_active, 0.0),
        lerp_color(theme.fg_dim, theme.session_active, 0.35),
        lerp_color(theme.fg_dim, theme.session_active, 0.65),
        theme.session_active,
    ];

    trail
        .iter()
        .map(|&level| {
            let color = colors[level as usize];
            let ch = if level >= 2 { "\u{25C6}" } else { "\u{00B7}" };
            Span::styled(ch, Style::default().fg(color))
        })
        .collect()
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, is_focused: bool) {
    // Track which item index we're on (for hotkey labels)
    let mut item_counter: usize = 0;

    let items: Vec<ListItem> = app
        .sidebar_tree
        .visible
        .iter()
        .map(|node| {
            match node {
                TreeNode::Section(si) => {
                    let section = &app.sidebar_tree.sections[*si];
                    let arrow = if section.collapsed { "\u{25B6}" } else { "\u{25BC}" };
                    let name_color = section.color.unwrap_or(app.theme.fg);
                    let spans = vec![
                        Span::styled(
                            format!("{} {}", arrow, section.name),
                            Style::default()
                                .fg(name_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    ListItem::new(Line::from(spans))
                }
                TreeNode::Item(si, ii) => {
                    let item = &app.sidebar_tree.sections[*si].items[*ii];
                    let arrow = if item.collapsed { "\u{25B6}" } else { "\u{25BC}" };

                    // Find Claude session status for the indicator
                    let claude_slot = item.sessions.iter().find(|s| s.kind == SessionKind::Claude);
                    let session_id = claude_slot.and_then(|s| s.session_id.as_deref());
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

                    let branch_str = item.branch.as_deref().unwrap_or("detached");
                    let exited_marker = if is_exited { " [exited]" } else { "" };

                    // Claude status text (window title from OSC 0/2) for active sessions.
                    // Filter out bare directory names — only show titles that look like
                    // real status updates (e.g. "Thinking...", "Reading src/app.rs").
                    let claude_status = if !is_exited && has_session {
                        session_id
                            .and_then(|id| app.session_statuses.get(id))
                            .filter(|s| !s.is_empty() && s.contains(' ') && !s.contains("Claude Code"))
                            .cloned()
                    } else {
                        None
                    };

                    // Hotkey label: 1-9 for first 9 items, 0 for 10th
                    let hotkey = if item_counter < 9 {
                        format!("{}", item_counter + 1)
                    } else if item_counter == 9 {
                        "0".to_string()
                    } else {
                        " ".to_string()
                    };
                    item_counter += 1;

                    // Shell session count
                    let shell_count = item.sessions.iter()
                        .filter(|s| s.kind == SessionKind::Shell && s.session_id.is_some())
                        .count();
                    let shell_indicator = if shell_count > 0 {
                        format!(" $×{}", shell_count)
                    } else {
                        String::new()
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

                    let item_name_color = item.color.unwrap_or(app.theme.fg);
                    let mut spans = if is_exited {
                        let exited_color = app.theme.session_exited;
                        vec![
                            Span::styled(
                                format!("  {} {} {} ", hotkey, arrow, indicator),
                                Style::default().fg(exited_color).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                item.display_name.clone(),
                                Style::default().fg(item_name_color),
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
                                shell_indicator,
                                Style::default().fg(app.theme.fg_dim),
                            ),
                        ]
                    } else if needs_attention {
                        let attn = app.theme.session_attention;
                        let attn_name = item.color.unwrap_or(attn);
                        vec![
                            Span::styled(
                                format!("  {} {} {} ", hotkey, arrow, indicator),
                                Style::default().fg(attn).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                item.display_name.clone(),
                                Style::default().fg(attn_name).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!(" [{}]", branch_str),
                                Style::default().fg(attn),
                            ),
                            Span::styled(
                                shell_indicator,
                                Style::default().fg(app.theme.fg_dim),
                            ),
                        ]
                    } else {
                        let mut v = vec![
                            Span::styled(
                                format!("  {} {} {} ", hotkey, arrow, indicator),
                                Style::default().fg(indicator_color),
                            ),
                            Span::styled(
                                item.display_name.clone(),
                                Style::default().fg(item_name_color),
                            ),
                            Span::styled(
                                format!(" [{}]", branch_str),
                                Style::default().fg(app.theme.fg_dim),
                            ),
                            Span::styled(
                                shell_indicator,
                                Style::default().fg(app.theme.fg_dim),
                            ),
                        ];
                        if let Some(ref status) = claude_status {
                            let prefix_len = 8 + item.display_name.len() + 3 + branch_str.len();
                            let max_len = (area.width as usize).saturating_sub(prefix_len + 5);
                            if max_len > 3 {
                                let truncated = if status.len() > max_len {
                                    format!(" {}…", &status[..max_len.saturating_sub(1)])
                                } else {
                                    format!(" {}", status)
                                };
                                v.push(Span::styled(
                                    truncated,
                                    Style::default().fg(app.theme.session_active).add_modifier(Modifier::DIM),
                                ));
                            }
                        }
                        v
                    };

                    // Right-align bouncing animation
                    if is_animating {
                        let content_width = (area.width as usize).saturating_sub(4);
                        let text_width = 8 + item.display_name.len() + 3 + branch_str.len()
                            + if is_exited { 9 } else { 0 };
                        let anim_width = 5;
                        let right_margin = 1;
                        let padding = content_width.saturating_sub(text_width + anim_width + right_margin);

                        spans.push(Span::raw(" ".repeat(padding)));
                        let trail = app.activity.trail(session_id.unwrap());
                        spans.extend(build_animation_spans(trail, &app.theme));
                    }

                    ListItem::new(Line::from(spans))
                }
                TreeNode::Session(si, ii, slot_idx) => {
                    let slot = &app.sidebar_tree.sections[*si].items[*ii].sessions[*slot_idx];
                    let (icon, label_color) = match slot.kind {
                        SessionKind::Claude => ("\u{25CF}", app.theme.session_active), // ●
                        SessionKind::Shell => ("$", app.theme.fg_dim),
                    };

                    let status_color = match &slot.session_id {
                        Some(id) if app.exited_sessions.contains(id.as_str()) => app.theme.session_exited,
                        Some(id) if app.attention_sessions.contains(id.as_str()) => app.theme.session_attention,
                        Some(_) => label_color,
                        None => app.theme.fg_dim,
                    };

                    let status_suffix = match &slot.session_id {
                        Some(id) if app.exited_sessions.contains(id.as_str()) => " [exited]",
                        None => " (not started)",
                        _ => "",
                    };

                    let label_fg = slot.color.unwrap_or(status_color);

                    // Show Claude status text for active sessions (no [exited] or (not started)).
                    // Filter out bare directory names — only show multi-word status updates.
                    let claude_status = if status_suffix.is_empty() {
                        slot.session_id.as_ref()
                            .and_then(|id| app.session_statuses.get(id))
                            .filter(|s| !s.is_empty() && s.contains(' ') && !s.contains("Claude Code"))
                            .cloned()
                    } else {
                        None
                    };

                    let mut spans = vec![
                        Span::styled(
                            format!("    {} ", icon),
                            Style::default().fg(status_color),
                        ),
                        Span::styled(
                            slot.label.clone(),
                            Style::default().fg(label_fg),
                        ),
                    ];

                    if let Some(status) = claude_status {
                        let max_status_len = (area.width as usize)
                            .saturating_sub(8 + slot.label.len());
                        let truncated = if status.len() > max_status_len && max_status_len > 1 {
                            format!("{}…", &status[..max_status_len.saturating_sub(1)])
                        } else {
                            status
                        };
                        spans.push(Span::styled(
                            format!(" {}", truncated),
                            Style::default().fg(app.theme.session_active).add_modifier(Modifier::DIM),
                        ));
                    } else {
                        spans.push(Span::styled(
                            status_suffix.to_string(),
                            Style::default().fg(app.theme.fg_dim),
                        ));
                    }

                    ListItem::new(Line::from(spans))
                }
            }
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
                .fg(app.theme.fg)
                .bg(app.theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{2502} ");

    let mut state = ListState::default();
    state.select(Some(app.sidebar_tree.cursor));
    frame.render_stateful_widget(list, area, &mut state);
}
