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
fn theme_and_appearance_catalogs_are_unique_and_defaulted() {
    assert_eq!(ThemeName::default(), ThemeName::Quinjet);
    assert_eq!(AppearanceChoice::default(), AppearanceChoice::System);
    let theme_labels = ThemeName::ALL.map(ThemeName::label);
    let appearance_labels = AppearanceChoice::ALL.map(AppearanceChoice::label);
    assert!(theme_labels.iter().all(|label| !label.is_empty()));
    assert!(appearance_labels.iter().all(|label| !label.is_empty()));
    assert_eq!(
        theme_labels
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        ThemeName::ALL.len()
    );
    assert_eq!(
        appearance_labels
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        AppearanceChoice::ALL.len()
    );
}

#[test]
fn every_syntax_role_selects_its_stable_array_slot() {
    let theme = Theme::default();
    for (index, role) in [
        SyntaxColor::Text,
        SyntaxColor::Comment,
        SyntaxColor::Red,
        SyntaxColor::Orange,
        SyntaxColor::Yellow,
        SyntaxColor::Green,
        SyntaxColor::Cyan,
        SyntaxColor::Blue,
        SyntaxColor::Purple,
        SyntaxColor::Brown,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(theme.syntax(role), theme.syntax[index]);
    }
}

#[test]
fn rgb_conversion_and_blending_cover_endpoints_and_midpoint() {
    assert_eq!(color(0x12_34_56), Color::Rgb(0x12, 0x34, 0x56));
    assert_eq!(color_value(Color::Rgb(0x12, 0x34, 0x56)), 0x12_34_56);
    assert_eq!(color_value(Color::Reset), 0);
    assert_eq!(blend(0xff_00_00, 0x00_00_ff, 0), color(0x00_00_ff));
    assert_eq!(blend(0xff_00_00, 0x00_00_ff, 100), color(0xff_00_00));
    assert_eq!(blend(0xff_00_00, 0x00_00_ff, 50), color(0x7f_00_7f));
}

#[test]
fn contrast_is_symmetric_and_matches_black_white_bounds() {
    let black = color(0x00_00_00);
    let white = color(0xff_ff_ff);
    assert!((contrast(black, white) - 21.0).abs() < f64::EPSILON);
    assert!((contrast(black, white) - contrast(white, black)).abs() < f64::EPSILON);
    assert!((contrast(black, black) - 1.0).abs() < f64::EPSILON);
    assert!(luminance(Color::Reset).abs() < f64::EPSILON);
    assert!(linear_channel(0).abs() < f64::EPSILON);
    assert!((linear_channel(255) - 1.0).abs() < f64::EPSILON);
}

#[test]
fn every_theme_has_distinct_light_and_dark_surfaces() {
    for name in ThemeName::ALL {
        let light = Theme::new(name, Appearance::Light);
        let dark = Theme::new(name, Appearance::Dark);
        assert_ne!(light.background, dark.background, "{name:?}");
        assert_ne!(light.text, dark.text, "{name:?}");
        assert_ne!(light.selected, dark.selected, "{name:?}");
    }
}

#[test]
fn github_dark_uses_the_official_black_surfaces() {
    let theme = Theme::new(ThemeName::Github, Appearance::Dark);

    assert_eq!(theme.background, color(0x0d1117));
    assert_eq!(theme.panel, color(0x010409));
    assert_eq!(theme.panel_alt, color(0x161b22));
}

#[test]
fn every_theme_keeps_text_and_graphics_readable_on_every_surface() {
    for name in ThemeName::ALL {
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
                contrast(theme.text, theme.background) >= contrast(theme.muted, theme.background),
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
