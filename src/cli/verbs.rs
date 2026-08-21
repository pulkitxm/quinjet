#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Args)]
pub(super) struct ReposArgs {
    /// Read the remotes again instead of answering from the cache
    #[arg(long)]
    pub(super) refresh: bool,
}

#[derive(Debug, Subcommand)]
pub(super) enum BranchVerb {
    /// List local branches
    List(BranchListArgs),
    /// Switch to a branch
    Switch {
        /// Branch to switch to
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        name: String,
    },
    /// Create a branch and switch to it
    Create {
        /// New branch name
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        name: String,
        /// Commit to branch from
        #[arg(value_name = "START", value_hint = ValueHint::Other)]
        start: Option<String>,
    },
    /// Rename a branch
    Rename {
        /// Existing branch name
        #[arg(value_name = "OLD", value_hint = ValueHint::Other)]
        old: String,
        /// New branch name
        #[arg(value_name = "NEW", value_hint = ValueHint::Other)]
        new: String,
    },
    /// Delete a branch
    Delete {
        /// Branch to delete
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        name: String,
        /// Confirm; without it the command reports what it would delete
        #[arg(long)]
        yes: bool,
    },
    /// Diff a branch against the current one without checking anything out
    Compare {
        /// Local or remote-tracking branch to compare
        #[arg(value_name = "BRANCH", value_hint = ValueHint::Other)]
        reference: String,
        /// Print whole files instead of three lines of context
        #[arg(long)]
        expanded: bool,
    },
}

#[derive(Debug, Args)]
pub(super) struct BranchListArgs {
    /// Include remote-tracking branches
    #[arg(long)]
    pub(super) all: bool,
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub(super) enum WorktreeVerb {
    /// List this repository's worktrees
    List,
}

#[derive(Debug, Subcommand)]
pub(super) enum StashVerb {
    /// List stashes
    List,
    /// Stash the current changes
    Push {
        /// Message to record
        #[arg(short, long, default_value = "")]
        message: String,
        /// Include untracked files
        #[arg(long)]
        include_untracked: bool,
        /// Stash only what is staged
        #[arg(long, conflicts_with = "include_untracked")]
        staged: bool,
        /// Limit the stash to these paths
        #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
        paths: Vec<PathBuf>,
    },
    /// Apply a stash and keep it
    Apply {
        /// Stash reference to apply
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: String,
    },
    /// Apply a stash and drop it
    Pop {
        /// Stash reference to apply and drop
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: Option<String>,
    },
    /// Drop a stash
    Drop {
        /// Stash reference to drop
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: String,
        /// Confirm; without it the command reports what it would drop
        #[arg(long)]
        yes: bool,
    },
    /// Drop every stash
    Clear {
        /// Confirm; without it the command reports what it would drop
        #[arg(long)]
        yes: bool,
    },
    /// Print a stash as a patch
    Show {
        /// Stash reference to print
        #[arg(value_name = "STASH", value_hint = ValueHint::Other)]
        reference: String,
        /// Print whole files instead of three lines of context
        #[arg(long)]
        expanded: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum PrVerb {
    /// Print a pull request's metadata and description
    View(PrWatchArgs),
    /// List the files a pull request changes
    Files(PrArgs),
    /// Print a pull request's patch
    Diff(PrDiffArgs),
    /// Print a pull request's timeline and review comments
    Conversation(PrWatchArgs),
    /// List a pull request's checks
    Checks(PrChecksArgs),
    /// Print one check run's steps and log
    Logs(PrLogsArgs),
    /// Open a pull request in a browser
    Open(PrOpenArgs),
    /// Merge a pull request
    Merge(PrMergeArgs),
    /// Merge despite branch protections
    AdminMerge(PrMergeArgs),
    /// Merge automatically after requirements pass
    AutoMerge(PrMergeArgs),
    /// Disable automatic merging
    DisableAutoMerge(PrMutateArgs),
    /// Remove a pull request from its merge queue
    Dequeue(PrMutateArgs),
    /// Mark a draft pull request ready for review
    Ready(PrMutateArgs),
    /// Convert an open pull request to a draft
    Draft(PrMutateArgs),
    /// Submit an approval, comment, or change request
    Review(PrReviewArgs),
    /// Add a conversation comment
    Comment(PrTextMutateArgs),
    /// Edit your latest conversation comment
    EditLastComment(PrTextMutateArgs),
    /// Delete your latest conversation comment
    DeleteLastComment(PrMutateArgs),
    /// Edit pull-request metadata
    Edit(PrEditArgs),
    /// Bring the head branch up to date with its base
    UpdateBranch(PrUpdateBranchArgs),
    /// Lock the conversation
    Lock(PrLockArgs),
    /// Unlock the conversation
    Unlock(PrMutateArgs),
    /// Subscribe to notifications
    Subscribe(PrMutateArgs),
    /// Unsubscribe from notifications
    Unsubscribe(PrMutateArgs),
    /// Allow maintainers to edit a fork's head branch
    AllowMaintainerEdits(PrMutateArgs),
    /// Prevent maintainers from editing a fork's head branch
    DisallowMaintainerEdits(PrMutateArgs),
    /// Create a pull request that reverts a merged pull request
    Revert(PrRevertArgs),
    /// Close a pull request
    Close(PrMutateArgs),
    /// Reopen a closed pull request
    Reopen(PrMutateArgs),
}

#[derive(Debug, Args, Clone)]
pub(super) struct PrArgs {
    /// Pull-request number
    #[arg(value_name = "NUMBER", value_hint = ValueHint::Other)]
    pub(super) number: u64,
    /// Repository the number belongs to, as owner/name
    #[arg(long, value_name = "OWNER/NAME", value_hint = ValueHint::Other)]
    pub(super) repo: Option<String>,
    /// Ask GitHub again instead of answering from the cache
    #[arg(long)]
    pub(super) refresh: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrWatchArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Keep the reading on screen and refresh it
    #[arg(long)]
    pub(super) watch: bool,
    /// Seconds between refreshes
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
    /// Open a matching check run instead of the pull request
    #[arg(long, value_name = "NAME", value_hint = ValueHint::Other)]
    pub(super) check: Option<String>,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrMergeMethodArgs {
    /// Create a merge commit
    #[arg(long)]
    pub(super) merge: bool,
    /// Squash commits into one and merge
    #[arg(long)]
    pub(super) squash: bool,
    /// Rebase commits onto the base branch and merge
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
    /// Delete the head branch after merging
    #[arg(long)]
    pub(super) delete_branch: bool,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrMutateArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrTextMutateArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Text to submit
    #[arg(value_name = "BODY", value_hint = ValueHint::Other)]
    pub(super) body: String,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrReviewChoiceArgs {
    /// Approve the pull request
    #[arg(long)]
    pub(super) approve: bool,
    /// Submit a review without a verdict
    #[arg(long)]
    pub(super) comment: bool,
    /// Request changes before merging
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
    /// Optional review body
    #[arg(long, value_name = "TEXT", value_hint = ValueHint::Other)]
    pub(super) body: Option<String>,
    /// Confirm; without it the command reports what it would do
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
    /// Metadata field or relationship to change
    #[arg(value_enum, value_name = "FIELD")]
    pub(super) field: PrEditFieldArg,
    /// New value, or a comma-separated list for relationship fields
    #[arg(value_name = "VALUE", value_hint = ValueHint::Other)]
    pub(super) value: Option<String>,
    /// Confirm; without it the command reports what it would do
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
    /// Rebase onto the base branch instead of merging it
    #[arg(long)]
    pub(super) rebase: bool,
    /// Confirm; without it the command reports what it would do
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
    /// Why the conversation is being locked
    #[arg(long, value_enum)]
    pub(super) reason: Option<PrLockReasonArg>,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrRevertArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Title for the revert pull request
    #[arg(long, value_name = "TEXT", value_hint = ValueHint::Other)]
    pub(super) title: Option<String>,
    /// Description for the revert pull request
    #[arg(long, value_name = "TEXT", value_hint = ValueHint::Other)]
    pub(super) body: Option<String>,
    /// Create the revert pull request as a draft
    #[arg(long)]
    pub(super) draft: bool,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrDiffArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Limit the patch to one path
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(super) path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(super) struct PrChecksArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Keep reading until every check has settled
    #[arg(long)]
    pub(super) watch: bool,
    /// Seconds between reads while watching
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = CHECK_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(CHECK_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    pub(super) interval: u64,
    /// Exit 1 when a check has not passed
    #[arg(long, conflicts_with = "watch")]
    pub(super) exit_code: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrLogsArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Check run to read, by name
    #[arg(value_name = "CHECK", value_hint = ValueHint::Other)]
    pub(super) check: String,
    /// Keep reading while the run is still going
    #[arg(long)]
    pub(super) watch: bool,
    /// Seconds between reads while watching
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
