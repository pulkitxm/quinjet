use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
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
    pub removed: Color,
    pub removed_background: Color,
    pub modified: Color,
    pub conflict: Color,
    pub error: Color,
    pub success: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(13, 17, 23),
            panel: Color::Rgb(17, 22, 29),
            panel_alt: Color::Rgb(22, 27, 34),
            border: Color::Rgb(48, 56, 66),
            border_focus: Color::Rgb(73, 156, 255),
            text: Color::Rgb(218, 224, 232),
            muted: Color::Rgb(126, 139, 153),
            accent: Color::Rgb(88, 166, 255),
            accent_soft: Color::Rgb(37, 80, 126),
            selected: Color::Rgb(32, 60, 92),
            added: Color::Rgb(126, 231, 135),
            added_background: Color::Rgb(21, 56, 38),
            removed: Color::Rgb(255, 123, 114),
            removed_background: Color::Rgb(70, 30, 34),
            modified: Color::Rgb(210, 153, 34),
            conflict: Color::Rgb(219, 109, 40),
            error: Color::Rgb(248, 81, 73),
            success: Color::Rgb(63, 185, 80),
        }
    }
}
