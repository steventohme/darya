use ratatui::style::Color;
use serde::Deserialize;

pub const TICK_RATE_MS: u64 = 50;
pub const CLAUDE_COMMAND: &str = "claude";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub mode: ThemeMode,
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
    pub session_attention: Color,
    pub warning: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
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
            session_attention: Color::Rgb(0x00, 0xDD, 0x00),
            warning: Color::Rgb(0xE0, 0xA0, 0x3A),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg: Color::Rgb(0xC8, 0xC3, 0xBE),
            fg: Color::Rgb(0x2A, 0x2A, 0x2A),
            fg_dim: Color::Rgb(0x7A, 0x74, 0x6E),
            border_active: Color::Rgb(0xD0, 0x6B, 0x1A),
            border_inactive: Color::Rgb(0xA0, 0x9A, 0x94),
            highlight_bg: Color::Rgb(0xB8, 0xB3, 0xAE),
            session_active: Color::Rgb(0xD0, 0x6B, 0x1A),
            session_inactive: Color::Rgb(0x9A, 0x90, 0x88),
            status_bar_fg: Color::Rgb(0xF5, 0xF0, 0xEB),
            status_bar_bg: Color::Rgb(0xD0, 0x6B, 0x1A),
            prompt_border: Color::Rgb(0xD0, 0x6B, 0x1A),
            prompt_delete_border: Color::Rgb(0xCC, 0x44, 0x44),
            session_attention: Color::Rgb(0x00, 0x99, 0x00),
            warning: Color::Rgb(0xC0, 0x8A, 0x20),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Raw TOML representation — all fields optional so partial configs work.
#[derive(Debug, Deserialize, Default)]
struct ThemeToml {
    mode: Option<String>,
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
    session_attention: Option<String>,
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

/// Load theme from `~/.config/darya/config.toml`, falling back to defaults.
pub fn load_theme() -> Theme {
    let mut theme = Theme::default();

    let Some(home) = dirs_path() else {
        return theme;
    };

    let config_path = home.join(".config").join("darya").join("config.toml");
    let Ok(contents) = std::fs::read_to_string(&config_path) else {
        return theme;
    };

    let Ok(config) = toml::from_str::<ConfigToml>(&contents) else {
        eprintln!("Warning: failed to parse {}", config_path.display());
        return theme;
    };

    if let Some(ref t) = config.theme {
        match t.mode.as_deref() {
            Some("light") => theme = Theme::light(),
            _ => {} // dark is already the default
        }
    }

    if let Some(t) = config.theme {
        macro_rules! apply {
            ($field:ident) => {
                if let Some(ref val) = t.$field {
                    if let Some(color) = parse_hex_color(val) {
                        theme.$field = color;
                    }
                }
            };
        }
        apply!(bg);
        apply!(fg);
        apply!(fg_dim);
        apply!(border_active);
        apply!(border_inactive);
        apply!(highlight_bg);
        apply!(session_active);
        apply!(session_inactive);
        apply!(status_bar_fg);
        apply!(status_bar_bg);
        apply!(prompt_border);
        apply!(prompt_delete_border);
        apply!(session_attention);
        apply!(warning);
    }

    theme
}

fn dirs_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}
