pub(crate) mod command;
mod completion;
mod package_manager;
mod pr_verbs;
mod remote;
mod render;
mod review;
mod session;
mod stack;
mod stack_verbs;
mod tui_args;
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
pub(crate) use command::{Command, Outcome};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pr_verbs::*;
use serde::Serialize;
pub(crate) use session::Session;
use stack::stack;
use stack_verbs::StackVerb;
use tui_args::TuiArgs;

use crate::git::diff::{DiffDocument, DiffIndex};
use crate::git::github::{
    CheckRunLog, GitHubRepository, MergeGate, PullRequest, PullRequestCheck,
    PullRequestCheckStatus, PullRequestCommentMode, PullRequestDiffIndex, PullRequestEdit,
    PullRequestLockReason, PullRequestMergeMethod, PullRequestMergeMode, PullRequestOperation,
    PullRequestReviewDecision, PullRequestReviewKind, PullRequestReviewOperation,
    PullRequestReviewSide, PullRequestReviewThreadSubject, PullRequestSnapshot, PullRequestStack,
    PullRequestStackSnapshot, PullRequestUpdateMethod, ReviewNextStep, ReviewProgress,
    ReviewSinceRequest,
};
use crate::git::status::{Change, ChangeArea};
use crate::git::{ConflictChoice, GitOperation, LocalDiffRequest, Repository};
use crate::integration::Client;
use crate::theme::{AppearanceChoice, ThemeName, ThemeSelection};

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
    pub theme: ThemeSelection,
    pub appearance: AppearanceChoice,
    pub pull_request: Option<u64>,
    pub client: Option<Client>,
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

    #[doc = " Repository to run a subcommand against"]
    #[arg(
        short = 'C',
        long = "path",
        visible_alias = "folder",
        value_name = "DIR",
        default_value = ".",
        global = true,
        value_hint = ValueHint::DirPath
    )]
    repository: PathBuf,

    #[doc = " Run Quinjet on an SSH machine"]
    #[arg(long, value_name = "SSH_TARGET", global = true, value_hint = ValueHint::Hostname)]
    remote: Option<String>,

    #[doc = " Reuse an existing SSH control socket"]
    #[arg(long, value_name = "PATH", global = true, value_hint = ValueHint::FilePath)]
    ssh_control_path: Option<PathBuf>,

    #[doc = " Print one JSON document on stdout instead of text"]
    #[arg(long, global = true)]
    json: bool,

    #[doc = " Open the terminal interface focused on this pull request"]
    #[arg(long = "pr", value_name = "NUMBER")]
    pull_request: Option<u64>,

    #[doc = " Delegate supported interface actions to an embedding client"]
    #[arg(long, value_enum, global = true)]
    client: Option<Client>,
}

#[derive(Debug, Subcommand)]
enum Verb {
    #[doc = " Open the terminal interface"]
    Tui(TuiArgs),
    #[doc = " Show the working tree, the index and the branch"]
    Status(WatchableArgs),
    #[doc = " Print the working-tree diff"]
    Diff(DiffArgs),
    #[doc = " Stage paths, or everything"]
    Stage(SelectionArgs),
    #[doc = " Unstage paths, or everything"]
    Unstage(SelectionArgs),
    #[doc = " Throw away changes to paths"]
    Discard(DiscardArgs),
    #[doc = " Delete paths from the working tree and the index"]
    #[command(visible_alias = "rm")]
    Remove(RemoveArgs),
    #[doc = " Record the staged changes"]
    Commit(CommitArgs),
    #[doc = " Fetch every remote and prune deleted refs"]
    Fetch,
    #[doc = " Pull the current branch"]
    Pull,
    #[doc = " Push the current branch"]
    Push,
    #[doc = " Pull, then push"]
    Sync,
    #[doc = " List commits"]
    Log(LogArgs),
    #[doc = " Show one commit and its patch"]
    Show(ShowArgs),
    #[doc = " Work with branches"]
    Branch {
        #[command(subcommand)]
        command: BranchVerb,
    },
    #[doc = " Work with stashes"]
    Stash {
        #[command(subcommand)]
        command: StashVerb,
    },
    #[doc = " List linked worktrees"]
    Worktree {
        #[command(subcommand)]
        command: WorktreeVerb,
    },
    #[doc = " Read recently opened projects"]
    Project {
        #[command(subcommand)]
        command: ProjectVerb,
    },
    #[doc = " Apply a commit onto the current branch"]
    CherryPick(RevisionArgs),
    #[doc = " Record a commit that undoes another"]
    Revert(RevisionArgs),
    #[doc = " Take one side of a merge conflict"]
    Resolve(ResolveArgs),
    #[doc = " List the GitHub repositories this checkout points at"]
    Repos(ReposArgs),
    #[doc = " Inspect recent SSH repositories"]
    Remote {
        #[command(subcommand)]
        command: RemoteVerb,
    },
    #[doc = " Read or update a pull request"]
    Pr {
        #[command(subcommand)]
        command: PrVerb,
    },
    #[doc = " Read or update stacked pull requests"]
    Stack {
        #[command(subcommand)]
        command: StackVerb,
    },
    #[doc = " Print or install shell completions"]
    #[command(visible_alias = "completion")]
    Completions(CompletionsArgs),
    #[doc = " Print the manual page, or write one page per command"]
    Man(ManArgs),
    #[doc = " Describe commands and arguments for automation"]
    Capabilities,
    #[doc = " Update this executable to the latest stable release"]
    Update(UpdateArgs),
}

impl Verb {
    const fn progress_label(&self) -> Option<&'static str> {
        match self {
            Self::Tui(_)
            | Self::Remote { .. }
            | Self::Project { .. }
            | Self::Completions(_)
            | Self::Man(_)
            | Self::Capabilities => None,
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
                command: PrVerb::Gate(args),
            } if args.watch => None,
            Self::Pr {
                command: PrVerb::Gate(_),
            } => Some("Evaluating the merge gate"),
            Self::Pr {
                command: PrVerb::Merge(_) | PrVerb::Close(_) | PrVerb::Reopen(_),
            } => Some("Updating pull request"),
            Self::Pr {
                command: PrVerb::Reviews { .. },
            } => Some("Updating pull-request review"),
            Self::Pr { .. } => Some("Loading pull request"),
            Self::Stack { .. } => Some("Loading pull-request stack"),
            Self::Update(_) => Some("Checking for updates"),
        }
    }
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    #[doc = " Shell to write or install a completion script for"]
    #[arg(value_enum, required_unless_present = "install")]
    shell: Option<Shell>,
    #[doc = " Install completions and a q launcher on PATH"]
    #[arg(long)]
    install: bool,
    #[arg(long, hide = true, requires = "install")]
    automatic: bool,
}

#[derive(Debug, Args)]
struct ManArgs {
    #[doc = " Write one page per command into this directory instead of printing one"]
    #[arg(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct UpdateArgs {
    #[doc = " Check for a newer release without installing it"]
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct WatchableArgs {
    #[doc = " Keep the reading on screen and refresh it"]
    #[arg(long)]
    watch: bool,
    #[doc = " Seconds between refreshes"]
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
    #[doc = " Limit the diff to these paths"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    paths: Vec<PathBuf>,
    #[doc = " Only what is staged"]
    #[arg(long)]
    staged: bool,
    #[doc = " Only what is not staged"]
    #[arg(long, conflicts_with = "staged")]
    unstaged: bool,
    #[doc = " Print whole files instead of three lines of context"]
    #[arg(long)]
    expanded: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct SelectionArgs {
    #[doc = " Paths to act on"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    paths: Vec<PathBuf>,
    #[doc = " Act on every change instead"]
    #[arg(long, conflicts_with = "paths")]
    all: bool,
}

#[derive(Debug, Args)]
struct DiscardArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    #[doc = " Confirm; without it the command reports what it would discard"]
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct RemoveArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    #[doc = " Confirm; without it the command reports what it would remove"]
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct CommitArgs {
    #[doc = " Commit message"]
    #[arg(short, long)]
    message: String,
    #[doc = " Replace the previous commit"]
    #[arg(long)]
    amend: bool,
}

#[derive(Debug, Args)]
struct LogArgs {
    #[doc = " Branch, tag, or commit to read from"]
    #[arg(default_value = "HEAD", value_name = "REVISION", value_hint = ValueHint::Other)]
    revision: String,
    #[doc = " Commits to skip"]
    #[arg(long, default_value_t = 0, value_hint = ValueHint::Other)]
    skip: usize,
    #[doc = " Commits to print"]
    #[arg(long, short = 'n', default_value_t = 30, value_hint = ValueHint::Other)]
    limit: usize,
}

#[derive(Debug, Args)]
struct ShowArgs {
    #[doc = " Commit to show"]
    #[arg(default_value = "HEAD", value_name = "REVISION", value_hint = ValueHint::Other)]
    revision: String,
    #[doc = " Print whole files instead of three lines of context"]
    #[arg(long)]
    expanded: bool,
}

#[derive(Debug, Args)]
struct RevisionArgs {
    #[doc = " Commit to act on"]
    #[arg(value_name = "REVISION", value_hint = ValueHint::Other)]
    revision: String,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct ResolveArgs {
    #[doc = " Conflicted path"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    path: PathBuf,
    #[command(flatten)]
    side: ConflictSide,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct ConflictSide {
    #[doc = " Keep the version already on this branch"]
    #[arg(long)]
    ours: bool,
    #[doc = " Keep the version being merged in"]
    #[arg(long)]
    theirs: bool,
    #[doc = " Accept the file as it stands and stage it"]
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
pub(crate) use remote::{begin_ssh_probe, local_ssh_context, run_selected_terminal};
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use repository::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use review::*;
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
