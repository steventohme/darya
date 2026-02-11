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
