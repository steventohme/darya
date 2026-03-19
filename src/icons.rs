use ratatui::style::Color;

pub struct FileIcon {
    pub icon: char,
    pub color: Color,
}

pub fn file_icon(filename: &str, is_dir: bool) -> FileIcon {
    if is_dir {
        return FileIcon {
            icon: '\u{f07b}',
            color: Color::Rgb(0xE0, 0x7A, 0x2A),
        };
    }

    let lower = filename.to_lowercase();

    // Exact filename matches
    match lower.as_str() {
        "dockerfile" | "dockerfile.dev" | "dockerfile.prod" => {
            return FileIcon { icon: '\u{e7b0}', color: Color::Rgb(0x23, 0x96, 0xED) };
        }
        ".gitignore" | ".gitattributes" | ".gitmodules" => {
            return FileIcon { icon: '\u{e702}', color: Color::Rgb(0xF0, 0x50, 0x33) };
        }
        ".env" | ".env.local" | ".env.development" | ".env.production" => {
            return FileIcon { icon: '\u{f462}', color: Color::Rgb(0xFA, 0xF7, 0x43) };
        }
        _ => {}
    }

    // Extension-based lookup
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => FileIcon { icon: '\u{e7a8}', color: Color::Rgb(0xDE, 0xA5, 0x84) },
        "py" => FileIcon { icon: '\u{e73c}', color: Color::Rgb(0x35, 0x72, 0xA5) },
        "go" => FileIcon { icon: '\u{e627}', color: Color::Rgb(0x00, 0xAD, 0xD8) },
        "ts" => FileIcon { icon: '\u{e628}', color: Color::Rgb(0x31, 0x78, 0xC6) },
        "tsx" | "jsx" => FileIcon { icon: '\u{e7ba}', color: Color::Rgb(0x61, 0xDA, 0xFB) },
        "js" => FileIcon { icon: '\u{e74e}', color: Color::Rgb(0xF7, 0xDF, 0x1E) },
        "sql" => FileIcon { icon: '\u{e706}', color: Color::Rgb(0xE4, 0x8E, 0x00) },
        "graphql" | "gql" => FileIcon { icon: '\u{e662}', color: Color::Rgb(0xE1, 0x35, 0xAB) },
        "md" => FileIcon { icon: '\u{e73e}', color: Color::Rgb(0x51, 0x9A, 0xBA) },
        "json" => FileIcon { icon: '\u{e60b}', color: Color::Rgb(0xCB, 0xCB, 0x41) },
        "yaml" | "yml" => FileIcon { icon: '\u{e6a8}', color: Color::Rgb(0xCB, 0x17, 0x1E) },
        "toml" => FileIcon { icon: '\u{f0ad}', color: Color::Rgb(0x9C, 0x40, 0x36) },
        "html" => FileIcon { icon: '\u{e736}', color: Color::Rgb(0xE4, 0x4D, 0x26) },
        "css" => FileIcon { icon: '\u{e749}', color: Color::Rgb(0x56, 0x3D, 0x7C) },
        "sh" | "bash" | "zsh" => FileIcon { icon: '\u{e795}', color: Color::Rgb(0x89, 0xE0, 0x51) },
        _ => FileIcon { icon: '\u{f15b}', color: Color::Rgb(0xD0, 0xD0, 0xD0) },
    }
}

/// Returns an open-folder icon for expanded directories.
pub fn dir_icon_open() -> FileIcon {
    FileIcon {
        icon: '\u{f07c}',
        color: Color::Rgb(0xE0, 0x7A, 0x2A),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extensions() {
        let rs = file_icon("main.rs", false);
        assert_eq!(rs.icon, '\u{e7a8}');

        let py = file_icon("script.py", false);
        assert_eq!(py.icon, '\u{e73c}');

        let js = file_icon("index.js", false);
        assert_eq!(js.icon, '\u{e74e}');
    }

    #[test]
    fn exact_filename_matches() {
        let docker = file_icon("Dockerfile", false);
        assert_eq!(docker.icon, '\u{e7b0}');

        let gitignore = file_icon(".gitignore", false);
        assert_eq!(gitignore.icon, '\u{e702}');

        let env = file_icon(".env", false);
        assert_eq!(env.icon, '\u{f462}');
    }

    #[test]
    fn case_insensitivity() {
        let tsx = file_icon("App.TSX", false);
        assert_eq!(tsx.icon, '\u{e7ba}');

        let docker = file_icon("DOCKERFILE", false);
        assert_eq!(docker.icon, '\u{e7b0}');
    }

    #[test]
    fn unknown_extension_gets_default() {
        let unknown = file_icon("data.xyz", false);
        assert_eq!(unknown.icon, '\u{f15b}');
    }

    #[test]
    fn directory_overrides_extension() {
        let dir = file_icon("src.rs", true);
        assert_eq!(dir.icon, '\u{f07b}');
    }
}
