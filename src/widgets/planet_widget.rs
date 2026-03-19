use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

/// Render pre-rendered planet lines centered within a bordered box.
pub fn render(frame: &mut Frame, area: Rect, lines: &[Line<'static>]) {
    if area.height < 3 || area.width < 3 || lines.is_empty() {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content_height = lines.len() as u16;
    let y_offset = inner.height.saturating_sub(content_height) / 2;

    // Find widest line for horizontal centering
    let content_width = lines
        .iter()
        .map(|l| l.spans.len() as u16)
        .max()
        .unwrap_or(0);
    let x_offset = inner.width.saturating_sub(content_width) / 2;

    let render_area = Rect::new(
        inner.x + x_offset,
        inner.y + y_offset,
        content_width.min(inner.width),
        content_height.min(inner.height),
    );

    let paragraph = Paragraph::new(lines.to_vec());
    frame.render_widget(paragraph, render_area);
}
