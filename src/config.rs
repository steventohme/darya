use ratatui::style::Color;
use serde::Deserialize;

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

/// Raw TOML representation — all fields optional so partial configs work.
#[derive(Debug, Deserialize, Default)]
struct ThemeToml {
    bg: Option<String>,
    fg: Option<String>,
    fg_dim: Option<String>,
    border_active: Option<String>,
    border_inactive: Option<String>,
    highlight_bg: Option<String>,
    session_active: Option<String>,
    session_inactive: Option<String>,
    status_bar_fg: Option<String>,
    status_bar_bg: Option<String>,
    prompt_border: Option<String>,
    prompt_delete_border: Option<String>,
    warning: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigToml {
    theme: Option<ThemeToml>,
}

/// Parse a hex color string like "#33FF33" or "33FF33" into a ratatui Color.
fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
