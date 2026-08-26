#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Subcommand)]
pub(super) enum StackVerb {
    #[doc = " Show the stack containing a pull request"]
    View(PrArgs),
    #[doc = " List files changed by a composed stack range"]
    Files(StackRangeArgs),
    #[doc = " Print the patch for a composed stack range"]
    Diff(StackDiffArgs),
}

#[derive(Debug, Args)]
pub(super) struct StackRangeArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Bottom stack position to include"]
    #[arg(long, value_name = "POSITION", value_hint = ValueHint::Other)]
    pub(super) from: Option<usize>,
    #[doc = " Top stack position to include"]
    #[arg(long, value_name = "POSITION", value_hint = ValueHint::Other)]
    pub(super) to: Option<usize>,
}

#[derive(Debug, Args)]
pub(super) struct StackDiffArgs {
    #[command(flatten)]
    pub(super) range: StackRangeArgs,
    #[doc = " Limit the patch to one path"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(super) path: Option<PathBuf>,
}
