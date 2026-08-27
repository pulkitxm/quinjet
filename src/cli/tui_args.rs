use std::path::PathBuf;

use clap::{Args, ValueHint};

use crate::theme::{AppearanceChoice, HostTheme, ThemeName};

#[derive(Debug, Args)]
pub(super) struct TuiArgs {
    #[doc = " Git repository to open"]
    #[arg(default_value = ".", value_hint = ValueHint::DirPath)]
    pub(super) path: PathBuf,
    #[doc = " Disable mouse capture"]
    #[arg(long)]
    pub(super) no_mouse: bool,
    #[doc = " Listen for forwarded GitHub webhooks on a port or host:port"]
    #[arg(long, value_name = "ADDRESS")]
    pub(super) webhook_listen: Option<String>,
    #[doc = " Color palette to use throughout the interface"]
    #[arg(long, value_enum, default_value_t, conflicts_with = "theme_palette")]
    pub(super) theme: ThemeName,
    #[doc = " Host-provided light and dark color palettes as JSON"]
    #[arg(long, value_name = "JSON")]
    pub(super) theme_palette: Option<HostTheme>,
    #[doc = " Use the system, light, or dark variant of the palette"]
    #[arg(long, value_enum, default_value_t)]
    pub(super) appearance: AppearanceChoice,
    #[doc = " Open the interface focused on this pull request"]
    #[arg(long = "pr", value_name = "NUMBER")]
    pub(super) pull_request: Option<u64>,
}
