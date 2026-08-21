pub(crate) mod command;
mod completion;
mod package_manager;
mod render;
mod update;
mod watch;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::Shell;
use clap_mangen::Man;
pub(crate) use command::{Command, Outcome, Session};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use serde::Serialize;

use crate::git::diff::{DiffDocument, DiffIndex};
use crate::git::github::{
    CheckRunLog, GitHubRepository, PullRequest, PullRequestCheck, PullRequestCheckStatus,
    PullRequestCommentMode, PullRequestDiffIndex, PullRequestEdit, PullRequestLockReason,
    PullRequestMergeMethod, PullRequestMergeMode, PullRequestOperation, PullRequestReviewKind,
    PullRequestSnapshot, PullRequestUpdateMethod,
};
use crate::git::status::{Change, ChangeArea};
use crate::git::{ConflictChoice, GitOperation, LocalDiffRequest, Repository};
use crate::theme::{AppearanceChoice, ThemeName};

pub(crate) const EXIT_FAILURE: u8 = 1;
pub(crate) const EXIT_NOT_FOUND: u8 = 3;
pub(crate) const EXIT_UNAVAILABLE: u8 = 4;

const PROGRAM: &str = "quinjet";
const ROOT_HELP: &str = "Examples:
  quinjet
  quinjet status --json
  quinjet -C ../project diff --staged
  quinjet pr checks 42 --watch

Documentation: https://github.com/pulkitxm/quinjet/wiki/Command-Line";

const CHECK_WATCH_INTERVAL: u64 = 5;
const CHECK_WATCH_FLOOR: u64 = 2;
const LOG_WATCH_INTERVAL: u64 = 8;
const LOG_WATCH_FLOOR: u64 = 3;

#[derive(Debug)]
pub(crate) struct Failure {
    pub code: u8,
    pub message: String,
    pub hint: Option<String>,
}

impl Failure {
    pub(crate) fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
        }
    }

    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for Failure {}

pub(crate) struct TerminalOptions {
    pub path: PathBuf,
    pub no_mouse: bool,
    pub webhook_listen: Option<String>,
    pub theme: ThemeName,
    pub appearance: AppearanceChoice,
    pub pull_request: Option<u64>,
}

#[expect(
    variant_size_differences,
    reason = "the terminal options are already boxed; the only way to level this is to box one byte"
)]
pub(crate) enum Launch {
    Terminal(Box<TerminalOptions>),
    Finished(u8),
}

#[derive(Debug, Parser)]
#[command(name = "quinjet", version, about)]
#[command(subcommand_negates_reqs = true)]
#[command(propagate_version = true)]
#[command(after_long_help = ROOT_HELP)]
struct Cli {
    #[command(subcommand)]
    command: Option<Verb>,

    /// Repository to run a subcommand against
    #[arg(
        short = 'C',
        long = "path",
        value_name = "DIR",
        default_value = ".",
        global = true,
        value_hint = ValueHint::DirPath
    )]
    repository: PathBuf,

    /// Print one JSON document on stdout instead of text
    #[arg(long, global = true)]
    json: bool,

    /// Open the terminal interface focused on this pull request
    #[arg(long = "pr", value_name = "NUMBER")]
    pull_request: Option<u64>,
}

#[derive(Debug, Subcommand)]
enum Verb {
    /// Open the terminal interface
    Tui(TuiArgs),
    /// Show the working tree, the index and the branch
    Status(WatchableArgs),
    /// Print the working-tree diff
    Diff(DiffArgs),
    /// Stage paths, or everything
    Stage(SelectionArgs),
    /// Unstage paths, or everything
    Unstage(SelectionArgs),
    /// Throw away changes to paths
    Discard(DiscardArgs),
    /// Delete paths from the working tree and the index
    #[command(visible_alias = "rm")]
    Remove(RemoveArgs),
    /// Record the staged changes
    Commit(CommitArgs),
    /// Fetch every remote and prune deleted refs
    Fetch,
    /// Pull the current branch
    Pull,
    /// Push the current branch
    Push,
    /// Pull, then push
    Sync,
    /// List commits
    Log(LogArgs),
    /// Show one commit and its patch
    Show(ShowArgs),
    /// Work with branches
    Branch {
        #[command(subcommand)]
        command: BranchVerb,
    },
    /// Work with stashes
    Stash {
        #[command(subcommand)]
        command: StashVerb,
    },
    /// List linked worktrees
    Worktree {
        #[command(subcommand)]
        command: WorktreeVerb,
    },
    /// Apply a commit onto the current branch
    CherryPick(RevisionArgs),
    /// Record a commit that undoes another
    Revert(RevisionArgs),
    /// Take one side of a merge conflict
    Resolve(ResolveArgs),
    /// List the GitHub repositories this checkout points at
    Repos(ReposArgs),
    /// Read or update a pull request
    Pr {
        #[command(subcommand)]
        command: PrVerb,
    },
    /// Print or install shell completions
    #[command(visible_alias = "completion")]
    Completions(CompletionsArgs),
    /// Print the manual page, or write one page per command
    Man(ManArgs),
    /// Describe commands and arguments for automation
    Capabilities,
    /// Update this executable to the latest stable release
    Update(UpdateArgs),
}

impl Verb {
    const fn progress_label(&self) -> Option<&'static str> {
        match self {
            Self::Tui(_) | Self::Completions(_) | Self::Man(_) | Self::Capabilities => None,
            Self::Status(args) if args.watch => None,
            Self::Status(_) => Some("Reading repository status"),
            Self::Diff(_) => Some("Loading the working-tree diff"),
            Self::Stage(_) => Some("Staging changes"),
            Self::Unstage(_) => Some("Unstaging changes"),
            Self::Discard(_) => Some("Reading changes to discard"),
            Self::Remove(_) => Some("Reading files to remove"),
            Self::Commit(_) => Some("Creating commit"),
            Self::Fetch => Some("Fetching remotes"),
            Self::Pull => Some("Pulling changes"),
            Self::Push => Some("Pushing changes"),
            Self::Sync => Some("Synchronizing changes"),
            Self::Log(_) => Some("Reading commit history"),
            Self::Show(_) => Some("Loading commit patch"),
            Self::Branch { .. } => Some("Reading branch state"),
            Self::Stash { .. } => Some("Reading stash state"),
            Self::Worktree { .. } => Some("Reading worktrees"),
            Self::CherryPick(_) => Some("Resolving commit to cherry-pick"),
            Self::Revert(_) => Some("Resolving commit to revert"),
            Self::Resolve(_) => Some("Resolving conflict"),
            Self::Repos(_) => Some("Discovering GitHub repositories"),
            Self::Pr {
                command: PrVerb::View(args) | PrVerb::Conversation(args),
            } if args.watch => None,
            Self::Pr {
                command: PrVerb::Checks(args),
            } if args.watch => None,
            Self::Pr {
                command: PrVerb::Logs(args),
            } if args.watch => None,
            Self::Pr {
                command: PrVerb::Merge(_) | PrVerb::Close(_) | PrVerb::Reopen(_),
            } => Some("Updating pull request"),
            Self::Pr { .. } => Some("Loading pull request"),
            Self::Update(_) => Some("Checking for updates"),
        }
    }
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    /// Shell to write or install a completion script for
    #[arg(value_enum, required_unless_present = "install")]
    shell: Option<Shell>,
    /// Install completions and a q launcher on PATH
    #[arg(long)]
    install: bool,
    #[arg(long, hide = true, requires = "install")]
    automatic: bool,
}

#[derive(Debug, Args)]
struct ManArgs {
    /// Write one page per command into this directory instead of printing one
    #[arg(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct UpdateArgs {
    /// Check for a newer release without installing it
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct TuiArgs {
    /// Git repository to open
    #[arg(default_value = ".", value_hint = ValueHint::DirPath)]
    path: PathBuf,
    /// Disable mouse capture
    #[arg(long)]
    no_mouse: bool,
    /// Listen for forwarded GitHub webhooks on a port or host:port
    #[arg(long, value_name = "ADDRESS")]
    webhook_listen: Option<String>,
    /// Color palette to use throughout the interface
    #[arg(long, value_enum, default_value_t)]
    theme: ThemeName,
    /// Use the system, light, or dark variant of the palette
    #[arg(long, value_enum, default_value_t)]
    appearance: AppearanceChoice,
    /// Open the interface focused on this pull request
    #[arg(long = "pr", value_name = "NUMBER")]
    pull_request: Option<u64>,
}

#[derive(Debug, Args)]
struct WatchableArgs {
    /// Keep the reading on screen and refresh it
    #[arg(long)]
    watch: bool,
    /// Seconds between refreshes
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = 2,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(1..),
        value_hint = ValueHint::Other
    )]
    interval: u64,
}

#[derive(Debug, Args)]
struct DiffArgs {
    /// Limit the diff to these paths
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    paths: Vec<PathBuf>,
    /// Only what is staged
    #[arg(long)]
    staged: bool,
    /// Only what is not staged
    #[arg(long, conflicts_with = "staged")]
    unstaged: bool,
    /// Print whole files instead of three lines of context
    #[arg(long)]
    expanded: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct SelectionArgs {
    /// Paths to act on
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    paths: Vec<PathBuf>,
    /// Act on every change instead
    #[arg(long, conflicts_with = "paths")]
    all: bool,
}

#[derive(Debug, Args)]
struct DiscardArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Confirm; without it the command reports what it would discard
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct RemoveArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Confirm; without it the command reports what it would remove
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct CommitArgs {
    /// Commit message
    #[arg(short, long)]
    message: String,
    /// Replace the previous commit
    #[arg(long)]
    amend: bool,
}

#[derive(Debug, Args)]
struct LogArgs {
    /// Branch, tag, or commit to read from
    #[arg(default_value = "HEAD", value_name = "REVISION", value_hint = ValueHint::Other)]
    revision: String,
    /// Commits to skip
    #[arg(long, default_value_t = 0, value_hint = ValueHint::Other)]
    skip: usize,
    /// Commits to print
    #[arg(long, short = 'n', default_value_t = 30, value_hint = ValueHint::Other)]
    limit: usize,
}

#[derive(Debug, Args)]
struct ShowArgs {
    /// Commit to show
    #[arg(default_value = "HEAD", value_name = "REVISION", value_hint = ValueHint::Other)]
    revision: String,
    /// Print whole files instead of three lines of context
    #[arg(long)]
    expanded: bool,
}

#[derive(Debug, Args)]
struct RevisionArgs {
    /// Commit to act on
    #[arg(value_name = "REVISION", value_hint = ValueHint::Other)]
    revision: String,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct ResolveArgs {
    /// Conflicted path
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    path: PathBuf,
    #[command(flatten)]
    side: ConflictSide,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct ConflictSide {
    /// Keep the version already on this branch
    #[arg(long)]
    ours: bool,
    /// Keep the version being merged in
    #[arg(long)]
    theirs: bool,
    /// Accept the file as it stands and stage it
    #[arg(long)]
    stage: bool,
}

mod entry;
mod output;
mod pull_request;
mod repository;
pub(crate) mod support;
mod verbs;

pub(crate) use entry::dispatch;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use output::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pull_request::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use repository::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use support::*;
pub(crate) use support::{open_url, report, stdout_is_terminal};
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use verbs::*;

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests;
