#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " What a bundle is being assembled for, as the command line spells it."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(in crate::cli) enum PrContextPurpose {
    #[doc = " Reading the change"]
    Review,
    #[doc = " Answering reviewers"]
    AddressFeedback,
    #[doc = " Making the checks green"]
    FixCi,
}

impl PrContextPurpose {
    pub(in crate::cli) const fn purpose(self) -> ContextPurpose {
        match self {
            Self::Review => ContextPurpose::Review,
            Self::AddressFeedback => ContextPurpose::AddressFeedback,
            Self::FixCi => ContextPurpose::FixCi,
        }
    }
}

#[derive(Debug, Args)]
pub(in crate::cli) struct PrContextArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " What the bundle is for, which decides what it keeps"]
    #[arg(long, value_name = "PURPOSE", default_value = "review")]
    pub(in crate::cli) purpose: PrContextPurpose,
    #[doc = " How many characters the bundle may spend"]
    #[arg(long, value_name = "CHARACTERS", default_value_t = DEFAULT_CONTEXT_BUDGET)]
    pub(in crate::cli) budget: usize,
    #[doc = " Only this file's patch rather than the whole pull request"]
    #[arg(long = "file", value_name = "PATH", value_hint = ValueHint::FilePath)]
    pub(in crate::cli) path: Option<PathBuf>,
}
