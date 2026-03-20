use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::config::{Theme, ThemeMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanetKind {
    Earth,
    Mars,
    Venus,
    Neptune,
    Jupiter,
    Pluto,
}

impl PlanetKind {
    pub fn all() -> &'static [PlanetKind] {
        &[
            PlanetKind::Earth,
            PlanetKind::Mars,
            PlanetKind::Venus,
            PlanetKind::Neptune,
            PlanetKind::Jupiter,
            PlanetKind::Pluto,
        ]
    }

    pub fn name(&self) -> &str {
        match self {
            PlanetKind::Earth => "earth",
            PlanetKind::Mars => "mars",
            PlanetKind::Venus => "venus",
            PlanetKind::Neptune => "neptune",
            PlanetKind::Jupiter => "jupiter",
            PlanetKind::Pluto => "pluto",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            PlanetKind::Earth => "Earth",
            PlanetKind::Mars => "Mars",
            PlanetKind::Venus => "Venus",
            PlanetKind::Neptune => "Neptune",
            PlanetKind::Jupiter => "Jupiter",
            PlanetKind::Pluto => "Pluto",
        }
    }

    pub fn parse(s: &str) -> Option<PlanetKind> {
        match s.to_lowercase().as_str() {
            "earth" => Some(PlanetKind::Earth),
            "mars" => Some(PlanetKind::Mars),
            "venus" => Some(PlanetKind::Venus),
            "neptune" => Some(PlanetKind::Neptune),
            "jupiter" => Some(PlanetKind::Jupiter),
            "pluto" => Some(PlanetKind::Pluto),
            _ => None,
        }
    }

    /// Accent color for this planet (used as the primary theme accent).
    pub fn accent(&self) -> Color {
        match self {
            PlanetKind::Earth => Color::Rgb(0x3A, 0x8F, 0xD4), // ocean blue
            PlanetKind::Mars => Color::Rgb(0xCC, 0x44, 0x22),  // rust red
            PlanetKind::Venus => Color::Rgb(0xE0, 0xA8, 0x4A), // golden
            PlanetKind::Neptune => Color::Rgb(0x4A, 0x7A, 0xCC), // deep blue
            PlanetKind::Jupiter => Color::Rgb(0xD4, 0x94, 0x4A), // storm orange
            PlanetKind::Pluto => Color::Rgb(0x9A, 0x8A, 0xAA), // ice purple
        }
    }

    pub fn dark_theme(&self) -> Theme {
        let accent = self.accent();
        let (ar, ag, ab) = match accent {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => unreachable!(),
        };

        // Status bar fg: dark bg for contrast on accent bg
        let status_fg = Color::Rgb(0x1A, 0x1A, 0x1A);

        Theme {
            mode: ThemeMode::Dark,
            bg: Color::Rgb(0x1A, 0x1A, 0x1A),
            fg: Color::Rgb(0xD0, 0xD0, 0xD0),
            fg_dim: Color::Rgb(0x5A, 0x5A, 0x5A),
            border_active: accent,
            border_inactive: Color::Rgb(0x3A, 0x3A, 0x3A),
            highlight_bg: Color::Rgb(0x2A, 0x2A, 0x2A),
            session_active: accent,
            session_inactive: Color::Rgb(0x5A, 0x5A, 0x5A),
            status_bar_fg: status_fg,
            status_bar_bg: accent,
            prompt_border: accent,
            prompt_delete_border: Color::Rgb(0xCC, 0x55, 0x55),
            session_attention: Color::Rgb(0x00, 0xDD, 0x00),
            session_exited: Color::Rgb(0xCC, 0x55, 0x55),
            warning: Color::Rgb(
                ar.saturating_add(0x20).min(0xE0),
                ag.saturating_add(0x20).min(0xE0),
                ab.min(0x60),
            ),
        }
    }

    pub fn light_theme(&self) -> Theme {
        let accent = self.accent();
        let (ar, ag, ab) = match accent {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => unreachable!(),
        };

        // Slightly darker accent for light mode
        let dark_accent = Color::Rgb(
            ar.saturating_sub(0x10),
            ag.saturating_sub(0x10),
            ab.saturating_sub(0x10),
        );

        Theme {
            mode: ThemeMode::Light,
            bg: Color::Rgb(0xC8, 0xC3, 0xBE),
            fg: Color::Rgb(0x2A, 0x2A, 0x2A),
            fg_dim: Color::Rgb(0x7A, 0x74, 0x6E),
            border_active: dark_accent,
            border_inactive: Color::Rgb(0xA0, 0x9A, 0x94),
            highlight_bg: Color::Rgb(0xB8, 0xB3, 0xAE),
            session_active: dark_accent,
            session_inactive: Color::Rgb(0x9A, 0x90, 0x88),
            status_bar_fg: Color::Rgb(0xF5, 0xF0, 0xEB),
            status_bar_bg: dark_accent,
            prompt_border: dark_accent,
            prompt_delete_border: Color::Rgb(0xCC, 0x44, 0x44),
            session_attention: Color::Rgb(0x00, 0x99, 0x00),
            session_exited: Color::Rgb(0xCC, 0x44, 0x44),
            warning: Color::Rgb(
                ar.saturating_sub(0x20).max(0x80),
                ag.saturating_sub(0x20).max(0x60),
                ab.min(0x40),
            ),
        }
    }
}
