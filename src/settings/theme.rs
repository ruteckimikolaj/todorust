use super::{ColorTheme, CustomThemeColors};
use ratatui::style::Color;

fn hex_to_color(hex: &str) -> Option<Color> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

pub struct Theme {
    pub high_color: Color,
    pub medium_color: Color,
    pub low_color: Color,
    pub done_color: Color,
    pub high_bg: Color,
    pub medium_bg: Color,
    pub low_bg: Color,
    pub accent_color: Color,
    pub base_fg: Color,
    pub base_bg: Color,
    pub highlight_bg: Color,
    pub help_text_fg: Color,
}

impl Theme {
    pub fn from_settings(theme_enum: ColorTheme, custom: Option<&CustomThemeColors>) -> Self {
        match theme_enum {
            ColorTheme::Default => Self::default(),
            ColorTheme::Dracula => Self::dracula(),
            ColorTheme::Solarized => Self::solarized(),
            ColorTheme::Nord => Self::nord(),
            ColorTheme::GruvboxDark => Self::gruvbox_dark(),
            ColorTheme::Cyberpunk => Self::cyberpunk(),
            ColorTheme::Custom => Self::from_custom(custom),
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            high_color: Color::Rgb(251, 73, 52),    // bright red   #fb4934
            medium_color: Color::Rgb(254, 128, 25), // bright orange #fe8019
            low_color: Color::Rgb(131, 165, 152),   // bright blue  #83a598
            done_color: Color::Rgb(142, 192, 124),  // bright aqua  #8ec07c
            high_bg: Color::Rgb(54, 36, 32),
            medium_bg: Color::Rgb(48, 40, 30),
            low_bg: Color::Rgb(32, 44, 52),
            accent_color: Color::Rgb(250, 189, 47), // bright yellow #fabd2f
            base_fg: Color::Rgb(235, 219, 178),     // fg #ebdbb2
            base_bg: Color::Rgb(40, 40, 40),        // bg #282828
            highlight_bg: Color::Rgb(60, 56, 54),   // bg1 #3c3836
            help_text_fg: Color::Rgb(146, 131, 116), // gray #928374
        }
    }

    pub fn cyberpunk() -> Self {
        Self {
            high_color: Color::Rgb(255, 45, 120),  // neon hot pink   #ff2d78
            medium_color: Color::Rgb(255, 109, 0), // neon orange     #ff6d00
            low_color: Color::Rgb(0, 255, 249),    // electric cyan   #00fff9
            done_color: Color::Rgb(57, 255, 20),   // matrix green    #39ff14
            high_bg: Color::Rgb(45, 0, 24),
            medium_bg: Color::Rgb(40, 16, 0),
            low_bg: Color::Rgb(0, 32, 40),
            accent_color: Color::Rgb(255, 230, 0), // neon yellow     #ffe600
            base_fg: Color::Rgb(226, 217, 243),    // soft lavender   #e2d9f3
            base_bg: Color::Rgb(13, 2, 33),        // deep void       #0d0221
            highlight_bg: Color::Rgb(30, 10, 60),  // deep purple     #1e0a3c
            help_text_fg: Color::Rgb(123, 104, 238), // medium slate    #7b68ee
        }
    }

    fn from_custom(custom: Option<&CustomThemeColors>) -> Self {
        let base = Self::default();
        let Some(c) = custom else {
            return base;
        };
        macro_rules! field {
            ($f:ident) => {
                c.$f.as_deref().and_then(hex_to_color).unwrap_or(base.$f)
            };
        }
        Self {
            high_color: field!(high_color),
            medium_color: field!(medium_color),
            low_color: field!(low_color),
            done_color: field!(done_color),
            high_bg: field!(high_bg),
            medium_bg: field!(medium_bg),
            low_bg: field!(low_bg),
            accent_color: field!(accent_color),
            base_fg: field!(base_fg),
            base_bg: field!(base_bg),
            highlight_bg: field!(highlight_bg),
            help_text_fg: field!(help_text_fg),
        }
    }

    pub fn dracula() -> Self {
        Self {
            high_color: Color::Rgb(255, 85, 85),     // red
            medium_color: Color::Rgb(255, 184, 108), // orange
            low_color: Color::Rgb(189, 147, 249),    // purple
            done_color: Color::Rgb(80, 250, 123),    // green
            high_bg: Color::Rgb(58, 38, 42),
            medium_bg: Color::Rgb(52, 44, 36),
            low_bg: Color::Rgb(42, 38, 62),
            accent_color: Color::Rgb(255, 121, 198), // pink — Dracula's brand color
            base_fg: Color::Rgb(248, 248, 242),
            base_bg: Color::Rgb(40, 42, 54),
            highlight_bg: Color::Rgb(68, 71, 90),
            help_text_fg: Color::Rgb(98, 114, 164),
        }
    }

    pub fn solarized() -> Self {
        Self {
            high_color: Color::Rgb(220, 50, 47),   // red
            medium_color: Color::Rgb(181, 137, 0), // yellow
            low_color: Color::Rgb(38, 139, 210),   // blue
            done_color: Color::Rgb(133, 153, 0),   // green
            high_bg: Color::Rgb(28, 36, 44),
            medium_bg: Color::Rgb(24, 40, 40),
            low_bg: Color::Rgb(8, 40, 60),
            accent_color: Color::Rgb(108, 113, 196), // violet — less aggressive than magenta
            base_fg: Color::Rgb(131, 148, 150),      // base0
            base_bg: Color::Rgb(0, 43, 54),          // base03
            highlight_bg: Color::Rgb(7, 54, 66),     // base02
            help_text_fg: Color::Rgb(88, 110, 117),  // base01
        }
    }

    pub fn nord() -> Self {
        Self {
            high_color: Color::Rgb(191, 97, 106),    // nord11 red
            medium_color: Color::Rgb(235, 203, 139), // nord13 yellow
            low_color: Color::Rgb(129, 161, 193),    // nord9 blue
            done_color: Color::Rgb(163, 190, 140),   // nord14 green
            high_bg: Color::Rgb(60, 46, 50),
            medium_bg: Color::Rgb(54, 50, 40),
            low_bg: Color::Rgb(44, 50, 68),
            accent_color: Color::Rgb(136, 192, 208), // nord8 frost — teal, no pink
            base_fg: Color::Rgb(216, 222, 233),      // nord4
            base_bg: Color::Rgb(46, 52, 64),         // nord0
            highlight_bg: Color::Rgb(59, 66, 82),    // nord1
            help_text_fg: Color::Rgb(76, 86, 106),   // nord3
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // Warm tomato red for high priority — not the terminal's LightRed which renders pink
            high_color: Color::Rgb(210, 70, 55),
            medium_color: Color::Rgb(205, 160, 55), // amber/gold
            low_color: Color::Rgb(75, 145, 210),    // calm blue
            done_color: Color::Rgb(80, 185, 105),   // green
            // Very subtle tints over near-black bg — just a hint of colour
            high_bg: Color::Rgb(38, 22, 20),
            medium_bg: Color::Rgb(34, 30, 18),
            low_bg: Color::Rgb(18, 24, 42),
            // Amber/gold accent — warm, neutral, no pink
            accent_color: Color::Rgb(210, 155, 50),
            base_fg: Color::Rgb(210, 210, 210),
            base_bg: Color::Rgb(18, 18, 22),
            // Highlight clearly different from bg, text stays readable
            highlight_bg: Color::Rgb(45, 52, 68),
            // Subdued but legible against near-black bg
            help_text_fg: Color::Rgb(110, 115, 130),
        }
    }
}
