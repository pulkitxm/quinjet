use std::{fmt, str::FromStr};

use serde::Deserialize;

use super::Appearance;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostTheme {
    light: HostPalette,
    dark: HostPalette,
}

impl HostTheme {
    pub(super) const fn palette(self, appearance: Appearance) -> [u32; 16] {
        match appearance {
            Appearance::Light => self.light.values,
            Appearance::Dark => self.dark.values,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostPalette {
    values: [u32; 16],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostThemeInput {
    light: HostPaletteInput,
    dark: HostPaletteInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostPaletteInput {
    background: String,
    panel: String,
    panel_alt: String,
    border: String,
    muted: String,
    text: String,
    text_strong: String,
    contrast: String,
    removed: String,
    orange: String,
    modified: String,
    added: String,
    cyan: String,
    accent: String,
    purple: String,
    brown: String,
}

impl HostPaletteInput {
    fn parse(self) -> Result<HostPalette, String> {
        Ok(HostPalette {
            values: [
                parse_hex(&self.background)?,
                parse_hex(&self.panel)?,
                parse_hex(&self.panel_alt)?,
                parse_hex(&self.border)?,
                parse_hex(&self.muted)?,
                parse_hex(&self.text)?,
                parse_hex(&self.text_strong)?,
                parse_hex(&self.contrast)?,
                parse_hex(&self.removed)?,
                parse_hex(&self.orange)?,
                parse_hex(&self.modified)?,
                parse_hex(&self.added)?,
                parse_hex(&self.cyan)?,
                parse_hex(&self.accent)?,
                parse_hex(&self.purple)?,
                parse_hex(&self.brown)?,
            ],
        })
    }
}

impl FromStr for HostTheme {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let input: HostThemeInput =
            serde_json::from_str(value).map_err(|error| format!("invalid theme JSON: {error}"))?;
        Ok(Self {
            light: input.light.parse()?,
            dark: input.dark.parse()?,
        })
    }
}

impl fmt::Display for HostTheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("host theme")
    }
}

fn parse_hex(value: &str) -> Result<u32, String> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{value} is not a six-digit RGB color"));
    }
    u32::from_str_radix(value, 16).map_err(|error| format!("invalid RGB color {value}: {error}"))
}
