use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Render pre-rendered planet lines centered within the given area.
pub fn render(frame: &mut Frame, area: Rect, lines: &[Line<'static>]) {
    if area.height == 0 || area.width == 0 || lines.is_empty() {
        return;
    }

    let content_height = lines.len() as u16;
    let y_offset = area.height.saturating_sub(content_height) / 2;

    // Find widest line for horizontal centering
    let content_width = lines
        .iter()
        .map(|l| l.spans.len() as u16)
        .max()
        .unwrap_or(0);
    let x_offset = area.width.saturating_sub(content_width) / 2;

    let render_area = Rect::new(
        area.x + x_offset,
        area.y + y_offset,
        content_width.min(area.width),
        content_height.min(area.height),
    );

    let paragraph = Paragraph::new(lines.to_vec());
    frame.render_widget(paragraph, render_area);
}
