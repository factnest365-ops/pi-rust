use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeKind {
    #[default]
    DefaultPi,
    TokyoNight,
    CatppuccinMacchiato,
    GruvboxDark,
    Nord,
    SolarizedDark,
    OneDark,
}

impl ThemeKind {
    pub const ALL: &'static [ThemeKind] = &[
        ThemeKind::DefaultPi,
        ThemeKind::TokyoNight,
        ThemeKind::CatppuccinMacchiato,
        ThemeKind::GruvboxDark,
        ThemeKind::Nord,
        ThemeKind::SolarizedDark,
        ThemeKind::OneDark,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ThemeKind::DefaultPi => "Default Pi",
            ThemeKind::TokyoNight => "Tokyo Night",
            ThemeKind::CatppuccinMacchiato => "Catppuccin Macchiato",
            ThemeKind::GruvboxDark => "Gruvbox Dark",
            ThemeKind::Nord => "Nord",
            ThemeKind::SolarizedDark => "Solarized Dark",
            ThemeKind::OneDark => "One Dark",
        }
    }

    pub fn id_str(&self) -> &'static str {
        match self {
            ThemeKind::DefaultPi => "default",
            ThemeKind::TokyoNight => "tokyonight",
            ThemeKind::CatppuccinMacchiato => "catppuccin",
            ThemeKind::GruvboxDark => "gruvbox",
            ThemeKind::Nord => "nord",
            ThemeKind::SolarizedDark => "solarized",
            ThemeKind::OneDark => "onedark",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ThemeKind::DefaultPi => "Clean GitHub dark with vibrant blue accents",
            ThemeKind::TokyoNight => "Deep indigo background with neon pastel accents",
            ThemeKind::CatppuccinMacchiato => {
                "Soothing macchiato palette with warm sapphire highlights"
            }
            ThemeKind::GruvboxDark => "Retro groove warm dark palette with earthy tones",
            ThemeKind::Nord => "Arctic ice blue with serene frosty hues",
            ThemeKind::SolarizedDark => "Precision dark solarized teal and cyan contrasts",
            ThemeKind::OneDark => "Iconic balanced dark aesthetic with vibrant syntax",
        }
    }

    pub fn parse(s: &str) -> Option<ThemeKind> {
        let clean = s.trim().to_lowercase().replace(['-', '_', ' '], "");
        match clean.as_str() {
            "default" | "defaultpi" | "pi" | "github" | "githubdark" => Some(ThemeKind::DefaultPi),
            "tokyonight" | "tokyo" | "night" => Some(ThemeKind::TokyoNight),
            "catppuccin" | "catppuccinmacchiato" | "macchiato" | "cat" => {
                Some(ThemeKind::CatppuccinMacchiato)
            }
            "gruvbox" | "gruvboxdark" | "groove" => Some(ThemeKind::GruvboxDark),
            "nord" | "arctic" | "frost" => Some(ThemeKind::Nord),
            "solarized" | "solarizeddark" | "solar" => Some(ThemeKind::SolarizedDark),
            "onedark" | "one" | "atom" => Some(ThemeKind::OneDark),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub kind: ThemeKind,
    pub accent: Color,
    pub bg: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub green: Color,
    pub red: Color,
    pub yellow: Color,
    pub cyan: Color,
    pub magenta: Color,
    pub logo_gradient: [Color; 6],
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self::default_pi()
    }
}

impl ThemePalette {
    pub fn from_kind(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::DefaultPi => Self::default_pi(),
            ThemeKind::TokyoNight => Self::tokyo_night(),
            ThemeKind::CatppuccinMacchiato => Self::catppuccin_macchiato(),
            ThemeKind::GruvboxDark => Self::gruvbox_dark(),
            ThemeKind::Nord => Self::nord(),
            ThemeKind::SolarizedDark => Self::solarized_dark(),
            ThemeKind::OneDark => Self::one_dark(),
        }
    }

    pub fn default_pi() -> Self {
        Self {
            kind: ThemeKind::DefaultPi,
            accent: Color::Rgb(47, 129, 247),   // #2f81f7
            bg: Color::Rgb(13, 17, 23),         // #0d1117
            surface: Color::Rgb(22, 27, 34),    // #161b22
            border: Color::Rgb(48, 54, 61),     // #30363d
            text: Color::Rgb(230, 237, 243),    // #e6edf3
            muted: Color::Rgb(125, 133, 144),   // #7d8590
            green: Color::Rgb(63, 185, 80),     // #3fb950
            red: Color::Rgb(248, 81, 73),       // #f85149
            yellow: Color::Rgb(210, 153, 34),   // #d29922
            cyan: Color::Rgb(56, 189, 248),     // #38bdf8
            magenta: Color::Rgb(188, 140, 255), // #bc8cff
            logo_gradient: [
                Color::Rgb(130, 190, 255),
                Color::Rgb(100, 170, 250),
                Color::Rgb(70, 150, 240),
                Color::Rgb(47, 129, 247),
                Color::Rgb(30, 100, 220),
                Color::Rgb(20, 80, 190),
            ],
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            kind: ThemeKind::TokyoNight,
            accent: Color::Rgb(122, 162, 247),  // #7aa2f7
            bg: Color::Rgb(26, 27, 38),         // #1a1b26
            surface: Color::Rgb(36, 40, 59),    // #24283b
            border: Color::Rgb(65, 72, 104),    // #414868
            text: Color::Rgb(192, 202, 245),    // #c0caf5
            muted: Color::Rgb(86, 95, 137),     // #565f89
            green: Color::Rgb(158, 206, 106),   // #9ece6a
            red: Color::Rgb(247, 118, 142),     // #f7768e
            yellow: Color::Rgb(224, 175, 104),  // #e0af68
            cyan: Color::Rgb(125, 207, 255),    // #7dcfff
            magenta: Color::Rgb(187, 154, 247), // #bb9af7
            logo_gradient: [
                Color::Rgb(187, 154, 247),
                Color::Rgb(158, 170, 250),
                Color::Rgb(122, 162, 247),
                Color::Rgb(125, 207, 255),
                Color::Rgb(90, 140, 230),
                Color::Rgb(65, 100, 200),
            ],
        }
    }

    pub fn catppuccin_macchiato() -> Self {
        Self {
            kind: ThemeKind::CatppuccinMacchiato,
            accent: Color::Rgb(138, 173, 244),  // Sapphire #8aadf4
            bg: Color::Rgb(36, 39, 58),         // Base #24273a
            surface: Color::Rgb(49, 50, 68),    // Mantle #313244
            border: Color::Rgb(91, 96, 120),    // Surface2 #5b6078
            text: Color::Rgb(202, 211, 245),    // Text #cad3f5
            muted: Color::Rgb(147, 154, 183),   // Subtext0 #939ab7
            green: Color::Rgb(166, 218, 149),   // Green #a6da95
            red: Color::Rgb(237, 135, 150),     // Red #ed8796
            yellow: Color::Rgb(238, 212, 159),  // Yellow #eed49f
            cyan: Color::Rgb(145, 215, 227),    // Sky #91d7e3
            magenta: Color::Rgb(245, 189, 230), // Pink #f5bde6
            logo_gradient: [
                Color::Rgb(245, 189, 230),
                Color::Rgb(198, 160, 246),
                Color::Rgb(138, 173, 244),
                Color::Rgb(145, 215, 227),
                Color::Rgb(125, 196, 228),
                Color::Rgb(110, 160, 210),
            ],
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            kind: ThemeKind::GruvboxDark,
            accent: Color::Rgb(254, 128, 25),   // Orange #fe8019
            bg: Color::Rgb(40, 40, 40),         // #282828
            surface: Color::Rgb(60, 56, 54),    // #3c3836
            border: Color::Rgb(80, 73, 69),     // #504945
            text: Color::Rgb(235, 219, 178),    // #ebdbb2
            muted: Color::Rgb(168, 153, 132),   // #a89984
            green: Color::Rgb(184, 187, 38),    // #b8bb26
            red: Color::Rgb(251, 73, 52),       // #fb4934
            yellow: Color::Rgb(250, 189, 47),   // #fabd2f
            cyan: Color::Rgb(142, 192, 124),    // #8ec07c
            magenta: Color::Rgb(211, 134, 155), // #d3869b
            logo_gradient: [
                Color::Rgb(250, 189, 47),
                Color::Rgb(254, 128, 25),
                Color::Rgb(251, 73, 52),
                Color::Rgb(211, 134, 155),
                Color::Rgb(177, 98, 134),
                Color::Rgb(143, 63, 113),
            ],
        }
    }

    pub fn nord() -> Self {
        Self {
            kind: ThemeKind::Nord,
            accent: Color::Rgb(136, 192, 208), // Frost Blue #88c0d0
            bg: Color::Rgb(46, 52, 64),        // Polar Night #2e3440
            surface: Color::Rgb(59, 66, 82),   // #3b4252
            border: Color::Rgb(76, 86, 106),   // #4c566a
            text: Color::Rgb(236, 239, 244),   // Snow Storm #eceff4
            muted: Color::Rgb(147, 160, 182),  // #93a0b6
            green: Color::Rgb(163, 190, 140),  // #a3be8c
            red: Color::Rgb(191, 97, 106),     // #bf616a
            yellow: Color::Rgb(235, 203, 139), // #ebcb8b
            cyan: Color::Rgb(143, 188, 187),   // #8fbcbb
            magenta: Color::Rgb(180, 142, 173), // #b48ead
            logo_gradient: [
                Color::Rgb(143, 188, 187),
                Color::Rgb(136, 192, 208),
                Color::Rgb(129, 161, 193),
                Color::Rgb(94, 129, 172),
                Color::Rgb(76, 108, 148),
                Color::Rgb(58, 88, 124),
            ],
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            kind: ThemeKind::SolarizedDark,
            accent: Color::Rgb(38, 139, 210),  // Blue #268bd2
            bg: Color::Rgb(0, 43, 54),         // base03 #002b36
            surface: Color::Rgb(7, 54, 66),    // base02 #073642
            border: Color::Rgb(88, 110, 117),  // base01 #586e75
            text: Color::Rgb(131, 148, 150),   // base0 #839496
            muted: Color::Rgb(101, 123, 131),  // base00 #657b83
            green: Color::Rgb(133, 153, 0),    // #859900
            red: Color::Rgb(220, 50, 47),      // #dc322f
            yellow: Color::Rgb(181, 137, 0),   // #b58900
            cyan: Color::Rgb(42, 161, 152),    // #2aa198
            magenta: Color::Rgb(211, 54, 130), // #d3869b
            logo_gradient: [
                Color::Rgb(42, 161, 152),
                Color::Rgb(38, 139, 210),
                Color::Rgb(108, 113, 196),
                Color::Rgb(211, 54, 130),
                Color::Rgb(170, 40, 100),
                Color::Rgb(130, 25, 75),
            ],
        }
    }

    pub fn one_dark() -> Self {
        Self {
            kind: ThemeKind::OneDark,
            accent: Color::Rgb(97, 175, 239),   // Blue #61afef
            bg: Color::Rgb(40, 44, 52),         // #282c34
            surface: Color::Rgb(33, 37, 43),    // #21252b
            border: Color::Rgb(75, 82, 99),     // #4b5263
            text: Color::Rgb(171, 178, 191),    // #abb2bf
            muted: Color::Rgb(92, 99, 112),     // #5c6370
            green: Color::Rgb(152, 195, 121),   // #98c379
            red: Color::Rgb(224, 108, 117),     // #e06c75
            yellow: Color::Rgb(229, 192, 123),  // #e5c07b
            cyan: Color::Rgb(86, 182, 194),     // #56b6c2
            magenta: Color::Rgb(198, 120, 221), // #c678dd
            logo_gradient: [
                Color::Rgb(198, 120, 221),
                Color::Rgb(97, 175, 239),
                Color::Rgb(86, 182, 194),
                Color::Rgb(152, 195, 121),
                Color::Rgb(110, 160, 90),
                Color::Rgb(70, 120, 60),
            ],
        }
    }

    pub fn logo_gradient(&self, line_index: usize) -> Style {
        let color = match line_index {
            0 => self.logo_gradient[0],
            1 => self.logo_gradient[1],
            2 => self.logo_gradient[2],
            3 => self.logo_gradient[3],
            4 => self.logo_gradient[4],
            _ => self.logo_gradient[5],
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn user_label(&self) -> Style {
        Style::default().fg(self.muted).add_modifier(Modifier::BOLD)
    }

    pub fn assistant_label(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tool_label(&self) -> Style {
        Style::default()
            .fg(self.yellow)
            .add_modifier(Modifier::BOLD)
    }

    pub fn system_label(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn system_content(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn code_border(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn prompt_separator(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn status_bar(&self) -> Style {
        Style::default().bg(self.bg).fg(self.muted)
    }

    pub fn status_bar_accent(&self) -> Style {
        Style::default().bg(self.bg).fg(self.text)
    }

    pub fn highlight_keyword(&self) -> Style {
        Style::default()
            .fg(self.yellow)
            .add_modifier(Modifier::BOLD)
    }

    pub fn highlight_type(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn highlight_string(&self) -> Style {
        Style::default().fg(self.green)
    }

    pub fn highlight_number(&self) -> Style {
        Style::default().fg(self.red)
    }

    pub fn highlight_comment(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Unified overlay chrome — G2 continuity: all modals share identical corner feel and padding
    pub fn overlay_block(&self, title: &str) -> ratatui::widgets::Block<'_> {
        use ratatui::widgets::{Block, BorderType, Borders};
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.border))
            .title(Span::styled(
                format!(" {} ", title.trim()),
                Style::default()
                    .fg(self.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(self.surface))
    }

    pub fn overlay_title_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }
}

pub struct Theme;

impl Theme {
    // GitHub Dark Default palette
    pub const ACCENT: Color = Color::Rgb(47, 129, 247); // #2f81f7
    pub const BG: Color = Color::Rgb(13, 17, 23); // #0d1117
    pub const SURFACE: Color = Color::Rgb(22, 27, 34); // #161b22
    pub const BORDER: Color = Color::Rgb(48, 54, 61); // #30363d
    pub const TEXT: Color = Color::Rgb(230, 237, 243); // #e6edf3
    pub const MUTED: Color = Color::Rgb(125, 133, 144); // #7d8590
    pub const GREEN: Color = Color::Rgb(63, 185, 80); // #3fb950
    pub const RED: Color = Color::Rgb(248, 81, 73); // #f85149
    pub const YELLOW: Color = Color::Rgb(210, 153, 34); // #d29922

    // Blue gradient stops for the PI logo (light->dark top-to-bottom)
    pub const LOGO_BLUE_1: Color = Color::Rgb(130, 190, 255); // lightest
    pub const LOGO_BLUE_2: Color = Color::Rgb(100, 170, 250);
    pub const LOGO_BLUE_3: Color = Color::Rgb(70, 150, 240);
    pub const LOGO_BLUE_4: Color = Color::Rgb(47, 129, 247); // accent
    pub const LOGO_BLUE_5: Color = Color::Rgb(30, 100, 220);
    pub const LOGO_BLUE_6: Color = Color::Rgb(20, 80, 190); // darkest

    pub fn logo_gradient(line_index: usize) -> Style {
        let color = match line_index {
            0 => Self::LOGO_BLUE_1,
            1 => Self::LOGO_BLUE_2,
            2 => Self::LOGO_BLUE_3,
            3 => Self::LOGO_BLUE_4,
            4 => Self::LOGO_BLUE_5,
            _ => Self::LOGO_BLUE_6,
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }

    pub fn title() -> Style {
        Style::default()
            .fg(Self::ACCENT)
            .add_modifier(Modifier::BOLD)
    }

    pub fn user_label() -> Style {
        Style::default()
            .fg(Self::MUTED)
            .add_modifier(Modifier::BOLD)
    }

    pub fn assistant_label() -> Style {
        Style::default()
            .fg(Self::ACCENT)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tool_label() -> Style {
        Style::default()
            .fg(Self::YELLOW)
            .add_modifier(Modifier::BOLD)
    }

    pub fn system_label() -> Style {
        Style::default()
            .fg(Self::ACCENT)
            .add_modifier(Modifier::BOLD)
    }

    pub fn system_content() -> Style {
        Style::default().fg(Self::MUTED)
    }

    pub fn code_border() -> Style {
        Style::default().fg(Self::BORDER)
    }

    pub fn prompt_separator() -> Style {
        Style::default().fg(Self::BORDER)
    }

    pub fn status_bar() -> Style {
        Style::default().bg(Self::BG).fg(Self::MUTED)
    }

    pub fn status_bar_accent() -> Style {
        Style::default().bg(Self::BG).fg(Self::TEXT)
    }

    pub fn highlight_keyword() -> Style {
        Style::default()
            .fg(Self::YELLOW)
            .add_modifier(Modifier::BOLD)
    }

    pub fn highlight_type() -> Style {
        Style::default().fg(Self::ACCENT)
    }

    pub fn highlight_string() -> Style {
        Style::default().fg(Self::GREEN)
    }

    pub fn highlight_number() -> Style {
        Style::default().fg(Self::RED)
    }

    pub fn highlight_comment() -> Style {
        Style::default().fg(Self::MUTED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_parsing() {
        assert_eq!(ThemeKind::parse("default"), Some(ThemeKind::DefaultPi));
        assert_eq!(ThemeKind::parse("tokyo-night"), Some(ThemeKind::TokyoNight));
        assert_eq!(
            ThemeKind::parse("catppuccin"),
            Some(ThemeKind::CatppuccinMacchiato)
        );
        assert_eq!(ThemeKind::parse("gruvbox"), Some(ThemeKind::GruvboxDark));
        assert_eq!(ThemeKind::parse("nord"), Some(ThemeKind::Nord));
        assert_eq!(
            ThemeKind::parse("solarized"),
            Some(ThemeKind::SolarizedDark)
        );
        assert_eq!(ThemeKind::parse("one-dark"), Some(ThemeKind::OneDark));
        assert_eq!(ThemeKind::parse("invalid-theme"), None);
    }

    #[test]
    fn test_all_themes_generate_palettes() {
        for kind in ThemeKind::ALL {
            let pal = ThemePalette::from_kind(*kind);
            assert_eq!(pal.kind, *kind);
            assert_eq!(pal.logo_gradient.len(), 6);
        }
    }
}
