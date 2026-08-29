#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Subcommand)]
pub(super) enum BranchVerb {
    #[doc = " List local branches"]
    List(BranchListArgs),
    #[doc = " Switch to a branch"]
    Switch {
        #[doc = " Branch to switch to"]
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        name: String,
    },
    #[doc = " Create a branch and switch to it"]
    Create {
        #[doc = " New branch name"]
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        name: String,
        #[doc = " Commit to branch from"]
        #[arg(value_name = "START", value_hint = ValueHint::Other)]
        start: Option<String>,
    },
    #[doc = " Rename a branch"]
    Rename {
        #[doc = " Existing branch name"]
        #[arg(value_name = "OLD", value_hint = ValueHint::Other)]
        old: String,
        #[doc = " New branch name"]
        #[arg(value_name = "NEW", value_hint = ValueHint::Other)]
        new: String,
    },
    #[doc = " Delete a branch"]
    Delete {
        #[doc = " Branch to delete"]
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        name: String,
        #[doc = " Confirm; without it the command reports what it would delete"]
        #[arg(long)]
        yes: bool,
    },
    #[doc = " Diff a branch against the current one without checking anything out"]
    Compare {
        #[doc = " Local or remote-tracking branch to compare"]
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        reference: String,
        #[doc = " Print whole files instead of three lines of context"]
        #[arg(long)]
        expanded: bool,
    },
}

#[derive(Debug, Args)]
pub(super) struct BranchListArgs {
    #[doc = " Include remote-tracking branches"]
    #[arg(long)]
    pub(super) all: bool,
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub(super) enum WorktreeVerb {
    #[doc = " List this repository's worktrees"]
    List,
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub(super) enum ProjectVerb {
    #[doc = " List recently opened projects and their worktrees"]
    List,
}

#[derive(Debug, Subcommand)]
pub(super) enum RemoteVerb {
    #[doc = " List recent SSH repositories and their reachability"]
    List,
    #[doc = " Forget recent SSH repositories"]
    Forget {
        #[doc = " SSH target to forget"]
        #[arg(value_name = "SSH_TARGET", value_hint = ValueHint::Hostname)]
        target: String,
        #[doc = " Forget only this remote folder"]
        #[arg(long = "only-folder", value_name = "DIR", value_hint = ValueHint::DirPath)]
        folder: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum StashVerb {
    #[doc = " List stashes"]
    List,
    #[doc = " Stash the current changes"]
    Push {
        #[doc = " Message to record"]
        #[arg(short, long, default_value = "")]
        message: String,
        #[doc = " Include untracked files"]
        #[arg(long)]
        include_untracked: bool,
        #[doc = " Stash only what is staged"]
        #[arg(long, conflicts_with = "include_untracked")]
        staged: bool,
        #[doc = " Limit the stash to these paths"]
        #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
        paths: Vec<PathBuf>,
    },
    #[doc = " Apply a stash and keep it"]
    Apply {
        #[doc = " Stash reference to apply"]
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: String,
    },
    #[doc = " Apply a stash and drop it"]
    Pop {
        #[doc = " Stash reference to apply and drop"]
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: Option<String>,
    },
    #[doc = " Drop a stash"]
    Drop {
        #[doc = " Stash reference to drop"]
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: String,
        #[doc = " Confirm; without it the command reports what it would drop"]
        #[arg(long)]
        yes: bool,
    },
    #[doc = " Drop every stash"]
    Clear {
        #[doc = " Confirm; without it the command reports what it would drop"]
        #[arg(long)]
        yes: bool,
    },
    #[doc = " Print a stash as a patch"]
    Show {
        #[doc = " Stash reference to print"]
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: String,
        #[doc = " Print whole files instead of three lines of context"]
        #[arg(long)]
        expanded: bool,
    },
}

#[derive(Debug, Args, Clone)]
pub(super) struct PrArgs {
    #[doc = " Pull-request number"]
    #[arg(value_name = "NUMBER", value_hint = ValueHint::Other)]
    pub(super) number: u64,
    #[doc = " Repository the number belongs to, as owner/name"]
    #[arg(long, value_name = "OWNER/NAME", value_hint = ValueHint::Other)]
    pub(super) repo: Option<String>,
    #[doc = " Ask GitHub again instead of answering from the cache"]
    #[arg(long)]
    pub(super) refresh: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrWatchArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Keep the reading on screen and refresh it"]
    #[arg(long)]
    pub(super) watch: bool,
    #[doc = " Seconds between refreshes"]
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = CHECK_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(CHECK_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    pub(super) interval: u64,
}

#[derive(Debug, Args)]
pub(super) struct PrOpenArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Open a matching check run instead of the pull request"]
    #[arg(long, value_name = "NAME", value_hint = ValueHint::Other)]
    pub(super) check: Option<String>,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrMergeMethodArgs {
    #[doc = " Create a merge commit"]
    #[arg(long)]
    pub(super) merge: bool,
    #[doc = " Squash commits into one and merge"]
    #[arg(long)]
    pub(super) squash: bool,
    #[doc = " Rebase commits onto the base branch and merge"]
    #[arg(long)]
    pub(super) rebase: bool,
}

impl PrMergeMethodArgs {
    pub(super) const fn method(&self) -> PullRequestMergeMethod {
        if self.merge {
            PullRequestMergeMethod::Merge
        } else if self.rebase {
            PullRequestMergeMethod::Rebase
        } else {
            PullRequestMergeMethod::Squash
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct PrMergeArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[command(flatten)]
    pub(super) method: PrMergeMethodArgs,
    #[doc = " Delete the head branch after merging"]
    #[arg(long)]
    pub(super) delete_branch: bool,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrMutateArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrTextMutateArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Text to submit"]
    #[arg(value_name = "BODY", value_hint = ValueHint::Other)]
    pub(super) body: String,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrReviewChoiceArgs {
    #[doc = " Approve the pull request"]
    #[arg(long)]
    pub(super) approve: bool,
    #[doc = " Submit a review without a verdict"]
    #[arg(long)]
    pub(super) comment: bool,
    #[doc = " Request changes before merging"]
    #[arg(long)]
    pub(super) request_changes: bool,
}

impl PrReviewChoiceArgs {
    pub(super) const fn kind(&self) -> PullRequestReviewKind {
        if self.approve {
            PullRequestReviewKind::Approve
        } else if self.request_changes {
            PullRequestReviewKind::RequestChanges
        } else {
            PullRequestReviewKind::Comment
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct PrReviewArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[command(flatten)]
    pub(super) choice: PrReviewChoiceArgs,
    #[doc = " Optional review body"]
    #[arg(long, value_name = "TEXT", value_hint = ValueHint::Other)]
    pub(super) body: Option<String>,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum PrEditFieldArg {
    Title,
    Body,
    Base,
    AddAssignee,
    RemoveAssignee,
    AddLabel,
    RemoveLabel,
    AddProject,
    RemoveProject,
    AddReviewer,
    RemoveReviewer,
    Milestone,
    RemoveMilestone,
}

#[derive(Debug, Args)]
pub(super) struct PrEditArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Metadata field or relationship to change"]
    #[arg(value_enum, value_name = "FIELD")]
    pub(super) field: PrEditFieldArg,
    #[doc = " New value, or a comma-separated list for relationship fields"]
    #[arg(value_name = "VALUE", value_hint = ValueHint::Other)]
    pub(super) value: Option<String>,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

impl PrEditArgs {
    pub(super) fn edit(&self) -> Result<PullRequestEdit> {
        if matches!(self.field, PrEditFieldArg::RemoveMilestone) {
            if self.value.is_some() {
                return Err(anyhow::anyhow!("remove-milestone does not take a value"));
            }
            return Ok(PullRequestEdit::RemoveMilestone);
        }
        let value = self
            .value
            .clone()
            .ok_or_else(|| anyhow::anyhow!("the selected edit field needs a value"))?;
        Ok(match self.field {
            PrEditFieldArg::Title => PullRequestEdit::Title(value),
            PrEditFieldArg::Body => PullRequestEdit::Body(value),
            PrEditFieldArg::Base => PullRequestEdit::Base(value),
            PrEditFieldArg::AddAssignee => PullRequestEdit::AddAssignee(value),
            PrEditFieldArg::RemoveAssignee => PullRequestEdit::RemoveAssignee(value),
            PrEditFieldArg::AddLabel => PullRequestEdit::AddLabel(value),
            PrEditFieldArg::RemoveLabel => PullRequestEdit::RemoveLabel(value),
            PrEditFieldArg::AddProject => PullRequestEdit::AddProject(value),
            PrEditFieldArg::RemoveProject => PullRequestEdit::RemoveProject(value),
            PrEditFieldArg::AddReviewer => PullRequestEdit::AddReviewer(value),
            PrEditFieldArg::RemoveReviewer => PullRequestEdit::RemoveReviewer(value),
            PrEditFieldArg::Milestone => PullRequestEdit::SetMilestone(value),
            PrEditFieldArg::RemoveMilestone => return Ok(PullRequestEdit::RemoveMilestone),
        })
    }
}

#[derive(Debug, Args)]
pub(super) struct PrUpdateBranchArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Rebase onto the base branch instead of merging it"]
    #[arg(long)]
    pub(super) rebase: bool,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum PrLockReasonArg {
    OffTopic,
    Resolved,
    Spam,
    TooHeated,
}

impl From<PrLockReasonArg> for PullRequestLockReason {
    fn from(value: PrLockReasonArg) -> Self {
        match value {
            PrLockReasonArg::OffTopic => Self::OffTopic,
            PrLockReasonArg::Resolved => Self::Resolved,
            PrLockReasonArg::Spam => Self::Spam,
            PrLockReasonArg::TooHeated => Self::TooHeated,
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct PrLockArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Why the conversation is being locked"]
    #[arg(long, value_enum)]
    pub(super) reason: Option<PrLockReasonArg>,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrRevertArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Title for the revert pull request"]
    #[arg(long, value_name = "TEXT", value_hint = ValueHint::Other)]
    pub(super) title: Option<String>,
    #[doc = " Description for the revert pull request"]
    #[arg(long, value_name = "TEXT", value_hint = ValueHint::Other)]
    pub(super) body: Option<String>,
    #[doc = " Create the revert pull request as a draft"]
    #[arg(long)]
    pub(super) draft: bool,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrDiffArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Limit the patch to one path"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(super) path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(super) struct PrChecksArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Keep reading until every check has settled"]
    #[arg(long)]
    pub(super) watch: bool,
    #[doc = " Seconds between reads while watching"]
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = CHECK_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(CHECK_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    pub(super) interval: u64,
    #[doc = " Exit 1 when a check has not passed"]
    #[arg(long, conflicts_with = "watch")]
    pub(super) exit_code: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrGateArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Keep reading until the verdict settles"]
    #[arg(long)]
    pub(super) watch: bool,
    #[doc = " Seconds between reads while watching"]
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = CHECK_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(CHECK_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    pub(super) interval: u64,
    #[doc = " Exit 0 whatever the verdict is"]
    #[arg(long)]
    pub(super) no_exit_code: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrLogsArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Check run to read, by name"]
    #[arg(value_name = "CHECK", value_hint = ValueHint::Other)]
    pub(super) check: String,
    #[doc = " Keep reading while the run is still going"]
    #[arg(long)]
    pub(super) watch: bool,
    #[doc = " Seconds between reads while watching"]
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = LOG_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(LOG_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    pub(super) interval: u64,
}
