use crossterm::event::{KeyCode, KeyModifiers};
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
    pub session_exited: Color,
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
            session_exited: Color::Rgb(0xCC, 0x55, 0x55),
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
            session_exited: Color::Rgb(0xCC, 0x44, 0x44),
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
    session_exited: Option<String>,
    warning: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct TerminalToml {
    start_at_bottom: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct WorktreeToml {
    dir_format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct KeybindingsToml {
    worktrees: Option<String>,
    terminal: Option<String>,
    files: Option<String>,
    editor: Option<String>,
    search: Option<String>,
    git_status: Option<String>,
    fuzzy_finder: Option<String>,
    project_search: Option<String>,
}

#[derive(Debug, Clone)]
pub struct KeybindingsConfig {
    pub worktrees: (KeyModifiers, KeyCode),
    pub terminal: (KeyModifiers, KeyCode),
    pub files: (KeyModifiers, KeyCode),
    pub editor: (KeyModifiers, KeyCode),
    pub search: (KeyModifiers, KeyCode),
    pub git_status: (KeyModifiers, KeyCode),
    pub fuzzy_finder: (KeyModifiers, KeyCode),
    pub project_search: (KeyModifiers, KeyCode),
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            worktrees: (KeyModifiers::CONTROL, KeyCode::Char('1')),
            terminal: (KeyModifiers::CONTROL, KeyCode::Char('2')),
            files: (KeyModifiers::CONTROL, KeyCode::Char('3')),
            editor: (KeyModifiers::CONTROL, KeyCode::Char('4')),
            search: (KeyModifiers::CONTROL, KeyCode::Char('5')),
            git_status: (KeyModifiers::CONTROL, KeyCode::Char('6')),
            fuzzy_finder: (KeyModifiers::CONTROL, KeyCode::Char('p')),
            project_search: (KeyModifiers::CONTROL, KeyCode::Char('f')),
        }
    }
}

impl KeybindingsConfig {
    /// Format a keybinding as a human-readable string (e.g. "Ctrl+1").
    pub fn format(binding: &(KeyModifiers, KeyCode)) -> String {
        let mut result = String::new();
        if binding.0.contains(KeyModifiers::CONTROL) {
            result.push_str("Ctrl+");
        }
        if binding.0.contains(KeyModifiers::ALT) {
            result.push_str("Alt+");
        }
        if binding.0.contains(KeyModifiers::SHIFT) {
            result.push_str("Shift+");
        }
        match binding.1 {
            KeyCode::Char(c) => {
                for uc in c.to_uppercase() {
                    result.push(uc);
                }
            }
            KeyCode::F(n) => result.push_str(&format!("F{}", n)),
            KeyCode::Enter => result.push_str("Enter"),
            KeyCode::Tab => result.push_str("Tab"),
            KeyCode::Esc => result.push_str("Esc"),
            _ => result.push('?'),
        }
        result
    }

    /// Check if a key event matches a binding.
    pub fn matches(binding: &(KeyModifiers, KeyCode), modifiers: KeyModifiers, code: KeyCode) -> bool {
        modifiers.contains(binding.0) && code == binding.1
    }
}

/// Parse a keybinding string like "ctrl+1" or "ctrl+p" into (KeyModifiers, KeyCode).
pub fn parse_keybinding(s: &str) -> Option<(KeyModifiers, KeyCode)> {
    let lowered = s.trim().to_lowercase();
    let parts: Vec<&str> = lowered.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut modifiers = KeyModifiers::NONE;
    for &part in &parts[..parts.len() - 1] {
        match part {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => return None,
        }
    }
    let key_str = parts.last()?;
    let code = match *key_str {
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "esc" | "escape" => KeyCode::Esc,
        s if s.len() == 1 => KeyCode::Char(s.chars().next()?),
        s if s.starts_with('f') => {
            let n: u8 = s[1..].parse().ok()?;
            KeyCode::F(n)
        }
        _ => return None,
    };
    Some((modifiers, code))
}

#[derive(Debug, Deserialize, Default)]
struct SessionToml {
    command: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigToml {
    theme: Option<ThemeToml>,
    terminal: Option<TerminalToml>,
    worktree: Option<WorktreeToml>,
    keybindings: Option<KeybindingsToml>,
    session: Option<SessionToml>,
}

pub const DEFAULT_WORKTREE_DIR_FORMAT: &str = "{repo}-{branch}";

/// Loaded application config (theme + terminal + keybinding settings).
pub struct AppConfig {
    pub theme: Theme,
    pub terminal_start_bottom: bool,
    pub worktree_dir_format: String,
    pub keybindings: KeybindingsConfig,
    pub session_command: String,
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

/// Load config from `~/.config/darya/config.toml`, falling back to defaults.
pub fn load_config() -> AppConfig {
    let mut theme = Theme::default();
    let mut terminal_start_bottom = true;
    let mut worktree_dir_format = DEFAULT_WORKTREE_DIR_FORMAT.to_string();
    let mut keybindings = KeybindingsConfig::default();
    let mut session_command = CLAUDE_COMMAND.to_string();

    let defaults = || AppConfig { theme: Theme::default(), terminal_start_bottom, worktree_dir_format: worktree_dir_format.clone(), keybindings: KeybindingsConfig::default(), session_command: CLAUDE_COMMAND.to_string() };

    let Some(home) = dirs_path() else {
        return defaults();
    };

    let config_path = home.join(".config").join("darya").join("config.toml");
    let Ok(contents) = std::fs::read_to_string(&config_path) else {
        return defaults();
    };

    let Ok(config) = toml::from_str::<ConfigToml>(&contents) else {
        eprintln!("Warning: failed to parse {}", config_path.display());
        return defaults();
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
        apply!(session_exited);
        apply!(warning);
    }

    if let Some(ref t) = config.terminal {
        if let Some(val) = t.start_at_bottom {
            terminal_start_bottom = val;
        }
    }

    if let Some(ref w) = config.worktree {
        if let Some(ref fmt) = w.dir_format {
            worktree_dir_format = fmt.clone();
        }
    }

    if let Some(ref kb) = config.keybindings {
        macro_rules! apply_kb {
            ($field:ident) => {
                if let Some(ref val) = kb.$field {
                    if let Some(binding) = parse_keybinding(val) {
                        keybindings.$field = binding;
                    }
                }
            };
        }
        apply_kb!(worktrees);
        apply_kb!(terminal);
        apply_kb!(files);
        apply_kb!(editor);
        apply_kb!(search);
        apply_kb!(git_status);
        apply_kb!(fuzzy_finder);
        apply_kb!(project_search);
    }

    if let Some(ref s) = config.session {
        if let Some(ref cmd) = s.command {
            session_command = cmd.clone();
        }
    }

    AppConfig { theme, terminal_start_bottom, worktree_dir_format, keybindings, session_command }
}

/// Resolve the session command for a worktree. Checks for a `.darya.toml`
/// override in the worktree root, falling back to the global config value.
pub fn resolve_session_command(worktree_path: &std::path::Path, global_command: &str) -> String {
    let local_config = worktree_path.join(".darya.toml");
    if let Ok(contents) = std::fs::read_to_string(&local_config) {
        if let Ok(config) = toml::from_str::<ConfigToml>(&contents) {
            if let Some(session) = config.session {
                if let Some(cmd) = session.command {
                    return cmd;
                }
            }
        }
    }
    global_command.to_string()
}

fn dirs_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Sync Claude Code's theme in `~/.claude.json` to match darya's theme mode.
/// Returns the original theme value so it can be restored later.
pub fn sync_claude_theme(mode: ThemeMode) -> Option<serde_json::Value> {
    let home = dirs_path()?;
    let config_path = home.join(".claude.json");

    let mut config: serde_json::Map<String, serde_json::Value> =
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

    let original = config.get("theme").cloned();

    let new_theme = match mode {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    };

    config.insert(
        "theme".to_string(),
        serde_json::Value::String(new_theme.to_string()),
    );

    if let Ok(json) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&config_path, json);
    }

    original
}

/// Restore the original theme value in `~/.claude.json`.
pub fn restore_claude_theme(original: Option<serde_json::Value>) {
    let Some(home) = dirs_path() else { return };
    let config_path = home.join(".claude.json");

    let mut config: serde_json::Map<String, serde_json::Value> =
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

    match original {
        Some(val) => {
            config.insert("theme".to_string(), val);
        }
        None => {
            config.remove("theme");
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&config_path, json);
    }
}
