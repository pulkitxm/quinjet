#![expect(
    clippy::unreadable_literal,
    reason = "six-digit hexadecimal RGB values are clearest without separators"
)]

use std::time::Duration;

use clap::ValueEnum;
use ratatui::style::Color;

const SYSTEM_APPEARANCE_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum ThemeName {
    #[default]
    Quinjet,
    Catppuccin,
    Dracula,
    Everforest,
    Gruvbox,
    Nord,
    One,
    RosePine,
    Solarized,
    TokyoNight,
    Ayu,
    Monokai,
}

impl ThemeName {
    pub(crate) const ALL: [Self; 12] = [
        Self::Quinjet,
        Self::Catppuccin,
        Self::Dracula,
        Self::Everforest,
        Self::Gruvbox,
        Self::Nord,
        Self::One,
        Self::RosePine,
        Self::Solarized,
        Self::TokyoNight,
        Self::Ayu,
        Self::Monokai,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Quinjet => "Quinjet",
            Self::Catppuccin => "Catppuccin",
            Self::Dracula => "Dracula",
            Self::Everforest => "Everforest",
            Self::Gruvbox => "Gruvbox",
            Self::Nord => "Nord",
            Self::One => "One",
            Self::RosePine => "Rosé Pine",
            Self::Solarized => "Solarized",
            Self::TokyoNight => "Tokyo Night",
            Self::Ayu => "Ayu",
            Self::Monokai => "Monokai",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum AppearanceChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearanceChoice {
    pub(crate) const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Appearance {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    pub background: Color,
    pub panel: Color,
    pub panel_alt: Color,
    pub border: Color,
    pub border_focus: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub selected: Color,
    pub added: Color,
    pub added_background: Color,
    pub added_emphasis_background: Color,
    pub removed: Color,
    pub removed_background: Color,
    pub removed_emphasis_background: Color,
    pub modified: Color,
    pub conflict: Color,
    pub error: Color,
    pub success: Color,
    pub syntax: [Color; 10],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SyntaxColor {
    Text,
    Comment,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Brown,
}

impl AppearanceChoice {
    pub(crate) fn resolve(self) -> Appearance {
        match self {
            Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
            Self::System => system_appearance(detect_system_appearance()),
        }
    }
}

fn detect_system_appearance() -> Option<dark_light::Mode> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    drop(
        std::thread::Builder::new()
            .name("system-appearance".to_owned())
            .spawn(move || sender.send(dark_light::detect().ok()).unwrap_or_else(drop))
            .ok()?,
    );
    receiver.recv_timeout(SYSTEM_APPEARANCE_TIMEOUT).ok()?
}

const fn system_appearance(mode: Option<dark_light::Mode>) -> Appearance {
    match mode {
        Some(dark_light::Mode::Light) => Appearance::Light,
        Some(dark_light::Mode::Dark | dark_light::Mode::Unspecified) | None => Appearance::Dark,
    }
}

impl Theme {
    pub(crate) fn new(name: ThemeName, appearance: Appearance) -> Self {
        let palette = palette(name, appearance);
        let background = color(palette[0]);
        let panel = color(palette[1]);
        let panel_alt = color(palette[2]);
        let accent_base = color(palette[13]);
        let accent_soft = blend(palette[13], palette[0], 38);
        let selected = blend(
            palette[5],
            palette[0],
            if appearance == Appearance::Light {
                40
            } else {
                24
            },
        );
        let added_background = blend(palette[11], palette[0], 14);
        let added_emphasis_background = blend(palette[11], palette[0], 27);
        let removed_background = blend(palette[8], palette[0], 14);
        let removed_emphasis_background = blend(palette[8], palette[0], 27);
        let surfaces = [
            background,
            panel,
            panel_alt,
            accent_soft,
            selected,
            added_background,
            added_emphasis_background,
            removed_background,
            removed_emphasis_background,
        ];
        let muted = readable(color(palette[4]), &surfaces, appearance, 4.5);
        let text = readable(
            color(palette[5]),
            &surfaces,
            appearance,
            contrast(muted, background).max(4.5),
        );
        let accent = readable(accent_base, &surfaces, appearance, 4.5);
        let added = readable(color(palette[11]), &surfaces, appearance, 4.5);
        let removed = readable(color(palette[8]), &surfaces, appearance, 4.5);
        let modified = readable(color(palette[10]), &surfaces, appearance, 4.5);
        let conflict = readable(color(palette[9]), &surfaces, appearance, 4.5);
        Self {
            background,
            panel,
            panel_alt,
            border: readable(color(palette[3]), &surfaces, appearance, 3.0),
            border_focus: accent,
            text,
            muted,
            accent,
            accent_soft,
            selected,
            added,
            added_background,
            added_emphasis_background,
            removed,
            removed_background,
            removed_emphasis_background,
            modified,
            conflict,
            error: removed,
            success: added,
            syntax: [
                text,
                readable(color(palette[3]), &surfaces, appearance, 4.5),
                removed,
                conflict,
                modified,
                added,
                readable(color(palette[12]), &surfaces, appearance, 4.5),
                accent,
                readable(color(palette[14]), &surfaces, appearance, 4.5),
                readable(color(palette[15]), &surfaces, appearance, 4.5),
            ],
        }
    }

    pub(crate) const fn syntax(&self, color: SyntaxColor) -> Color {
        match color {
            SyntaxColor::Text => self.syntax[0],
            SyntaxColor::Comment => self.syntax[1],
            SyntaxColor::Red => self.syntax[2],
            SyntaxColor::Orange => self.syntax[3],
            SyntaxColor::Yellow => self.syntax[4],
            SyntaxColor::Green => self.syntax[5],
            SyntaxColor::Cyan => self.syntax[6],
            SyntaxColor::Blue => self.syntax[7],
            SyntaxColor::Purple => self.syntax[8],
            SyntaxColor::Brown => self.syntax[9],
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(ThemeName::Quinjet, Appearance::Dark)
    }
}

const fn color(value: u32) -> Color {
    Color::Rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::integer_division,
    reason = "bounded integer RGB interpolation intentionally rounds down to a byte"
)]
const fn blend(foreground: u32, background: u32, amount: u32) -> Color {
    let inverse = 100 - amount;
    let red = (((foreground >> 16) & 0xff) * amount + ((background >> 16) & 0xff) * inverse) / 100;
    let green = (((foreground >> 8) & 0xff) * amount + ((background >> 8) & 0xff) * inverse) / 100;
    let blue = ((foreground & 0xff) * amount + (background & 0xff) * inverse) / 100;
    Color::Rgb(red as u8, green as u8, blue as u8)
}

fn readable(
    mut foreground: Color,
    backgrounds: &[Color],
    appearance: Appearance,
    minimum: f64,
) -> Color {
    let target = match appearance {
        Appearance::Light => 0,
        Appearance::Dark => 0x00ff_ffff,
    };
    for _ in 0..64 {
        if backgrounds
            .iter()
            .all(|background| contrast(foreground, *background) >= minimum)
        {
            return foreground;
        }
        foreground = blend(color_value(foreground), target, 96);
    }
    color(target)
}

const fn color_value(color: Color) -> u32 {
    match color {
        Color::Rgb(red, green, blue) => (red as u32) << 16 | (green as u32) << 8 | blue as u32,
        _ => 0,
    }
}

fn contrast(first: Color, second: Color) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn luminance(color: Color) -> f64 {
    let Color::Rgb(red, green, blue) = color else {
        return 0.0;
    };
    0.0722_f64.mul_add(
        linear_channel(blue),
        0.7152_f64.mul_add(linear_channel(green), 0.2126 * linear_channel(red)),
    )
}

fn linear_channel(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

const fn palette(name: ThemeName, appearance: Appearance) -> &'static [u32; 16] {
    match (name, appearance) {
        (ThemeName::Quinjet, Appearance::Dark) => &QUINJET_DARK,
        (ThemeName::Quinjet, Appearance::Light) => &QUINJET_LIGHT,
        (ThemeName::Catppuccin, Appearance::Dark) => &CATPPUCCIN_DARK,
        (ThemeName::Catppuccin, Appearance::Light) => &CATPPUCCIN_LIGHT,
        (ThemeName::Dracula, Appearance::Dark) => &DRACULA_DARK,
        (ThemeName::Dracula, Appearance::Light) => &DRACULA_LIGHT,
        (ThemeName::Everforest, Appearance::Dark) => &EVERFOREST_DARK,
        (ThemeName::Everforest, Appearance::Light) => &EVERFOREST_LIGHT,
        (ThemeName::Gruvbox, Appearance::Dark) => &GRUVBOX_DARK,
        (ThemeName::Gruvbox, Appearance::Light) => &GRUVBOX_LIGHT,
        (ThemeName::Nord, Appearance::Dark) => &NORD_DARK,
        (ThemeName::Nord, Appearance::Light) => &NORD_LIGHT,
        (ThemeName::One, Appearance::Dark) => &ONE_DARK,
        (ThemeName::One, Appearance::Light) => &ONE_LIGHT,
        (ThemeName::RosePine, Appearance::Dark) => &ROSE_PINE_DARK,
        (ThemeName::RosePine, Appearance::Light) => &ROSE_PINE_LIGHT,
        (ThemeName::Solarized, Appearance::Dark) => &SOLARIZED_DARK,
        (ThemeName::Solarized, Appearance::Light) => &SOLARIZED_LIGHT,
        (ThemeName::TokyoNight, Appearance::Dark) => &TOKYO_NIGHT_DARK,
        (ThemeName::TokyoNight, Appearance::Light) => &TOKYO_NIGHT_LIGHT,
        (ThemeName::Ayu, Appearance::Dark) => &AYU_DARK,
        (ThemeName::Ayu, Appearance::Light) => &AYU_LIGHT,
        (ThemeName::Monokai, Appearance::Dark) => &MONOKAI_DARK,
        (ThemeName::Monokai, Appearance::Light) => &MONOKAI_LIGHT,
    }
}

const QUINJET_DARK: [u32; 16] = [
    0x0d1117, 0x11161d, 0x161b22, 0x303842, 0x7e8b99, 0xdae0e8, 0xe6edf3, 0xffffff, 0xf85149,
    0xdb6d28, 0xd29922, 0x3fb950, 0x56d4dd, 0x58a6ff, 0xbc8cff, 0xab7df8,
];
const QUINJET_LIGHT: [u32; 16] = [
    0xffffff, 0xf6f8fa, 0xeaeef2, 0xd0d7de, 0x57606a, 0x24292f, 0x1f2328, 0x000000, 0xcf222e,
    0xbc4c00, 0x9a6700, 0x1a7f37, 0x1b7c83, 0x0969da, 0x8250df, 0xa40e26,
];
const CATPPUCCIN_DARK: [u32; 16] = [
    0x1e1e2e, 0x181825, 0x313244, 0x45475a, 0x9399b2, 0xcdd6f4, 0xf5e0dc, 0xbac2de, 0xf38ba8,
    0xfab387, 0xf9e2af, 0xa6e3a1, 0x94e2d5, 0x89b4fa, 0xcba6f7, 0xf2cdcd,
];
const CATPPUCCIN_LIGHT: [u32; 16] = [
    0xeff1f5, 0xe6e9ef, 0xdce0e8, 0x9ca0b0, 0x6c6f85, 0x4c4f69, 0x3c3f58, 0x303446, 0xd20f39,
    0xfe640b, 0xdf8e1d, 0x40a02b, 0x179299, 0x1e66f5, 0x8839ef, 0xdd7878,
];
const DRACULA_DARK: [u32; 16] = [
    0x282a36, 0x21222c, 0x343746, 0x6272a4, 0x9aa2c8, 0xf8f8f2, 0xffffff, 0xffffff, 0xff5555,
    0xffb86c, 0xf1fa8c, 0x50fa7b, 0x8be9fd, 0x66d9ef, 0xbd93f9, 0xff79c6,
];
const DRACULA_LIGHT: [u32; 16] = [
    0xf8f8f2, 0xefefe9, 0xe2e2dc, 0xbcbcbc, 0x686868, 0x282a36, 0x20212b, 0x191a21, 0xc41a16,
    0xa85d00, 0x8a7900, 0x14710a, 0x036a76, 0x005cc5, 0x6f42c1, 0xa90d91,
];
const EVERFOREST_DARK: [u32; 16] = [
    0x2d353b, 0x343f44, 0x3d484d, 0x56635f, 0x859289, 0xd3c6aa, 0xe4d9bd, 0xfdf6e3, 0xe67e80,
    0xe69875, 0xdbbc7f, 0xa7c080, 0x83c092, 0x7fbbb3, 0xd699b6, 0x9da9a0,
];
const EVERFOREST_LIGHT: [u32; 16] = [
    0xfdf6e3, 0xf4f0d9, 0xefebd4, 0xbdc3af, 0x829181, 0x5c6a72, 0x4b565c, 0x3a454a, 0xf85552,
    0xf57d26, 0xdfa000, 0x8da101, 0x35a77c, 0x3a94c5, 0xdf69ba, 0x8f5e15,
];
const GRUVBOX_DARK: [u32; 16] = [
    0x282828, 0x1d2021, 0x3c3836, 0x665c54, 0xa89984, 0xebdbb2, 0xfbf1c7, 0xf9f5d7, 0xfb4934,
    0xfe8019, 0xfabd2f, 0xb8bb26, 0x8ec07c, 0x83a598, 0xd3869b, 0xd65d0e,
];
const GRUVBOX_LIGHT: [u32; 16] = [
    0xfbf1c7, 0xebdbb2, 0xd5c4a1, 0xbdae93, 0x7c6f64, 0x3c3836, 0x282828, 0x1d2021, 0xcc241d,
    0xd65d0e, 0xd79921, 0x98971a, 0x689d6a, 0x458588, 0xb16286, 0x9d0006,
];
const NORD_DARK: [u32; 16] = [
    0x2e3440, 0x3b4252, 0x434c5e, 0x4c566a, 0x7b88a1, 0xd8dee9, 0xe5e9f0, 0xeceff4, 0xbf616a,
    0xd08770, 0xebcb8b, 0xa3be8c, 0x8fbcbb, 0x81a1c1, 0xb48ead, 0x5e81ac,
];
const NORD_LIGHT: [u32; 16] = [
    0xeceff4, 0xe5e9f0, 0xd8dee9, 0xb8c1d1, 0x66738c, 0x3b4252, 0x2e3440, 0x242933, 0xbf616a,
    0xc56a4a, 0xa17b16, 0x668a4c, 0x398e91, 0x426b94, 0x8b5e83, 0x345d8c,
];
const ONE_DARK: [u32; 16] = [
    0x282c34, 0x21252b, 0x2c313a, 0x4b5263, 0x7f848e, 0xabb2bf, 0xd7dae0, 0xffffff, 0xe06c75,
    0xd19a66, 0xe5c07b, 0x98c379, 0x56b6c2, 0x61afef, 0xc678dd, 0xbe5046,
];
const ONE_LIGHT: [u32; 16] = [
    0xfafafa, 0xf0f0f0, 0xe5e5e6, 0xa0a1a7, 0x696c77, 0x383a42, 0x202227, 0x121417, 0xe45649,
    0x986801, 0xc18401, 0x50a14f, 0x0184bc, 0x4078f2, 0xa626a4, 0xca1243,
];
const ROSE_PINE_DARK: [u32; 16] = [
    0x191724, 0x1f1d2e, 0x26233a, 0x555169, 0x6e6a86, 0xe0def4, 0xeeeaf4, 0xfaf4ed, 0xeb6f92,
    0xf6c177, 0xebbcba, 0x9ccfd8, 0x31748f, 0xc4a7e7, 0x9b8cbd, 0xe5a3a1,
];
const ROSE_PINE_LIGHT: [u32; 16] = [
    0xfaf4ed, 0xfff8f0, 0xf2e9e1, 0x9893a5, 0x797593, 0x575279, 0x403d52, 0x2b2838, 0xb4637a,
    0xea9d34, 0xd7827e, 0x56949f, 0x286983, 0x907aa9, 0x6e6a86, 0x9d6c74,
];
const SOLARIZED_DARK: [u32; 16] = [
    0x002b36, 0x073642, 0x094451, 0x586e75, 0x839496, 0xeee8d5, 0xfdf6e3, 0xffffff, 0xdc322f,
    0xcb4b16, 0xb58900, 0x859900, 0x2aa198, 0x268bd2, 0x6c71c4, 0xd33682,
];
const SOLARIZED_LIGHT: [u32; 16] = [
    0xfdf6e3, 0xeee8d5, 0xe4ddca, 0x93a1a1, 0x657b83, 0x586e75, 0x073642, 0x002b36, 0xdc322f,
    0xcb4b16, 0xb58900, 0x859900, 0x2aa198, 0x268bd2, 0x6c71c4, 0xd33682,
];
const TOKYO_NIGHT_DARK: [u32; 16] = [
    0x1a1b26, 0x16161e, 0x24283b, 0x414868, 0x737aa2, 0xc0caf5, 0xd5d6db, 0xffffff, 0xf7768e,
    0xff9e64, 0xe0af68, 0x9ece6a, 0x7dcfff, 0x7aa2f7, 0xbb9af7, 0xdb4b4b,
];
const TOKYO_NIGHT_LIGHT: [u32; 16] = [
    0xe1e2e7, 0xd5d6db, 0xc4c8da, 0x9699a3, 0x6172b0, 0x3760bf, 0x2e3c64, 0x1a1b26, 0xf52a65,
    0xb15c00, 0x8c6c3e, 0x587539, 0x007197, 0x2e7de9, 0x9854f1, 0x8c4351,
];
const AYU_DARK: [u32; 16] = [
    0x0b0e14, 0x11151c, 0x1f2430, 0x3b414d, 0x8a9199, 0xbfbdb6, 0xe6e1cf, 0xffffff, 0xf07178,
    0xff8f40, 0xffb454, 0xb8cc52, 0x95e6cb, 0x59c2ff, 0xd2a6ff, 0xe6b673,
];
const AYU_LIGHT: [u32; 16] = [
    0xfafafa, 0xf3f4f5, 0xe7e8e9, 0xabb0b6, 0x828c99, 0x5c6166, 0x3f454a, 0x1f2430, 0xf07171,
    0xfa8d3e, 0xf2ae49, 0x86b300, 0x4cbf99, 0x399ee6, 0xa37acc, 0xe6ba7e,
];
const MONOKAI_DARK: [u32; 16] = [
    0x272822, 0x1e1f1c, 0x383830, 0x5f6055, 0x8f908a, 0xf8f8f2, 0xf5f4f1, 0xffffff, 0xf92672,
    0xfd971f, 0xe6db74, 0xa6e22e, 0xa1efe4, 0x66d9ef, 0xae81ff, 0xcc6633,
];
const MONOKAI_LIGHT: [u32; 16] = [
    0xf9f8f5, 0xf0efe9, 0xe4e2da, 0xa7a59b, 0x737167, 0x3a3935, 0x272822, 0x171814, 0xd81e5b,
    0xc9670a, 0x8b7d00, 0x5c8f0b, 0x168c82, 0x007fa3, 0x7950c5, 0x9e4b1b,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_appearance_does_not_depend_on_the_system() {
        assert_eq!(AppearanceChoice::Light.resolve(), Appearance::Light);
        assert_eq!(AppearanceChoice::Dark.resolve(), Appearance::Dark);
    }

    #[test]
    fn unavailable_or_unspecified_system_appearance_falls_back_to_dark() {
        assert_eq!(system_appearance(None), Appearance::Dark);
        assert_eq!(
            system_appearance(Some(dark_light::Mode::Unspecified)),
            Appearance::Dark
        );
    }

    #[test]
    fn system_appearance_maps_light_and_dark() {
        assert_eq!(
            system_appearance(Some(dark_light::Mode::Light)),
            Appearance::Light
        );
        assert_eq!(
            system_appearance(Some(dark_light::Mode::Dark)),
            Appearance::Dark
        );
    }

    #[test]
    fn every_theme_has_distinct_light_and_dark_surfaces() {
        let names = [
            ThemeName::Quinjet,
            ThemeName::Catppuccin,
            ThemeName::Dracula,
            ThemeName::Everforest,
            ThemeName::Gruvbox,
            ThemeName::Nord,
            ThemeName::One,
            ThemeName::RosePine,
            ThemeName::Solarized,
            ThemeName::TokyoNight,
            ThemeName::Ayu,
            ThemeName::Monokai,
        ];
        for name in names {
            let light = Theme::new(name, Appearance::Light);
            let dark = Theme::new(name, Appearance::Dark);
            assert_ne!(light.background, dark.background, "{name:?}");
            assert_ne!(light.text, dark.text, "{name:?}");
            assert_ne!(light.selected, dark.selected, "{name:?}");
        }
    }

    #[test]
    fn every_theme_keeps_text_and_graphics_readable_on_every_surface() {
        let names = [
            ThemeName::Quinjet,
            ThemeName::Catppuccin,
            ThemeName::Dracula,
            ThemeName::Everforest,
            ThemeName::Gruvbox,
            ThemeName::Nord,
            ThemeName::One,
            ThemeName::RosePine,
            ThemeName::Solarized,
            ThemeName::TokyoNight,
            ThemeName::Ayu,
            ThemeName::Monokai,
        ];
        for name in names {
            for appearance in [Appearance::Light, Appearance::Dark] {
                let theme = Theme::new(name, appearance);
                let surfaces = [
                    theme.background,
                    theme.panel,
                    theme.panel_alt,
                    theme.accent_soft,
                    theme.selected,
                    theme.added_background,
                    theme.added_emphasis_background,
                    theme.removed_background,
                    theme.removed_emphasis_background,
                ];
                let foregrounds = [
                    theme.text,
                    theme.muted,
                    theme.accent,
                    theme.added,
                    theme.removed,
                    theme.modified,
                    theme.conflict,
                    theme.error,
                    theme.success,
                ];
                for foreground in foregrounds.into_iter().chain(theme.syntax) {
                    for background in surfaces {
                        assert!(
                            contrast(foreground, background) >= 4.5,
                            "{name:?} {appearance:?}: {foreground:?} on {background:?}"
                        );
                    }
                }
                for background in surfaces {
                    assert!(
                        contrast(theme.border, background) >= 3.0,
                        "{name:?} {appearance:?}: border on {background:?}"
                    );
                    assert!(
                        contrast(theme.border_focus, background) >= 3.0,
                        "{name:?} {appearance:?}: focus border on {background:?}"
                    );
                }
                assert!(
                    contrast(theme.selected, theme.background) >= 1.2,
                    "{name:?} {appearance:?}: selection against background"
                );
                assert!(
                    contrast(theme.selected, theme.panel) >= 1.2,
                    "{name:?} {appearance:?}: selection against panel"
                );
                assert!(
                    contrast(theme.selected, theme.panel_alt) >= 1.2,
                    "{name:?} {appearance:?}: selection against alternate panel"
                );
                assert!(
                    contrast(theme.text, theme.background)
                        >= contrast(theme.muted, theme.background),
                    "{name:?} {appearance:?}: muted text outranks primary text"
                );
                for semantic in [
                    theme.accent,
                    theme.added,
                    theme.removed,
                    theme.modified,
                    theme.conflict,
                ] {
                    assert!(
                        channel_span(semantic) >= 8,
                        "{name:?} {appearance:?}: semantic hue collapsed to {semantic:?}"
                    );
                }
            }
        }
    }

    fn channel_span(color: Color) -> u8 {
        let Color::Rgb(red, green, blue) = color else {
            return 0;
        };
        red.max(green).max(blue) - red.min(green).min(blue)
    }
}
