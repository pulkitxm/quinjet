use super::*;
use crate::theme::{AppearanceChoice, ThemeName};

#[test]
fn terminal_themes_default_to_quinjet_with_system_appearance() {
    let cli = Cli::try_parse_from(["quinjet", "tui"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Verb::Tui(TuiArgs {
            theme: ThemeName::Quinjet,
            theme_palette: None,
            appearance: AppearanceChoice::System,
            ..
        }))
    ));

    let cli = Cli::try_parse_from([
        "quinjet",
        "tui",
        "--theme",
        "rose-pine",
        "--appearance",
        "light",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Some(Verb::Tui(TuiArgs {
            theme: ThemeName::RosePine,
            theme_palette: None,
            appearance: AppearanceChoice::Light,
            ..
        }))
    ));
    drop(Cli::try_parse_from(["quinjet", "tui", "--theme", "unknown"]).unwrap_err());
    drop(Cli::try_parse_from(["quinjet", "tui", "--appearance", "unknown"]).unwrap_err());
}

#[test]
fn terminal_accepts_a_host_theme_with_both_appearances() {
    let palette = r##"{"light":{"background":"#f7f3ec","panel":"#fffdf8","panelAlt":"#ece5d8","border":"#d6cbb8","muted":"#5c5247","text":"#241f1a","textStrong":"#100f0d","contrast":"#000000","removed":"#c93c37","orange":"#c46b32","modified":"#9a6700","added":"#2f7d42","cyan":"#1b7c83","accent":"#d97757","purple":"#8250df","brown":"#8f5e15"},"dark":{"background":"#1a1714","panel":"#221d19","panelAlt":"#2b2620","border":"#5f5549","muted":"#bcae9c","text":"#f1e9dc","textStrong":"#fffdf8","contrast":"#ffffff","removed":"#ff6961","orange":"#f0a35e","modified":"#e5c07b","added":"#78c091","cyan":"#70c5ce","accent":"#e08a6a","purple":"#c792ea","brown":"#d7a65c"}}"##;
    let cli = Cli::try_parse_from(["quinjet", "tui", "--theme-palette", palette]).unwrap();

    assert!(matches!(
        cli.command,
        Some(Verb::Tui(TuiArgs {
            theme_palette: Some(_),
            ..
        }))
    ));
    drop(
        Cli::try_parse_from([
            "quinjet",
            "tui",
            "--theme",
            "github",
            "--theme-palette",
            palette,
        ])
        .unwrap_err(),
    );
}
