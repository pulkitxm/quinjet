use super::actions::PrOptionalArgs;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag narrows the queue independently of the others"
)]
pub(in crate::cli) struct PrFeedbackArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Only what the merge is actually waiting on"]
    #[arg(long)]
    pub(in crate::cli) unresolved: bool,
    #[doc = " Only what is waiting on a reply from you"]
    #[arg(long)]
    pub(in crate::cli) mine: bool,
    #[doc = " Leave out the line-level findings a check reported"]
    #[arg(long)]
    pub(in crate::cli) no_checks: bool,
    #[doc = " Print each row's whole text rather than one line"]
    #[arg(long)]
    pub(in crate::cli) full: bool,
    #[doc = " Exit 1 when anything the merge is waiting on remains"]
    #[arg(long)]
    pub(in crate::cli) exit_code: bool,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(in crate::cli) struct PrSuggestionsCommand {
    #[command(subcommand)]
    pub(in crate::cli) command: Option<PrSuggestionVerb>,
    #[command(flatten)]
    pub(in crate::cli) list: PrOptionalArgs,
}

#[derive(Debug, Subcommand)]
pub(in crate::cli) enum PrSuggestionVerb {
    #[doc = " Apply suggested changes to the working tree"]
    Apply(PrSuggestionApplyArgs),
}

#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("suggestion-scope").required(true).multiple(false)))]
pub(in crate::cli) struct PrSuggestionApplyArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Review comment id, or a unique prefix of one"]
    #[arg(value_name = "SUGGESTION_ID", group = "suggestion-scope")]
    pub(in crate::cli) id: Option<String>,
    #[doc = " Apply every suggestion that can be applied"]
    #[arg(long, group = "suggestion-scope")]
    pub(in crate::cli) all: bool,
    #[doc = " Record the result as one commit with this message"]
    #[arg(long, value_name = "TEXT", value_hint = ValueHint::Other)]
    pub(in crate::cli) message: Option<String>,
    #[doc = " Confirm; without it the command reports what it would change"]
    #[arg(long)]
    pub(in crate::cli) yes: bool,
}

#[derive(Debug, Args)]
pub(in crate::cli) struct PrSuggestArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Repository-relative path to suggest a change to"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(in crate::cli) path: PathBuf,
    #[doc = " Last line the suggestion replaces"]
    #[arg(long)]
    pub(in crate::cli) line: usize,
    #[doc = " First line the suggestion replaces, for a multi-line suggestion"]
    #[arg(long)]
    pub(in crate::cli) start_line: Option<usize>,
    #[doc = " Note to print above the suggestion"]
    #[arg(long, value_name = "TEXT", default_value = "", value_hint = ValueHint::Other)]
    pub(in crate::cli) note: String,
    #[doc = " Replacement text, or the lines to replace them with"]
    #[command(flatten)]
    pub(in crate::cli) text: PrReviewBodyArgs,
}
