use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use ratatui::text::{Line, Span};

use crate::app::{
    App, DirBrowser, InputMode, NotePosition, PanelFocus, Prompt, SplitDirection, SplitNode,
    ViewKind, PRESET_COLORS,
};
use crate::planet;
use crate::session::manager::SessionManager;
use crate::widgets;

/// Split the main body area into horizontal columns: sidebar, [notes], right panel.
/// Returns (columns, right_index) where right_index points to the right panel chunk.
fn split_main_columns(
    body: Rect,
    sidebar_pct: u16,
    notes_pct: Option<u16>,
) -> (std::rc::Rc<[Rect]>, usize) {
    let remaining = 100u16
        .saturating_sub(sidebar_pct)
        .saturating_sub(notes_pct.unwrap_or(0));
    let constraints: Vec<Constraint> = if let Some(np) = notes_pct {
        vec![
            Constraint::Percentage(sidebar_pct),
            Constraint::Percentage(np),
            Constraint::Percentage(remaining),
        ]
    } else {
        vec![
            Constraint::Percentage(sidebar_pct),
            Constraint::Percentage(remaining),
        ]
    };
    let right_idx = if notes_pct.is_some() { 2 } else { 1 };
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(body);
    (chunks, right_idx)
}

/// Compute the right panel Rect (before any pane splitting).
/// `notes_pct` is Some(pct) when a center notes column is present.
pub fn right_panel_rect(size: Rect, sidebar_pct: u16, notes_pct: Option<u16>) -> Rect {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    let (chunks, right_idx) = split_main_columns(outer[1], sidebar_pct, notes_pct);
    chunks[right_idx]
}

/// Compute inner Rects for each pane by splitting the right panel.
/// `direction` controls whether panes are side-by-side (Horizontal) or stacked (Vertical).
pub fn compute_pane_rects(
    size: Rect,
    pane_count: usize,
    sidebar_pct: u16,
    direction: SplitDirection,
    notes_pct: Option<u16>,
) -> Vec<Rect> {
    let panel = right_panel_rect(size, sidebar_pct, notes_pct);
    if pane_count <= 1 {
        return vec![panel];
    }
    let constraints: Vec<Constraint> = (0..pane_count)
        .map(|_| Constraint::Ratio(1, pane_count as u32))
        .collect();
    let layout_dir = match direction {
        SplitDirection::Horizontal => Direction::Horizontal,
        SplitDirection::Vertical => Direction::Vertical,
    };
    Layout::default()
        .direction(layout_dir)
        .constraints(constraints)
        .split(panel)
        .to_vec()
}

/// Recursively compute leaf Rects from a SplitNode tree within the given area.
/// Returns a Vec of Rects in in-order (left-to-right / top-to-bottom) leaf order.
pub fn compute_leaf_rects(node: &SplitNode, area: Rect) -> Vec<Rect> {
    match node {
        SplitNode::Leaf(_) => vec![area],
        SplitNode::Split {
            direction,
            first,
            second,
        } => {
            let layout_dir = match direction {
                SplitDirection::Horizontal => Direction::Horizontal,
                SplitDirection::Vertical => Direction::Vertical,
            };
            let chunks = Layout::default()
                .direction(layout_dir)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);
            let mut result = compute_leaf_rects(first, chunks[0]);
            result.extend(compute_leaf_rects(second, chunks[1]));
            result
        }
    }
}

/// Compute the inner Rect where the terminal PTY is rendered (single pane).
pub fn compute_pty_rect(size: Rect, sidebar_pct: u16, notes_pct: Option<u16>) -> Rect {
    let panel = right_panel_rect(size, sidebar_pct, notes_pct);
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
        ViewKind::Terminal => {
            widgets::terminal_panel::render(frame, area, app, session_manager, is_focused)
        }
        ViewKind::FileExplorer => widgets::file_explorer::render(frame, area, app, is_focused),
        ViewKind::Editor => widgets::editor::render(frame, area, app, is_focused),
        ViewKind::Search => widgets::search_results::render(frame, area, app, is_focused),
        ViewKind::GitStatus => widgets::git_status::render(frame, area, app, is_focused),
        ViewKind::DiffView => widgets::diff_view::render(frame, area, app, is_focused),
        ViewKind::GitBlame => widgets::git_blame::render(frame, area, app, is_focused),
        ViewKind::GitLog => widgets::git_log::render(frame, area, app, is_focused),
        ViewKind::Shell => {
            widgets::terminal_panel::render_shell(frame, area, app, session_manager, is_focused)
        }
        ViewKind::Notes => widgets::notes_editor::render(frame, area, app, is_focused),
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
    let header = Paragraph::new(" darya").style(
        Style::default()
            .fg(app.theme.border_active)
            .bg(app.theme.highlight_bg)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(header, outer[0]);

    // Main layout: left panel | [optional notes column] | right panel
    let sidebar_pct = app.sidebar_width;
    let notes_pct = app.notes_pct();
    let (main_chunks, right_chunk_idx) = split_main_columns(outer[1], sidebar_pct, notes_pct);

    // Left panel (sidebar) — optionally split to include notes preview and/or planet
    let left_focused = app.panel_focus == PanelFocus::Left;
    let show_sidebar_notes = app.note_position == NotePosition::Sidebar;
    let show_planet =
        app.show_planet && app.planet_kind.is_some() && app.planet_animation.is_some();

    // Build sidebar vertical constraints
    let mut sidebar_constraints: Vec<Constraint> =
        vec![Constraint::Percentage(if show_sidebar_notes {
            55
        } else {
            100
        })]; // worktree list
    if show_sidebar_notes {
        sidebar_constraints.push(Constraint::Percentage(45)); // notes panel
    }
    if show_planet {
        let max_planet_h: u16 = 16;
        let planet_height = max_planet_h.min(main_chunks[0].height / 3);
        sidebar_constraints.push(Constraint::Length(planet_height));
    }

    let sidebar_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints(sidebar_constraints)
        .split(main_chunks[0]);

    // Render worktree list (always first)
    render_view(
        frame,
        sidebar_split[0],
        app.sidebar_view.to_view_kind(),
        app,
        session_manager,
        left_focused,
    );

    // Render sidebar notes preview and/or planet
    let mut slot = 1usize;
    if show_sidebar_notes {
        widgets::notes_preview::render(frame, sidebar_split[slot], app, left_focused);
        slot += 1;
    }
    if show_planet {
        if let Some(ref anim) = app.planet_animation {
            let planet_area = sidebar_split[slot];
            let elapsed_ms = app.planet_start.elapsed().as_millis() as usize;
            let anim_frame = anim.frame_at(elapsed_ms / 100); // ~10fps
            let render_h = planet_area.height.saturating_sub(4);
            let render_w = (render_h * 2).min(planet_area.width);
            let planet_lines =
                planet::renderer::render_frame(anim_frame, render_w, render_h, app.theme.bg);
            widgets::planet_widget::render(frame, planet_area, &planet_lines);
        }
    }

    // Center column is no longer used — notes edit inline in the sidebar

    // Right panel (main) — split panes or full panel
    let right_focused = app.panel_focus == PanelFocus::Right;
    // Snapshot pane info to avoid borrow conflicts with mutable render calls
    let pane_leaves: Vec<(crate::app::PaneContent, bool)>;
    let pane_rects: Vec<Rect>;
    let has_split = if let Some(ref layout) = app.pane_layout {
        let leaf_count = layout.root.leaf_count();
        if leaf_count > 1 {
            let panel = right_panel_rect(size, sidebar_pct, notes_pct);
            pane_rects = compute_leaf_rects(&layout.root, panel);
            pane_leaves = layout
                .root
                .leaves()
                .into_iter()
                .enumerate()
                .map(|(i, c)| (c.clone(), right_focused && i == layout.focused))
                .collect();
            true
        } else {
            pane_leaves = Vec::new();
            pane_rects = Vec::new();
            false
        }
    } else {
        pane_leaves = Vec::new();
        pane_rects = Vec::new();
        false
    };
    if has_split {
        for (i, (content, pane_focused)) in pane_leaves.iter().enumerate() {
            match content {
                crate::app::PaneContent::Terminal(session_id)
                | crate::app::PaneContent::Shell(session_id) => {
                    widgets::terminal_panel::render_session(
                        frame,
                        pane_rects[i],
                        app,
                        session_manager,
                        session_id,
                        *pane_focused,
                    );
                }
                crate::app::PaneContent::Editor => {
                    widgets::editor::render(frame, pane_rects[i], app, *pane_focused);
                }
            }
        }
    } else {
        render_view(
            frame,
            main_chunks[right_chunk_idx],
            app.main_view.to_view_kind(),
            app,
            session_manager,
            right_focused,
        );
    }

    // Status bar
    if let Some(ref msg) = app.status_message {
        let status = Paragraph::new(format!(" {} ", msg)).style(
            Style::default()
                .fg(app.theme.warning)
                .bg(app.theme.status_bar_bg),
        );
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
            ViewKind::Shell => "shell",
            ViewKind::Notes => "notes",
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
        let (running, exited) = app.session_counts();
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

        let status = Paragraph::new(line).style(
            Style::default()
                .fg(app.theme.status_bar_fg)
                .bg(app.theme.status_bar_bg),
        );
        frame.render_widget(status, outer[2]);
    }

    // Render prompt overlay if active
    if let Some(ref prompt) = app.prompt {
        if let Prompt::ThemePicker { selected, .. } = prompt {
            widgets::theme_picker::render(frame, size, app, *selected);
        } else {
            render_prompt(frame, size, prompt, &app.theme);
        }
    }

    // Render directory browser overlay if active
    if let Some(ref browser) = app.dir_browser {
        render_dir_browser(frame, size, browser, &app.theme);
    }

    // Render help overlay if active
    if app.show_help {
        widgets::help_overlay::render(frame, size, app);
    }

    // Render fuzzy finder overlay if active
    if app.fuzzy_finder.is_some() {
        widgets::fuzzy_finder::render(frame, size, app);
    }

    // Render command palette overlay if active
    if app.command_palette.is_some() {
        widgets::command_palette::render(frame, size, app);
    }

    // Render split picker overlay if active
    if app.split_picker.is_some() {
        widgets::split_picker::render(frame, size, app);
    }

    // Render branch switcher overlay if active
    if app.branch_switcher.is_some() {
        widgets::branch_switcher::render(frame, size, app);
    }
}

fn render_prompt(frame: &mut Frame, area: Rect, prompt: &Prompt, theme: &crate::config::Theme) {
    // ThemePicker is rendered separately from draw() since it needs &App
    if matches!(prompt, Prompt::ThemePicker { .. }) {
        return;
    }

    // SetupGuide uses a larger overlay
    if matches!(prompt, Prompt::SetupGuide) {
        render_setup_guide(frame, area, theme);
        return;
    }

    // RestoreSession uses a compact centered overlay
    if let Prompt::RestoreSession { count } = prompt {
        render_restore_session(frame, area, theme, *count);
        return;
    }

    let width = if matches!(prompt, Prompt::ColorPicker { .. }) {
        30u16
    } else {
        50u16
    }
    .min(area.width.saturating_sub(4));
    let height = if matches!(prompt, Prompt::ColorPicker { .. }) {
        4u16
    } else {
        3u16
    };
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

            let text = Paragraph::new(format!("{}█", input)).style(
                Style::default()
                    .fg(theme.fg)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD),
            );
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
                .title(" Search project ")
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(theme.prompt_border))
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = Paragraph::new(format!("{}█", input)).style(
                Style::default()
                    .fg(theme.fg)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(text, inner);
        }
        Prompt::AddSessionSlot { input, kind } => {
            let kind_label = match kind {
                crate::sidebar::types::SessionKind::Claude => "Claude",
                crate::sidebar::types::SessionKind::Shell => "Shell",
            };
            let title = format!(" New {} slot (label) ", kind_label);
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(theme.prompt_border))
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = Paragraph::new(format!("{}█", input)).style(
                Style::default()
                    .fg(theme.fg)
                    .bg(theme.bg)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(text, inner);
        }
        Prompt::ConfirmDeleteSection { section_name, .. } => {
            let block = Block::default()
                .title(" Confirm Delete Section ")
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(theme.prompt_delete_border))
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let text = Paragraph::new(format!("Delete section '{}'? (y/N)", section_name))
                .style(Style::default().fg(theme.fg).bg(theme.bg));
            frame.render_widget(text, inner);
        }
        Prompt::ColorPicker { cursor, .. } => {
            // Use the same popup_area as all other prompts
            let block = Block::default()
                .title(" Assign Color ")
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(theme.prompt_border))
                .style(Style::default().bg(theme.bg));
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            // Fit swatches into the inner width: each slot is 3 chars (" ██" or "[██")
            // plus a closing bracket/space on the last one
            let cols = 7u16;
            let rows = ((PRESET_COLORS.len() as u16) + cols - 1) / cols;

            let mut lines: Vec<Line> = Vec::new();
            for row in 0..rows {
                let mut spans: Vec<Span> = Vec::new();
                for col in 0..cols {
                    let idx = (row * cols + col) as usize;
                    if idx >= PRESET_COLORS.len() {
                        break;
                    }
                    let is_selected = idx == *cursor;
                    let swatch = match PRESET_COLORS[idx] {
                        None => "--",
                        Some(_) => "\u{2588}\u{2588}",
                    };
                    let fg = PRESET_COLORS[idx].unwrap_or(theme.fg_dim);
                    let swatch_style = Style::default().fg(fg).bg(theme.bg);
                    let bracket_style = Style::default()
                        .fg(theme.fg)
                        .bg(theme.bg)
                        .add_modifier(Modifier::BOLD);
                    let space_style = Style::default().bg(theme.bg);
                    if is_selected {
                        spans.push(Span::styled("[", bracket_style));
                        spans.push(Span::styled(swatch, swatch_style));
                        spans.push(Span::styled("]", bracket_style));
                    } else {
                        spans.push(Span::styled(" ", space_style));
                        spans.push(Span::styled(swatch, swatch_style));
                        spans.push(Span::styled(" ", space_style));
                    }
                }
                lines.push(Line::from(spans));
            }

            let paragraph = Paragraph::new(lines).style(Style::default().bg(theme.bg));
            frame.render_widget(paragraph, inner);
        }
        Prompt::SetupGuide => unreachable!(), // handled by early return
        Prompt::RestoreSession { .. } => unreachable!(), // handled by early return
        Prompt::ThemePicker { .. } => unreachable!(), // handled by early return
    }
}

fn render_setup_guide(frame: &mut Frame, area: Rect, theme: &crate::config::Theme) {
    let width = 56u16.min(area.width.saturating_sub(4));
    let height = 16u16.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Welcome to Darya ")
        .title_style(
            Style::default()
                .fg(theme.border_active)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(theme.border_active))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let bold = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(theme.fg);
    let dim = Style::default().fg(theme.fg_dim);
    let accent = Style::default()
        .fg(theme.border_active)
        .add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Darya uses Cmd+key shortcuts:",
            normal,
        )]),
        Line::from(vec![
            Span::styled("    Cmd+1", accent),
            Span::styled("  Worktrees    ", normal),
            Span::styled("Cmd+2", accent),
            Span::styled("  Terminal", normal),
        ]),
        Line::from(vec![
            Span::styled("    Cmd+P", accent),
            Span::styled("  Fuzzy Find   ", normal),
            Span::styled("Cmd+K", accent),
            Span::styled("  Command Palette", normal),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  iTerm2 intercepts these by default. To fix:",
            normal,
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  1. ", bold),
            Span::styled(
                "iTerm2 \u{2192} Settings \u{2192} Keys \u{2192} Key Bindings",
                normal,
            ),
        ]),
        Line::from(vec![
            Span::styled("  2. ", bold),
            Span::styled("Remove or reassign Cmd+1 through Cmd+9", normal),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Or edit ~/.config/darya/config.toml to rebind keys",
            dim,
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Tip: Remap Caps Lock \u{2192} F18 via Karabiner-Elements",
            dim,
        )]),
        Line::from(vec![Span::styled(
            "  to use Caps Lock as the panel-switch key",
            dim,
        )]),
        Line::from(""),
        Line::from(vec![Span::styled("  Press Enter or Esc to dismiss", dim)]),
    ];

    let paragraph = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    frame.render_widget(paragraph, inner);
}

fn render_restore_session(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::config::Theme,
    count: usize,
) {
    let width = 45u16.min(area.width.saturating_sub(4));
    let height = 5u16.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Restore Sessions ")
        .title_style(
            Style::default()
                .fg(theme.border_active)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(theme.border_active))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let session_word = if count == 1 { "session" } else { "sessions" };
    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  Restore {} previous {}? ", count, session_word),
            Style::default().fg(theme.fg),
        )]),
        Line::from(vec![
            Span::styled(
                "  y",
                Style::default()
                    .fg(theme.border_active)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("/Enter: yes  ", Style::default().fg(theme.fg_dim)),
            Span::styled(
                "n",
                Style::default()
                    .fg(theme.border_active)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("/Esc: no", Style::default().fg(theme.fg_dim)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).style(Style::default().bg(theme.bg));
    frame.render_widget(paragraph, inner);
}

fn render_dir_browser(
    frame: &mut Frame,
    area: Rect,
    browser: &DirBrowser,
    theme: &crate::config::Theme,
) {
    let width = (area.width * 60 / 100)
        .max(30)
        .min(area.width.saturating_sub(4));
    let height = (area.height * 70 / 100)
        .max(10)
        .min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Select directory ")
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(theme.prompt_border))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 2 {
        return;
    }

    // List area (leave 1 row for footer hint)
    let list_height = inner.height.saturating_sub(1) as usize;
    let total = browser.entries.len();

    // Scroll so selected item is visible
    let scroll_offset = if total <= list_height || browser.selected < list_height / 2 {
        0
    } else if browser.selected + list_height / 2 >= total {
        total.saturating_sub(list_height)
    } else {
        browser.selected.saturating_sub(list_height / 2)
    };

    let visible_entries = browser
        .entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(list_height);

    for (i, (idx, entry)) in visible_entries.enumerate() {
        let row_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        let is_selected = idx == browser.selected;
        let indent = "  ".repeat(entry.depth);
        let arrow = if browser.is_expanded(&entry.path) {
            "\u{25be} " // ▾
        } else {
            "\u{25b8} " // ▸
        };
        let line_text = format!("{}{}{}", indent, arrow, entry.name);

        let style = if is_selected {
            Style::default()
                .fg(theme.fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg).bg(theme.bg)
        };

        let paragraph = Paragraph::new(line_text).style(style);
        frame.render_widget(paragraph, row_area);
    }

    // Footer hint
    let footer_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
    let footer = Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(theme.border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": select  ", Style::default().fg(theme.fg_dim)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(theme.border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": cancel  ", Style::default().fg(theme.fg_dim)),
        Span::styled(
            "h/l",
            Style::default()
                .fg(theme.border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": collapse/expand", Style::default().fg(theme.fg_dim)),
    ]);
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().bg(theme.bg)),
        footer_area,
    );
}
