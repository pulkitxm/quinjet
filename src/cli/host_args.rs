use std::path::PathBuf;

use clap::{Args, ValueHint};
use clap_complete::Shell;

#[derive(Debug, Args)]
pub(super) struct CompletionsArgs {
    #[doc = " Shell to write or install a completion script for"]
    #[arg(value_enum, required_unless_present = "install")]
    pub(super) shell: Option<Shell>,
    #[doc = " Install completions and a q launcher on PATH"]
    #[arg(long)]
    pub(super) install: bool,
    #[arg(long, hide = true, requires = "install")]
    pub(super) automatic: bool,
}

#[derive(Debug, Args)]
pub(super) struct ManArgs {
    #[doc = " Write one page per command into this directory instead of printing one"]
    #[arg(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub(super) dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(super) struct UpdateArgs {
    #[doc = " Check for a newer release without installing it"]
    #[arg(long)]
    pub(super) check: bool,
}
