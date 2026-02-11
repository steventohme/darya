use ratatui::style::Color;

pub const TICK_RATE_MS: u64 = 50;
pub const CLAUDE_COMMAND: &str = "claude";

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub border_active: Color,
    pub border_inactive: Color,
    pub highlight_bg: Color,
    pub session_active: Color,
    pub session_inactive: Color,
    pub status_bar_fg: Color,
    pub status_bar_bg: Color,
    pub prompt_border: Color,
    pub prompt_delete_border: Color,
    pub warning: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(0x1A, 0x1A, 0x1A),
            fg: Color::Rgb(0xD0, 0xD0, 0xD0),
            fg_dim: Color::Rgb(0x5A, 0x5A, 0x5A),
            border_active: Color::Rgb(0xE0, 0x7A, 0x2A),
            border_inactive: Color::Rgb(0x3A, 0x3A, 0x3A),
            highlight_bg: Color::Rgb(0x2A, 0x2A, 0x2A),
            session_active: Color::Rgb(0xE0, 0x7A, 0x2A),
            session_inactive: Color::Rgb(0x5A, 0x5A, 0x5A),
            status_bar_fg: Color::Rgb(0x1A, 0x1A, 0x1A),
            status_bar_bg: Color::Rgb(0xE0, 0x7A, 0x2A),
            prompt_border: Color::Rgb(0xE0, 0x7A, 0x2A),
            prompt_delete_border: Color::Rgb(0xCC, 0x55, 0x55),
            warning: Color::Rgb(0xE0, 0xA0, 0x3A),
        }
    }
}
