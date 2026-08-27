use std::time::Duration;

use clap::ValueEnum;

const SYSTEM_APPEARANCE_TIMEOUT: Duration = Duration::from_millis(250);

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

    pub(crate) fn resolve(self) -> Appearance {
        match self {
            Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
            Self::System => system_appearance(detect_system_appearance()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Appearance {
    Light,
    Dark,
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

pub(super) const fn system_appearance(mode: Option<dark_light::Mode>) -> Appearance {
    match mode {
        Some(dark_light::Mode::Light) => Appearance::Light,
        Some(dark_light::Mode::Dark | dark_light::Mode::Unspecified) | None => Appearance::Dark,
    }
}
