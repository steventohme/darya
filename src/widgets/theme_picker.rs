use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::planet::renderer;
use crate::planet::types::PlanetKind;
use crate::widgets::planet_widget;

/// Render the theme picker overlay.
/// `selected` is the index into PlanetKind::all().
pub fn render(frame: &mut Frame, area: Rect, app: &App, selected: usize) {
    let planets = PlanetKind::all();
    let planet = planets[selected % planets.len()];
    let theme = &app.theme;

    // Overlay size: ~70% of screen
    let width = (area.width * 70 / 100).max(40).min(area.width.saturating_sub(4));
    let height = (area.height * 70 / 100).max(16).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Choose Your Planet ")
        .title_style(Style::default().fg(theme.border_active).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(theme.border_active))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 6 || inner.width < 10 {
        return;
    }

    // Layout: planet animation takes most space, then info + nav at bottom
    let footer_height = 4u16; // planet name + dots + controls + blank
    let planet_area_height = inner.height.saturating_sub(footer_height);

    // Render planet animation — cap size to keep it compact
    if let Some(ref anim) = app.planet_animation {
        let anim_frame = anim.frame_at(app.planet_tick / 2); // ~10fps at 50ms tick
        let max_h = planet_area_height.min(20);
        let planet_width = (max_h * 2).min(inner.width); // ~square aspect ratio
        let lines = renderer::render_frame(anim_frame, planet_width, max_h, theme.bg);

        let planet_rect = Rect::new(inner.x, inner.y, inner.width, planet_area_height);
        planet_widget::render(frame, planet_rect, &lines);
    }

    // Footer area
    let footer_y = inner.y + planet_area_height;
    let footer_area = Rect::new(inner.x, footer_y, inner.width, footer_height.min(inner.height));

    let accent = planet.accent();
    let bold = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(theme.fg_dim);
    let accent_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);

    // Planet name with accent color swatch
    let name_line = Line::from(vec![
        Span::styled("  ", Style::default().bg(theme.bg)),
        Span::styled(planet.display_name(), accent_style),
        Span::styled("  ", Style::default().bg(theme.bg)),
        Span::styled("\u{2588}\u{2588}\u{2588}", Style::default().fg(accent)),
    ]);

    // Dot indicators
    let mut dot_spans = vec![Span::styled("  ", Style::default().bg(theme.bg))];
    for (i, _) in planets.iter().enumerate() {
        if i == selected {
            dot_spans.push(Span::styled("\u{25CF} ", accent_style)); // ●
        } else {
            dot_spans.push(Span::styled("\u{25CB} ", dim)); // ○
        }
    }
    let dots_line = Line::from(dot_spans);

    // Mode indicator
    let mode_str = match app.theme.mode {
        crate::config::ThemeMode::Dark => "dark",
        crate::config::ThemeMode::Light => "light",
    };

    // Controls
    let controls_line = Line::from(vec![
        Span::styled("  \u{2190} \u{2192}", bold),
        Span::styled(" browse  ", dim),
        Span::styled("Enter", bold),
        Span::styled(" select  ", dim),
        Span::styled("d/l", bold),
        Span::styled(format!(" {} mode  ", mode_str), dim),
        Span::styled("Esc", bold),
        Span::styled(" cancel", dim),
    ]);

    let footer_lines = vec![
        name_line,
        dots_line,
        Line::from(""),
        controls_line,
    ];

    let footer_para = Paragraph::new(footer_lines).style(Style::default().bg(theme.bg));
    frame.render_widget(footer_para, footer_area);
}
