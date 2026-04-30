use ratatui::style::Color;

pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub surface: Color,
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
    pub highlight: Color,
    pub muted: Color,
    pub critical_colors: [Color; 4],
}

pub const THEMES: &[Theme] = &[
    Theme {
        name: "dark",
        bg: Color::Rgb(30, 30, 46),
        fg: Color::Rgb(205, 214, 244),
        surface: Color::Rgb(24, 24, 37),
        primary: Color::Rgb(137, 180, 250),
        secondary: Color::Rgb(245, 194, 231),
        success: Color::Rgb(166, 227, 161),
        warning: Color::Rgb(249, 226, 175),
        error: Color::Rgb(243, 139, 168),
        border: Color::Rgb(88, 91, 112),
        highlight: Color::Rgb(203, 166, 126),
        muted: Color::Rgb(108, 112, 134),
        critical_colors: [
            Color::Rgb(137, 180, 250), // Low - blue
            Color::Rgb(249, 226, 175), // Medium - yellow
            Color::Rgb(245, 194, 231), // High - pink
            Color::Rgb(243, 139, 168), // Critical - red
        ],
    },
    Theme {
        name: "light",
        bg: Color::Rgb(250, 250, 250),
        fg: Color::Rgb(40, 40, 40),
        surface: Color::Rgb(240, 240, 240),
        primary: Color::Rgb(30, 111, 159),
        secondary: Color::Rgb(143, 88, 150),
        success: Color::Rgb(34, 139, 34),
        warning: Color::Rgb(218, 165, 32),
        error: Color::Rgb(178, 34, 34),
        border: Color::Rgb(180, 180, 180),
        highlight: Color::Rgb(210, 105, 30),
        muted: Color::Rgb(118, 118, 118),
        critical_colors: [
            Color::Rgb(30, 111, 159),
            Color::Rgb(218, 165, 32),
            Color::Rgb(255, 140, 0),
            Color::Rgb(178, 34, 34),
        ],
    },
    Theme {
        name: "solarized",
        bg: Color::Rgb(0, 43, 54),
        fg: Color::Rgb(131, 148, 150),
        surface: Color::Rgb(7, 54, 66),
        primary: Color::Rgb(38, 139, 210),
        secondary: Color::Rgb(211, 54, 130),
        success: Color::Rgb(133, 153, 0),
        warning: Color::Rgb(181, 137, 0),
        error: Color::Rgb(220, 50, 47),
        border: Color::Rgb(7, 54, 66),
        highlight: Color::Rgb(42, 161, 152),
        muted: Color::Rgb(88, 110, 117),
        critical_colors: [
            Color::Rgb(38, 139, 210),
            Color::Rgb(181, 137, 0),
            Color::Rgb(203, 75, 22),
            Color::Rgb(220, 50, 47),
        ],
    },
    Theme {
        name: "dracula",
        bg: Color::Rgb(40, 42, 54),
        fg: Color::Rgb(248, 248, 242),
        surface: Color::Rgb(33, 34, 44),
        primary: Color::Rgb(139, 233, 253),
        secondary: Color::Rgb(255, 121, 198),
        success: Color::Rgb(80, 250, 123),
        warning: Color::Rgb(241, 250, 140),
        error: Color::Rgb(255, 85, 85),
        border: Color::Rgb(68, 71, 90),
        highlight: Color::Rgb(189, 147, 249),
        muted: Color::Rgb(98, 114, 164),
        critical_colors: [
            Color::Rgb(139, 233, 253),
            Color::Rgb(241, 250, 140),
            Color::Rgb(255, 184, 108),
            Color::Rgb(255, 85, 85),
        ],
    },
    Theme {
        name: "monokai",
        bg: Color::Rgb(39, 40, 34),
        fg: Color::Rgb(248, 248, 242),
        surface: Color::Rgb(31, 32, 27),
        primary: Color::Rgb(102, 217, 239),
        secondary: Color::Rgb(249, 38, 114),
        success: Color::Rgb(166, 226, 46),
        warning: Color::Rgb(253, 151, 31),
        error: Color::Rgb(249, 38, 114),
        border: Color::Rgb(73, 72, 62),
        highlight: Color::Rgb(174, 129, 255),
        muted: Color::Rgb(117, 113, 94),
        critical_colors: [
            Color::Rgb(102, 217, 239),
            Color::Rgb(253, 151, 31),
            Color::Rgb(249, 38, 114),
            Color::Rgb(255, 0, 0),
        ],
    },
    Theme {
        name: "ansi",
        bg: Color::Reset,
        fg: Color::White,
        surface: Color::Reset,
        primary: Color::Blue,
        secondary: Color::Magenta,
        success: Color::Green,
        warning: Color::Yellow,
        error: Color::Red,
        border: Color::DarkGray,
        highlight: Color::Cyan,
        muted: Color::Gray,
        critical_colors: [Color::Blue, Color::Yellow, Color::Magenta, Color::Red],
    },
    Theme {
        name: "plain",
        bg: Color::Reset,
        fg: Color::Reset,
        surface: Color::Reset,
        primary: Color::Reset,
        secondary: Color::Reset,
        success: Color::Reset,
        warning: Color::Reset,
        error: Color::Reset,
        border: Color::Reset,
        highlight: Color::Reset,
        muted: Color::Reset,
        critical_colors: [Color::Reset, Color::Reset, Color::Reset, Color::Reset],
    },
];

pub fn get_theme(name: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.name == name).unwrap_or(&THEMES[0])
}

pub fn get_runtime_theme(name: &str) -> &'static Theme {
    if std::env::var_os("NO_COLOR").is_some() {
        get_theme("plain")
    } else {
        get_theme(name)
    }
}

pub fn theme_names() -> Vec<&'static str> {
    THEMES.iter().map(|t| t.name).collect()
}

pub fn criticality_color(theme: &Theme, criticality: crate::types::Criticality) -> Color {
    match criticality {
        crate::types::Criticality::Low => theme.critical_colors[0],
        crate::types::Criticality::Medium => theme.critical_colors[1],
        crate::types::Criticality::High => theme.critical_colors[2],
        crate::types::Criticality::Critical => theme.critical_colors[3],
    }
}

pub fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_prefers_plain_theme() {
        std::env::set_var("NO_COLOR", "1");
        assert_eq!(get_runtime_theme("dark").name, "plain");
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn ansi_theme_uses_terminal_palette_colors() {
        let theme = get_theme("ansi");
        assert_eq!(theme.primary, Color::Blue);
        assert_eq!(theme.success, Color::Green);
        assert_eq!(theme.warning, Color::Yellow);
        assert_eq!(theme.error, Color::Red);
    }
}
