use image::imageops::FilterType;
use image::RgbaImage;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Upper half-block character — top pixel is fg, bottom pixel is bg.
const HALF_BLOCK: &str = "\u{2580}"; // ▀

/// Render an RGBA image frame into a Vec<Line> using half-block characters.
///
/// Each terminal cell row represents 2 vertical pixels:
/// - fg color = top pixel
/// - bg color = bottom pixel
///
/// `target_width` and `target_height` are in terminal cells.
/// The image is resized to `target_width x (target_height * 2)` pixels.
/// Transparent pixels (alpha < 128) use the provided `bg` color.
pub fn render_frame(
    frame: &RgbaImage,
    target_width: u16,
    target_height: u16,
    bg: Color,
) -> Vec<Line<'static>> {
    if target_width == 0 || target_height == 0 {
        return Vec::new();
    }

    let pixel_width = target_width as u32;
    let pixel_height = (target_height as u32) * 2;

    let resized = image::imageops::resize(frame, pixel_width, pixel_height, FilterType::Nearest);

    let mut lines = Vec::with_capacity(target_height as usize);

    for row in 0..target_height as u32 {
        let top_y = row * 2;
        let bot_y = top_y + 1;

        let mut spans = Vec::with_capacity(target_width as usize);

        for col in 0..pixel_width {
            let top_pixel = resized.get_pixel(col, top_y);
            let bot_pixel = if bot_y < pixel_height {
                *resized.get_pixel(col, bot_y)
            } else {
                image::Rgba([0, 0, 0, 0])
            };

            let fg = if top_pixel[3] >= 128 {
                Color::Rgb(top_pixel[0], top_pixel[1], top_pixel[2])
            } else {
                bg
            };

            let bg_color = if bot_pixel[3] >= 128 {
                Color::Rgb(bot_pixel[0], bot_pixel[1], bot_pixel[2])
            } else {
                bg
            };

            spans.push(Span::styled(
                HALF_BLOCK,
                Style::default().fg(fg).bg(bg_color),
            ));
        }

        lines.push(Line::from(spans));
    }

    lines
}
