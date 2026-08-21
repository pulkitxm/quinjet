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
