use std::path::PathBuf;

use clap::{Args, Subcommand};

use super::{PrArgs, PrMergeMethodArgs};
use crate::git::{StackModifyAction, StackOperation, StackRebaseAction};

#[derive(Debug, Subcommand)]
pub(super) enum StackVerb {
    #[doc = "Print the ordered pull requests in a stack"]
    View(PrArgs),
    #[doc = "List files across a contiguous stack range"]
    Files(StackRangeArgs),
    #[doc = "Print the patch across a contiguous stack range"]
    Diff(StackDiffArgs),
    #[doc = "Say which stack members can merge, and what blocks the rest"]
    Gate(StackGateArgs),
    #[doc = "Initialize a local branch stack"]
    Init(StackInitArgs),
    #[doc = "Add a branch to the active stack"]
    Add(StackAddArgs),
    #[doc = "Check out a stack"]
    Checkout(StackCheckoutArgs),
    #[doc = "Abort or continue a stack modification"]
    Modify(StackModifyArgs),
    #[doc = "Remove local or GitHub stack tracking"]
    Unstack(StackUnstackArgs),
    #[doc = "Link branches or pull requests into a GitHub stack"]
    Link(StackLinkArgs),
    #[doc = "Atomically merge a stack"]
    Merge(StackMergeArgs),
    #[doc = "Push active branches in the current stack"]
    Push(StackRemoteArgs),
    #[doc = "Cascade-rebase branches in the current stack"]
    Rebase(StackRebaseArgs),
    #[doc = "Push branches and create or update stacked pull requests"]
    Submit(StackSubmitArgs),
    #[doc = "Fetch, rebase, push, and synchronize the current stack"]
    Sync(StackSyncArgs),
    #[doc = "Check out the bottom branch of the current stack"]
    Bottom(StackConfirmArgs),
    #[doc = "Move toward the trunk in the current stack"]
    Down(StackStepArgs),
    #[doc = "Check out the top branch of the current stack"]
    Top(StackConfirmArgs),
    #[doc = "Check out the trunk of the current stack"]
    Trunk(StackConfirmArgs),
    #[doc = "Move toward the top of the current stack"]
    Up(StackStepArgs),
}

impl StackVerb {
    pub(super) fn into_operation(self) -> Option<(StackOperation, bool)> {
        match self {
            Self::View(_) | Self::Files(_) | Self::Diff(_) | Self::Gate(_) => None,
            Self::Init(args) => Some((
                StackOperation::Init {
                    branches: args.branches,
                    base: args.base,
                },
                args.yes,
            )),
            Self::Add(args) => Some((
                StackOperation::Add {
                    branch: args.branch,
                    all: args.all,
                    update: args.update,
                    message: args.message,
                },
                args.yes,
            )),
            Self::Checkout(args) => Some((StackOperation::Checkout(args.target), args.yes)),
            Self::Modify(args) => Some((
                StackOperation::Modify(if args.abort {
                    StackModifyAction::Abort
                } else {
                    StackModifyAction::Continue
                }),
                args.yes,
            )),
            Self::Unstack(args) => Some((
                StackOperation::Unstack {
                    stack: args.stack.map(|stack| stack.to_string()),
                    local: args.local,
                },
                args.yes,
            )),
            Self::Link(args) => Some((
                StackOperation::Link {
                    members: args.members,
                    base: args.base,
                    open: args.open,
                    remote: args.remote,
                },
                args.yes,
            )),
            Self::Merge(args) => Some((
                StackOperation::Merge {
                    target: args.target.map(|target| target.to_string()),
                    method: args.method.method(),
                },
                args.yes,
            )),
            Self::Push(args) => Some((
                StackOperation::Push {
                    remote: args.remote,
                },
                args.yes,
            )),
            Self::Rebase(args) => {
                let yes = args.yes;
                Some((StackOperation::Rebase(args.action()), yes))
            }
            Self::Submit(args) => Some((
                StackOperation::Submit {
                    open: args.open,
                    remote: args.remote,
                },
                args.yes,
            )),
            Self::Sync(args) => Some((
                StackOperation::Sync {
                    prune: args.prune,
                    remote: args.remote,
                },
                args.yes,
            )),
            Self::Bottom(args) => Some((StackOperation::Bottom, args.yes)),
            Self::Down(args) => Some((StackOperation::Down(args.steps), args.yes)),
            Self::Top(args) => Some((StackOperation::Top, args.yes)),
            Self::Trunk(args) => Some((StackOperation::Trunk, args.yes)),
            Self::Up(args) => Some((StackOperation::Up(args.steps), args.yes)),
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct StackRangeArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = "First one-based stack position to include"]
    #[arg(long, value_name = "POSITION")]
    pub(super) from: Option<usize>,
    #[doc = "Last one-based stack position to include"]
    #[arg(long, value_name = "POSITION")]
    pub(super) to: Option<usize>,
}

#[derive(Debug, Args)]
pub(super) struct StackDiffArgs {
    #[command(flatten)]
    pub(super) range: StackRangeArgs,
    #[doc = "Show only one changed path"]
    #[arg(value_name = "PATH")]
    pub(super) path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(super) struct StackInitArgs {
    #[doc = "Use this trunk branch instead of the repository default"]
    #[arg(long, short = 'b', value_name = "BRANCH", value_parser = non_empty)]
    pub(super) base: Option<String>,
    #[doc = "Branches to adopt or create, ordered from bottom to top"]
    #[arg(value_name = "BRANCH", num_args = 1.., required = true, value_parser = non_empty)]
    pub(super) branches: Vec<String>,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackAddArgs {
    #[doc = "Branch to create on top of the active stack"]
    #[arg(value_name = "BRANCH", value_parser = non_empty)]
    pub(super) branch: String,
    #[doc = "Stage tracked and untracked changes before committing"]
    #[arg(long, short = 'A', conflicts_with = "update", requires = "message")]
    pub(super) all: bool,
    #[doc = "Stage tracked changes before committing"]
    #[arg(long, short = 'u', conflicts_with = "all", requires = "message")]
    pub(super) update: bool,
    #[doc = "Commit staged changes with this message"]
    #[arg(long, short = 'm', value_name = "MESSAGE", value_parser = non_empty)]
    pub(super) message: Option<String>,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackCheckoutArgs {
    #[doc = "Stack number, pull request, URL, or locally tracked branch"]
    #[arg(value_name = "STACK|PR|URL|BRANCH", value_parser = non_empty)]
    pub(super) target: String,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackModifyArgs {
    #[doc = "Abort the active modification and restore the stack"]
    #[arg(
        long,
        conflicts_with = "continue",
        required_unless_present = "continue"
    )]
    pub(super) abort: bool,
    #[doc = "Continue the active modification after resolving conflicts"]
    #[arg(long, conflicts_with = "abort", required_unless_present = "abort")]
    pub(super) r#continue: bool,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackUnstackArgs {
    #[doc = "Stack number; defaults to the active stack"]
    #[arg(value_name = "STACK", value_parser = clap::value_parser!(u64).range(1..))]
    pub(super) stack: Option<u64>,
    #[doc = "Remove only local tracking and keep the GitHub stack"]
    #[arg(long)]
    pub(super) local: bool,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackLinkArgs {
    #[doc = "Stack number, branches, pull requests, or URLs in stack order"]
    #[arg(
        value_name = "STACK|BRANCH|PR",
        num_args = 2..,
        required = true,
        value_parser = non_empty
    )]
    pub(super) members: Vec<String>,
    #[doc = "Base branch for the bottom pull request"]
    #[arg(long, value_name = "BRANCH", value_parser = non_empty)]
    pub(super) base: Option<String>,
    #[doc = "Mark linked pull requests ready for review"]
    #[arg(long)]
    pub(super) open: bool,
    #[doc = "Git remote used to push branches"]
    #[arg(id = "git_remote", long = "git-remote", value_name = "REMOTE", value_parser = non_empty)]
    pub(super) remote: Option<String>,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackMergeArgs {
    #[doc = "Stack or pull request number; defaults to the active stack"]
    #[arg(value_name = "STACK|PR", value_parser = clap::value_parser!(u64).range(1..))]
    pub(super) target: Option<u64>,
    #[command(flatten)]
    pub(super) method: PrMergeMethodArgs,
    #[doc = "Confirm the atomic merge"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackRemoteArgs {
    #[doc = "Git remote used to push branches"]
    #[arg(id = "git_remote", long = "git-remote", value_name = "REMOTE", value_parser = non_empty)]
    pub(super) remote: Option<String>,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "these booleans model independent command-line flags"
)]
pub(super) struct StackRebaseArgs {
    #[doc = "Branch that bounds a directional rebase"]
    #[arg(value_name = "BRANCH", conflicts_with_all = ["abort", "continue"], value_parser = non_empty)]
    pub(super) branch: Option<String>,
    #[doc = "Abort the active rebase and restore every branch"]
    #[arg(long, conflicts_with_all = ["continue", "branch", "downstack", "upstack", "no_trunk", "preserve_dates", "git_remote"])]
    pub(super) abort: bool,
    #[doc = "Continue the active rebase after resolving conflicts"]
    #[arg(long, conflicts_with_all = ["abort", "branch", "downstack", "upstack", "no_trunk", "preserve_dates", "git_remote"])]
    pub(super) r#continue: bool,
    #[doc = "Rebase from the trunk through the selected branch"]
    #[arg(long, conflicts_with = "upstack")]
    pub(super) downstack: bool,
    #[doc = "Rebase from the selected branch through the stack top"]
    #[arg(long, conflicts_with = "downstack")]
    pub(super) upstack: bool,
    #[doc = "Rebase stack branches without updating the trunk"]
    #[arg(long)]
    pub(super) no_trunk: bool,
    #[doc = "Preserve author dates as committer dates"]
    #[arg(
        long = "committer-date-is-author-date",
        visible_alias = "preserve-dates"
    )]
    pub(super) preserve_dates: bool,
    #[doc = "Git remote used to fetch branches"]
    #[arg(id = "git_remote", long = "git-remote", value_name = "REMOTE", value_parser = non_empty)]
    pub(super) remote: Option<String>,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

impl StackRebaseArgs {
    fn action(self) -> StackRebaseAction {
        if self.abort {
            StackRebaseAction::Abort
        } else if self.r#continue {
            StackRebaseAction::Continue
        } else {
            StackRebaseAction::Start {
                branch: self.branch,
                downstack: self.downstack,
                upstack: self.upstack,
                no_trunk: self.no_trunk,
                preserve_dates: self.preserve_dates,
                remote: self.remote,
            }
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct StackSubmitArgs {
    #[doc = "Mark new and existing pull requests ready for review"]
    #[arg(long)]
    pub(super) open: bool,
    #[doc = "Git remote used to push branches"]
    #[arg(id = "git_remote", long = "git-remote", value_name = "REMOTE", value_parser = non_empty)]
    pub(super) remote: Option<String>,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackSyncArgs {
    #[doc = "Delete local branches for merged pull requests"]
    #[arg(long)]
    pub(super) prune: bool,
    #[doc = "Git remote used to fetch and push branches"]
    #[arg(id = "git_remote", long = "git-remote", value_name = "REMOTE", value_parser = non_empty)]
    pub(super) remote: Option<String>,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackConfirmArgs {
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct StackStepArgs {
    #[doc = "Number of branches to move"]
    #[arg(default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
    pub(super) steps: u64,
    #[doc = "Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(super) yes: bool,
}

fn non_empty(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err("value cannot be empty".to_owned())
    } else {
        Ok(value.to_owned())
    }
}

#[derive(Debug, Args)]
pub(super) struct StackGateArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = "Exit 0 whatever the verdict is"]
    #[arg(long)]
    pub(super) no_exit_code: bool,
}
