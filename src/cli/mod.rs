pub(crate) mod command;
mod completion;
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
use clap::{Args, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use clap_mangen::Man;
pub(crate) use command::{Command, Outcome, Session};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use serde::Serialize;

use crate::git::diff::{DiffDocument, DiffIndex};
use crate::git::github::{
    CheckRunLog, GitHubRepository, PullRequest, PullRequestCheck, PullRequestCheckStatus,
    PullRequestDiffIndex, PullRequestMergeMethod, PullRequestOperation, PullRequestSnapshot,
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

#[derive(Debug, Args)]
struct ReposArgs {
    /// Read the remotes again instead of answering from the cache
    #[arg(long)]
    refresh: bool,
}

#[derive(Debug, Subcommand)]
enum BranchVerb {
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
struct BranchListArgs {
    /// Include remote-tracking branches
    #[arg(long)]
    all: bool,
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum WorktreeVerb {
    /// List this repository's worktrees
    List,
}

#[derive(Debug, Subcommand)]
enum StashVerb {
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
enum PrVerb {
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
struct PrArgs {
    /// Pull-request number
    #[arg(value_name = "NUMBER", value_hint = ValueHint::Other)]
    number: u64,
    /// Repository the number belongs to, as owner/name
    #[arg(long, value_name = "OWNER/NAME", value_hint = ValueHint::Other)]
    repo: Option<String>,
    /// Ask GitHub again instead of answering from the cache
    #[arg(long)]
    refresh: bool,
}

#[derive(Debug, Args)]
struct PrWatchArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Keep the reading on screen and refresh it
    #[arg(long)]
    watch: bool,
    /// Seconds between refreshes
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = CHECK_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(CHECK_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    interval: u64,
}

#[derive(Debug, Args)]
struct PrOpenArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Open a matching check run instead of the pull request
    #[arg(long, value_name = "NAME", value_hint = ValueHint::Other)]
    check: Option<String>,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct PrMergeMethodArgs {
    /// Create a merge commit
    #[arg(long)]
    merge: bool,
    /// Squash commits into one and merge
    #[arg(long)]
    squash: bool,
    /// Rebase commits onto the base branch and merge
    #[arg(long)]
    rebase: bool,
}

impl PrMergeMethodArgs {
    const fn method(&self) -> PullRequestMergeMethod {
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
struct PrMergeArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    #[command(flatten)]
    method: PrMergeMethodArgs,
    /// Delete the head branch after merging
    #[arg(long)]
    delete_branch: bool,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct PrMutateArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Confirm; without it the command reports what it would do
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct PrDiffArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Limit the patch to one path
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    path: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct PrChecksArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Keep reading until every check has settled
    #[arg(long)]
    watch: bool,
    /// Seconds between reads while watching
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = CHECK_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(CHECK_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    interval: u64,
    /// Exit 1 when a check has not passed
    #[arg(long, conflicts_with = "watch")]
    exit_code: bool,
}

#[derive(Debug, Args)]
struct PrLogsArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Check run to read, by name
    #[arg(value_name = "CHECK", value_hint = ValueHint::Other)]
    check: String,
    /// Keep reading while the run is still going
    #[arg(long)]
    watch: bool,
    /// Seconds between reads while watching
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = LOG_WATCH_INTERVAL,
        requires = "watch",
        value_parser = clap::value_parser!(u64).range(LOG_WATCH_FLOOR..),
        value_hint = ValueHint::Other
    )]
    interval: u64,
}

pub(crate) fn dispatch() -> Result<Launch> {
    completion::auto_install();
    let cli = Cli::parse();
    let mut out = Emitter::new(cli.json);
    let verb = match cli.command {
        None => {
            return Ok(Launch::Terminal(Box::new(TerminalOptions {
                path: PathBuf::from("."),
                no_mouse: false,
                webhook_listen: None,
                theme: ThemeName::default(),
                appearance: AppearanceChoice::default(),
            })));
        }
        Some(Verb::Tui(args)) => {
            return Ok(Launch::Terminal(Box::new(TerminalOptions {
                path: args.path,
                no_mouse: args.no_mouse,
                webhook_listen: args.webhook_listen,
                theme: args.theme,
                appearance: args.appearance,
            })));
        }
        Some(Verb::Completions(args)) => {
            return completions(&out, &args).map(Launch::Finished);
        }
        Some(Verb::Man(args)) => return manual(&out, &args).map(Launch::Finished),
        Some(Verb::Capabilities) => return capabilities(&out).map(Launch::Finished),
        Some(Verb::Update(args)) => {
            out.start_progress("Checking for updates")?;
            let result = update::run(&out, args.check);
            out.finish_progress();
            return result.map(Launch::Finished);
        }
        Some(other) => other,
    };
    if let Some(label) = verb.progress_label() {
        out.start_progress(label)?;
    }
    let result = (|| {
        let repository = Repository::discover(&cli.repository)?;
        let mut session = Session::new(repository);
        run(&mut session, &out, verb)
    })();
    out.finish_progress();
    result.map(Launch::Finished)
}

fn completions(out: &Emitter, args: &CompletionsArgs) -> Result<u8> {
    let shell = args
        .shell
        .or_else(completion::detected_shell)
        .context("could not detect a supported shell; name one explicitly")?;
    if args.install {
        let paths = if args.automatic {
            completion::maintain(shell)?
        } else {
            completion::install(shell)?
        };
        let paths: Vec<String> = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        return out
            .emit(
                &CompletionInstallation {
                    shell: shell.to_string(),
                    shortcut: "q",
                    paths: &paths,
                },
                || {
                    let mut text = format!("Installed {shell} shell integration\n");
                    for path in &paths {
                        text.push_str("  ");
                        text.push_str(path);
                        text.push('\n');
                    }
                    text
                },
            )
            .map(|()| 0);
    }
    let script = completion::script(shell)?;
    out.emit(
        &CompletionScript {
            shell: shell.to_string(),
            script: &script,
        },
        || script.clone(),
    )?;
    Ok(0)
}

fn manual(out: &Emitter, args: &ManArgs) -> Result<u8> {
    let mut command = Cli::command();
    command.build();
    let Some(directory) = args.dir.as_deref() else {
        let page = render_page(&command, PROGRAM)?;
        let text = String::from_utf8(page).context("the manual page was not valid UTF-8")?;
        return out
            .emit(&ManualPage { page: &text }, || text.clone())
            .map(|()| 0);
    };
    fs::create_dir_all(directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;
    let mut written = Vec::new();
    write_pages(&command, PROGRAM, directory, &mut written)?;
    out.emit(&ManualPages { pages: &written }, || {
        let mut text = format!("Wrote {} pages to {}\n", written.len(), directory.display());
        for page in &written {
            text.push_str("  ");
            text.push_str(page);
            text.push('\n');
        }
        text
    })?;
    Ok(0)
}

fn capabilities(out: &Emitter) -> Result<u8> {
    let mut command = Cli::command();
    command.build();
    let mut commands = Vec::new();
    collect_capabilities(&command, &[], &mut commands);
    let document = CapabilityDocument {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        output_modes: ["text", "json"],
        commands,
    };
    out.emit(&document, || render_capabilities(&document))?;
    Ok(0)
}

fn collect_capabilities(
    command: &clap::Command,
    parent: &[String],
    commands: &mut Vec<CommandCapability>,
) {
    let mut path = parent.to_vec();
    path.push(command.get_name().to_owned());
    let arguments = command
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .filter(|argument| argument.get_id() != "help" && argument.get_id() != "version")
        .map(|argument| {
            let (min_values, max_values) = argument.get_num_args().map_or((0, Some(0)), |range| {
                let maximum = range.max_values();
                (
                    range.min_values(),
                    (maximum != usize::MAX).then_some(maximum),
                )
            });
            ArgumentCapability {
                id: argument.get_id().to_string(),
                help: argument.get_help().map(ToString::to_string),
                short: argument.get_short(),
                long: argument.get_long().map(str::to_owned),
                positional: argument.is_positional(),
                required: argument.is_required_set(),
                action: argument_action(argument.get_action()),
                min_values,
                max_values,
                value_names: argument
                    .get_value_names()
                    .map(|names| names.iter().map(ToString::to_string).collect())
                    .unwrap_or_default(),
                possible_values: argument
                    .get_possible_values()
                    .iter()
                    .map(|value| value.get_name().to_owned())
                    .collect(),
                default_values: argument
                    .get_default_values()
                    .iter()
                    .map(|value| value.to_string_lossy().into_owned())
                    .collect(),
            }
        })
        .collect();
    let usage = command.clone().render_usage().to_string();
    let groups = command
        .get_groups()
        .map(|group| {
            let mut configured = group.clone();
            ArgumentGroupCapability {
                id: configured.get_id().to_string(),
                arguments: configured.get_args().map(ToString::to_string).collect(),
                required: configured.is_required_set(),
                multiple: configured.is_multiple(),
            }
        })
        .collect();
    commands.push(CommandCapability {
        path: path.join(" "),
        about: command.get_about().map(ToString::to_string),
        usage,
        arguments,
        groups,
        subcommands: command
            .get_subcommands()
            .filter(|child| child.get_name() != "help")
            .map(|child| child.get_name().to_owned())
            .collect(),
    });
    for child in command
        .get_subcommands()
        .filter(|child| child.get_name() != "help")
    {
        collect_capabilities(child, &path, commands);
    }
}

const fn argument_action(action: &clap::ArgAction) -> &'static str {
    match action {
        clap::ArgAction::Set => "set",
        clap::ArgAction::Append => "append",
        clap::ArgAction::SetTrue => "set_true",
        clap::ArgAction::SetFalse => "set_false",
        clap::ArgAction::Count => "count",
        clap::ArgAction::Help => "help",
        clap::ArgAction::HelpShort => "help_short",
        clap::ArgAction::HelpLong => "help_long",
        clap::ArgAction::Version => "version",
        _ => "other",
    }
}

fn render_capabilities(document: &CapabilityDocument) -> String {
    let mut text = format!(
        "Quinjet {} command capabilities (schema {})\n\n",
        document.version, document.schema_version
    );
    for command in &document.commands {
        text.push_str(&command.path);
        if let Some(about) = &command.about {
            text.push_str("  ");
            text.push_str(about);
        }
        text.push('\n');
    }
    text.push_str("\nUse --json for arguments, values, and command relationships.\n");
    text
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityDocument {
    schema_version: u8,
    version: &'static str,
    output_modes: [&'static str; 2],
    commands: Vec<CommandCapability>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandCapability {
    path: String,
    about: Option<String>,
    usage: String,
    arguments: Vec<ArgumentCapability>,
    groups: Vec<ArgumentGroupCapability>,
    subcommands: Vec<String>,
}

#[derive(Serialize)]
struct ArgumentGroupCapability {
    id: String,
    arguments: Vec<String>,
    required: bool,
    multiple: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArgumentCapability {
    id: String,
    help: Option<String>,
    short: Option<char>,
    long: Option<String>,
    positional: bool,
    required: bool,
    action: &'static str,
    min_values: usize,
    max_values: Option<usize>,
    value_names: Vec<String>,
    possible_values: Vec<String>,
    default_values: Vec<String>,
}

fn render_page(command: &clap::Command, name: &str) -> Result<Vec<u8>> {
    let mut page = Vec::new();
    Man::new(command.clone().display_name(name.to_owned()))
        .title(name.to_uppercase())
        .render(&mut page)
        .with_context(|| format!("failed to render the manual page for {name}"))?;
    Ok(page)
}

fn write_pages(
    command: &clap::Command,
    name: &str,
    directory: &Path,
    written: &mut Vec<String>,
) -> Result<()> {
    let file = directory.join(format!("{name}.1"));
    fs::write(&file, render_page(command, name)?)
        .with_context(|| format!("failed to write {}", file.display()))?;
    written.push(file.display().to_string());
    for child in command.get_subcommands() {
        write_pages(
            child,
            &format!("{name}-{}", child.get_name()),
            directory,
            written,
        )?;
    }
    Ok(())
}

#[derive(Serialize)]
struct CompletionScript<'a> {
    shell: String,
    script: &'a str,
}

#[derive(Serialize)]
struct CompletionInstallation<'a> {
    shell: String,
    shortcut: &'static str,
    paths: &'a [String],
}

#[derive(Serialize)]
struct ManualPage<'a> {
    page: &'a str,
}

#[derive(Serialize)]
struct ManualPages<'a> {
    pages: &'a [String],
}

struct Emitter {
    json: bool,
    progress: Option<ProgressBar>,
}

impl Emitter {
    const fn new(json: bool) -> Self {
        Self {
            json,
            progress: None,
        }
    }

    fn start_progress(&mut self, label: &'static str) -> Result<()> {
        if !progress_enabled(self.json, io::stderr().is_terminal()) {
            return Ok(());
        }
        let progress = progress_bar(label, ProgressDrawTarget::stderr())?;
        progress.enable_steady_tick(Duration::from_millis(100));
        self.progress = Some(progress);
        Ok(())
    }

    fn set_progress(&self, label: &'static str) {
        if let Some(progress) = &self.progress {
            progress.set_message(label);
        }
    }

    fn note(&self, text: &str) {
        if let Some(progress) = &self.progress {
            progress.println(text);
        } else {
            note(text);
        }
    }

    fn finish_progress(&self) {
        if let Some(progress) = &self.progress {
            progress.finish_and_clear();
        }
    }

    fn execute(&self, session: &mut Session, command: Command) -> Result<Outcome> {
        self.set_progress(command.progress_label());
        session.execute_with(
            command,
            &mut |event| self.set_progress(event.label()),
            &|| true,
        )
    }

    fn emit<T: Serialize>(&self, value: &T, text: impl FnOnce() -> String) -> Result<()> {
        self.finish_progress();
        let mut stdout = io::stdout().lock();
        if self.json {
            writeln!(stdout, "{}", serde_json::to_string_pretty(value)?)?;
        } else {
            write!(stdout, "{}", text())?;
        }
        stdout.flush()?;
        Ok(())
    }

    fn message(&self, message: &str) -> Result<()> {
        self.emit(&Message { message }, || format!("{message}\n"))
    }
}

const fn progress_enabled(json: bool, stderr_terminal: bool) -> bool {
    !json && stderr_terminal
}

fn progress_bar(label: &'static str, target: ProgressDrawTarget) -> Result<ProgressBar> {
    let progress = ProgressBar::with_draw_target(None, target);
    progress.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")?.tick_strings(&["-", "\\", "|", "/"]),
    );
    progress.set_message(label);
    Ok(progress)
}

#[derive(Serialize)]
struct Message<'a> {
    message: &'a str,
}

fn run(session: &mut Session, out: &Emitter, verb: Verb) -> Result<u8> {
    match verb {
        Verb::Tui(_) => Err(Failure::new(
            EXIT_FAILURE,
            "the terminal interface is launched before any verb runs",
        )
        .into()),
        Verb::Completions(_) | Verb::Man(_) | Verb::Capabilities | Verb::Update(_) => {
            Err(Failure::new(
                EXIT_FAILURE,
                "metadata commands run before a repository is opened",
            )
            .into())
        }
        Verb::Status(args) => status(session, out, &args),
        Verb::Diff(args) => working_diff(session, out, &args),
        Verb::Stage(args) => {
            let operation = if args.all {
                GitOperation::StageAll
            } else {
                GitOperation::Stage(require_paths(args.paths, "stage")?)
            };
            operate(session, out, operation)
        }
        Verb::Unstage(args) => {
            let operation = if args.all {
                GitOperation::UnstageAll
            } else {
                GitOperation::Unstage(require_paths(args.paths, "unstage")?)
            };
            operate(session, out, operation)
        }
        Verb::Discard(args) => discard(session, out, &args),
        Verb::Commit(args) => operate(
            session,
            out,
            GitOperation::Commit {
                message: args.message,
                amend: args.amend,
            },
        ),
        Verb::Fetch => operate(session, out, GitOperation::Fetch),
        Verb::Pull => operate(session, out, GitOperation::Pull),
        Verb::Push => operate(session, out, GitOperation::Push),
        Verb::Sync => operate(session, out, GitOperation::Sync),
        Verb::Log(args) => log(session, out, &args),
        Verb::Show(args) => show(session, out, &args),
        Verb::Branch { command } => branch(session, out, command),
        Verb::Stash { command } => stash(session, out, command),
        Verb::Worktree { command } => worktree(session, out, command),
        Verb::CherryPick(args) => {
            revision_operation(session, out, &args, "cherry-pick", GitOperation::CherryPick)
        }
        Verb::Revert(args) => {
            revision_operation(session, out, &args, "revert", GitOperation::Revert)
        }
        Verb::Resolve(args) => resolve(session, out, args),
        Verb::Repos(args) => repositories(session, out, &args),
        Verb::Pr { command } => pull_request(session, out, command),
    }
}

fn status(session: &mut Session, out: &Emitter, args: &WatchableArgs) -> Result<u8> {
    if args.watch {
        return watch::run(interval(args.interval, 1), out.json, || {
            let status = session.execute(Command::Status)?.status()?;
            Ok(watch::Frame {
                text: render::status(&status),
                value: status,
                finished: false,
                code: 0,
            })
        });
    }
    let status = session.execute(Command::Status)?.status()?;
    out.emit(&status, || render::status(&status))?;
    Ok(0)
}

fn working_diff(session: &mut Session, out: &Emitter, args: &DiffArgs) -> Result<u8> {
    let status = session.execute(Command::Status)?.status()?;
    let changes: Vec<Change> = status
        .changes
        .iter()
        .filter(|change| match (args.staged, args.unstaged) {
            (true, _) => change.area == ChangeArea::Staged,
            (_, true) => change.area == ChangeArea::Unstaged,
            _ => true,
        })
        .filter(|change| matches(&change.path, &args.paths))
        .cloned()
        .collect();
    if changes.is_empty() {
        out.message("No changes match")?;
        return Ok(0);
    }
    let document = whole_document(
        session,
        Command::PrepareLocalDiff {
            workspace: 0,
            request: Box::new(LocalDiffRequest::Changes {
                changes,
                version: 0,
                expanded: args.expanded,
            }),
        },
        |workspace, path| Command::LocalDiffFile { workspace, path },
    )?;
    out.emit(&document, || render::diff(&document))?;
    Ok(0)
}

fn log(session: &mut Session, out: &Emitter, args: &LogArgs) -> Result<u8> {
    let revision = revision(session, &args.revision)?;
    let commits = session
        .execute(Command::History {
            revision,
            skip: args.skip,
            limit: args.limit,
        })?
        .history()?;
    out.emit(&commits, || render::history(&commits))?;
    Ok(0)
}

fn show(session: &mut Session, out: &Emitter, args: &ShowArgs) -> Result<u8> {
    let revision = revision(session, &args.revision)?;
    let commits = session
        .execute(Command::History {
            revision: revision.clone(),
            skip: 0,
            limit: 1,
        })?
        .history()?;
    let Some(commit) = commits.into_iter().next() else {
        return Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("`{revision}` does not name a commit in this repository"),
        )
        .into());
    };
    let document = whole_document(
        session,
        Command::PrepareLocalDiff {
            workspace: 0,
            request: Box::new(LocalDiffRequest::Commit {
                commit: Box::new(commit.clone()),
                expanded: args.expanded,
            }),
        },
        |workspace, path| Command::LocalDiffFile { workspace, path },
    )?;
    out.emit(
        &CommitPatch {
            commit: &commit,
            diff: &document,
        },
        || format!("{}{}", render::commit(&commit), render::diff(&document)),
    )?;
    Ok(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitPatch<'a> {
    commit: &'a crate::git::history::Commit,
    diff: &'a DiffDocument,
}

fn branch(session: &mut Session, out: &Emitter, command: BranchVerb) -> Result<u8> {
    match command {
        BranchVerb::List(args) if args.all => {
            let branches = session
                .execute(Command::HistoryBranches)?
                .history_branches()?;
            out.emit(&branches, || render::history_branches(&branches))?;
            Ok(0)
        }
        BranchVerb::List(_) => {
            let branches = session.execute(Command::Branches)?.branches()?;
            out.emit(&branches, || render::branches(&branches))?;
            Ok(0)
        }
        BranchVerb::Switch { name } => operate(session, out, GitOperation::Checkout(name)),
        BranchVerb::Create { name, start } => {
            let start = match start {
                Some(start) => Some(revision(session, &start)?),
                None => None,
            };
            operate(session, out, GitOperation::CreateBranch { name, start })
        }
        BranchVerb::Rename { old, new } => {
            operate(session, out, GitOperation::RenameBranch { old, new })
        }
        BranchVerb::Delete { name, yes } => {
            if !yes {
                out.message(&format!("Would delete `{name}`. Pass --yes to delete it."))?;
                return Ok(0);
            }
            operate(session, out, GitOperation::DeleteBranch(name))
        }
        BranchVerb::Compare {
            reference,
            expanded,
        } => compare(session, out, &reference, expanded),
    }
}

fn compare(session: &mut Session, out: &Emitter, reference: &str, expanded: bool) -> Result<u8> {
    let branches = session
        .execute(Command::HistoryBranches)?
        .history_branches()?;
    let Some(branch) = branches
        .iter()
        .find(|branch| branch.name == reference || branch.reference == reference)
    else {
        return Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("`{reference}` does not name a branch in this repository"),
        )
        .hint("run `quinjet branch list --all` for the branches that exist")
        .into());
    };
    let status = session.execute(Command::Status)?.status()?;
    let document = whole_document(
        session,
        Command::PrepareLocalDiff {
            workspace: 0,
            request: Box::new(LocalDiffRequest::Branch {
                branch: Box::new(branch.clone()),
                current: status.branch.head.clone(),
                current_oid: status.branch.oid,
                expanded,
            }),
        },
        |workspace, path| Command::LocalDiffFile { workspace, path },
    )?;
    out.emit(&document, || render::diff(&document))?;
    Ok(0)
}

fn worktree(session: &mut Session, out: &Emitter, command: WorktreeVerb) -> Result<u8> {
    match command {
        WorktreeVerb::List => {
            let worktrees = session.execute(Command::Worktrees)?.worktrees()?;
            out.emit(&worktrees, || render::worktrees(&worktrees))?;
            Ok(0)
        }
    }
}

fn stash(session: &mut Session, out: &Emitter, command: StashVerb) -> Result<u8> {
    match command {
        StashVerb::List => {
            let stashes = session.execute(Command::Stashes)?.stashes()?;
            out.emit(&stashes, || render::stashes(&stashes))?;
            Ok(0)
        }
        StashVerb::Push {
            message,
            include_untracked,
            staged,
            paths,
        } => operate(
            session,
            out,
            GitOperation::StashPush {
                message,
                include_untracked,
                staged,
                paths,
            },
        ),
        StashVerb::Apply { reference } => {
            operate(session, out, GitOperation::StashApply(reference))
        }
        StashVerb::Pop { reference } => operate(session, out, GitOperation::StashPop(reference)),
        StashVerb::Drop { reference, yes } => {
            if !yes {
                out.message(&format!("Would drop `{reference}`. Pass --yes to drop it."))?;
                return Ok(0);
            }
            operate(session, out, GitOperation::StashDrop(reference))
        }
        StashVerb::Clear { yes } => {
            if !yes {
                let stashes = session.execute(Command::Stashes)?.stashes()?;
                out.message(&format!(
                    "Would drop {} stashes. Pass --yes to drop them.",
                    stashes.len()
                ))?;
                return Ok(0);
            }
            operate(session, out, GitOperation::StashClear)
        }
        StashVerb::Show {
            reference,
            expanded,
        } => {
            let stashes = session.execute(Command::Stashes)?.stashes()?;
            let Some(stash) = stashes.iter().find(|stash| stash.reference == reference) else {
                return Err(Failure::new(
                    EXIT_NOT_FOUND,
                    format!("`{reference}` does not name a stash in this repository"),
                )
                .hint("run `quinjet stash list` for the stashes that exist")
                .into());
            };
            let document = whole_document(
                session,
                Command::PrepareLocalDiff {
                    workspace: 0,
                    request: Box::new(LocalDiffRequest::Stash {
                        stash: Box::new(stash.clone()),
                        expanded,
                    }),
                },
                |workspace, path| Command::LocalDiffFile { workspace, path },
            )?;
            out.emit(&document, || render::diff(&document))?;
            Ok(0)
        }
    }
}

fn discard(session: &mut Session, out: &Emitter, args: &DiscardArgs) -> Result<u8> {
    let status = session.execute(Command::Status)?.status()?;
    let changes: Vec<Change> = status
        .changes
        .iter()
        .filter(|change| change.area != ChangeArea::Conflict)
        .filter(|change| args.selection.all || matches(&change.path, &args.selection.paths))
        .cloned()
        .collect();
    if !args.selection.all && args.selection.paths.is_empty() {
        return Err(Failure::new(
            EXIT_FAILURE,
            "discard needs paths, or --all for every change",
        )
        .into());
    }
    if changes.is_empty() {
        out.message("No changes match")?;
        return Ok(0);
    }
    if !args.yes {
        let paths: Vec<String> = changes.iter().map(Change::display_path).collect();
        out.message(&format!(
            "Would discard {} change(s): {}. Pass --yes to discard them.",
            paths.len(),
            paths.join(", ")
        ))?;
        return Ok(0);
    }
    operate(session, out, GitOperation::Discard(changes))
}

fn resolve(session: &mut Session, out: &Emitter, args: ResolveArgs) -> Result<u8> {
    let operation = if args.side.stage {
        GitOperation::Stage(vec![args.path])
    } else if args.side.ours {
        GitOperation::ResolveConflict {
            path: args.path,
            choice: ConflictChoice::Ours,
        }
    } else if args.side.theirs {
        GitOperation::ResolveConflict {
            path: args.path,
            choice: ConflictChoice::Theirs,
        }
    } else {
        return Err(Failure::new(
            EXIT_FAILURE,
            "resolve needs one of --ours, --theirs or --stage",
        )
        .into());
    };
    operate(session, out, operation)
}

fn repositories(session: &mut Session, out: &Emitter, args: &ReposArgs) -> Result<u8> {
    let (repositories, warnings) = session
        .execute(Command::GitHubRepositories {
            refresh: args.refresh,
        })?
        .github_repositories()?;
    out.emit(
        &RepositoryListing {
            repositories: &repositories,
            warnings: &warnings,
        },
        || render::repositories(&repositories, &warnings),
    )?;
    Ok(0)
}

#[derive(Serialize)]
struct RepositoryListing<'a> {
    repositories: &'a [GitHubRepository],
    warnings: &'a [String],
}

fn pull_request(session: &mut Session, out: &Emitter, command: PrVerb) -> Result<u8> {
    match command {
        PrVerb::View(args) => {
            if args.watch {
                return watch_pull_request(session, out, &args);
            }
            let snapshot = lookup_snapshot(session, out, &args.pull_request)?;
            report_warnings(out, &snapshot);
            out.emit(&snapshot, || render::pull_request(&snapshot.pull_request))?;
            Ok(0)
        }
        PrVerb::Files(args) => {
            let request = lookup(session, out, &args)?;
            let index = prepare(session, out, &request)?;
            out.emit(&index, || render::pull_request_files(&index))?;
            Ok(0)
        }
        PrVerb::Diff(args) => {
            let request = lookup(session, out, &args.pull_request)?;
            let document = pull_request_diff(session, out, &request, args.path.as_deref())?;
            out.emit(&document, || render::diff(&document))?;
            Ok(0)
        }
        PrVerb::Conversation(args) => {
            if args.watch {
                return watch_conversation(session, out, &args);
            }
            let request = lookup(session, out, &args.pull_request)?;
            let conversation = out
                .execute(
                    session,
                    Command::PullRequestConversation {
                        pull_request: Box::new(request),
                    },
                )?
                .conversation()?;
            out.emit(&conversation, || render::conversation(&conversation))?;
            Ok(0)
        }
        PrVerb::Checks(args) => checks(session, out, &args),
        PrVerb::Logs(args) => logs(session, out, &args),
        PrVerb::Open(args) => {
            let request = lookup(session, out, &args.pull_request)?;
            let url = match args.check {
                None => request.url,
                Some(name) => {
                    let listing = out
                        .execute(
                            session,
                            Command::PullRequestChecks {
                                pull_request: Box::new(request),
                                refresh: args.pull_request.refresh,
                            },
                        )?
                        .checks()?;
                    let check = select_check(&listing.checks, &name)?;
                    if check.link.is_empty() {
                        return Err(Failure::new(
                            EXIT_UNAVAILABLE,
                            format!("the `{}` check has no browser URL", check.name),
                        )
                        .into());
                    }
                    check.link
                }
            };
            open_url(&url)?;
            out.message(&format!("Opened {url}"))?;
            Ok(0)
        }
        PrVerb::Merge(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Merge {
                method: args.method.method(),
                delete_branch: args.delete_branch,
            },
        ),
        PrVerb::Close(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Close,
        ),
        PrVerb::Reopen(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Reopen,
        ),
    }
}

fn mutate_pull_request(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
    yes: bool,
    operation: PullRequestOperation,
) -> Result<u8> {
    let pull_request = lookup(session, out, args)?;
    if !yes {
        out.message(&preview_pull_request_operation(&pull_request, &operation))?;
        return Ok(0);
    }
    let message = session
        .execute(Command::OperatePullRequest {
            pull_request: Box::new(pull_request),
            operation,
        })?
        .operation()?
        .2;
    out.message(&message)?;
    Ok(0)
}

fn preview_pull_request_operation(
    pull_request: &PullRequest,
    operation: &PullRequestOperation,
) -> String {
    let mut message = match operation {
        PullRequestOperation::Merge { method, .. } => {
            let mut text = String::from("Would ");
            text.push_str(method.preview_verb());
            text
        }
        PullRequestOperation::Close => String::from("Would close"),
        PullRequestOperation::Reopen => String::from("Would reopen"),
    };
    message.push_str(" #");
    message.push_str(&pull_request.number.to_string());
    message.push_str(" (");
    message.push_str(&pull_request.title);
    message.push_str("). Pass --yes to ");
    match operation {
        PullRequestOperation::Merge { .. } => message.push_str("merge it."),
        PullRequestOperation::Close => message.push_str("close it."),
        PullRequestOperation::Reopen => message.push_str("reopen it."),
    }
    message
}

fn watch_pull_request(session: &mut Session, out: &Emitter, args: &PrWatchArgs) -> Result<u8> {
    watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
        let mut request = args.pull_request.clone();
        request.refresh = true;
        let snapshot = lookup_snapshot(session, out, &request)?;
        Ok(watch::Frame {
            text: render::pull_request(&snapshot.pull_request),
            value: snapshot,
            finished: false,
            code: 0,
        })
    })
}

fn watch_conversation(session: &mut Session, out: &Emitter, args: &PrWatchArgs) -> Result<u8> {
    watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
        let mut lookup_args = args.pull_request.clone();
        lookup_args.refresh = true;
        let request = lookup_snapshot(session, out, &lookup_args)?.pull_request;
        let conversation = session
            .execute(Command::PullRequestConversation {
                pull_request: Box::new(request),
            })?
            .conversation()?;
        Ok(watch::Frame {
            text: render::conversation(&conversation),
            value: conversation,
            finished: false,
            code: 0,
        })
    })
}

fn checks(session: &mut Session, out: &Emitter, args: &PrChecksArgs) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    if args.watch {
        return watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
            let checks = session
                .execute(Command::PullRequestChecks {
                    pull_request: Box::new(request.clone()),
                    refresh: true,
                })?
                .checks()?;
            let settled = !checks.checks.iter().any(|check| check.status.is_running());
            Ok(watch::Frame {
                text: render::checks(&checks.checks),
                finished: settled && !checks.checks.is_empty(),
                code: exit_for(&checks.checks),
                value: checks,
            })
        });
    }
    let checks = out
        .execute(
            session,
            Command::PullRequestChecks {
                pull_request: Box::new(request),
                refresh: args.pull_request.refresh,
            },
        )?
        .checks()?;
    out.emit(&checks, || render::checks(&checks.checks))?;
    Ok(if args.exit_code {
        exit_for(&checks.checks)
    } else {
        0
    })
}

fn logs(session: &mut Session, out: &Emitter, args: &PrLogsArgs) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    let listing = out
        .execute(
            session,
            Command::PullRequestChecks {
                pull_request: Box::new(request.clone()),
                refresh: args.pull_request.refresh,
            },
        )?
        .checks()?;
    let check = select_check(&listing.checks, &args.check)?;
    if args.watch {
        let name = check.name;
        return watch::run(interval(args.interval, LOG_WATCH_FLOOR), out.json, || {
            let listing = session
                .execute(Command::PullRequestChecks {
                    pull_request: Box::new(request.clone()),
                    refresh: true,
                })?
                .checks()?;
            let check = select_check(&listing.checks, &name)?;
            let log = session
                .execute(Command::CheckRunLog {
                    pull_request: Box::new(request.clone()),
                    check: Box::new(check.clone()),
                })?
                .check_log()?;
            ensure_log_available(&log)?;
            Ok(watch::Frame {
                text: render::check_log(&check, &log),
                finished: !check.status.is_running(),
                code: u8::from(check.status == PullRequestCheckStatus::Failed),
                value: log,
            })
        });
    }
    let log = out
        .execute(
            session,
            Command::CheckRunLog {
                pull_request: Box::new(request),
                check: Box::new(check.clone()),
            },
        )?
        .check_log()?;
    ensure_log_available(&log)?;
    out.emit(&log, || render::check_log(&check, &log))?;
    Ok(0)
}

fn ensure_log_available(log: &CheckRunLog) -> Result<()> {
    log.unavailable.as_ref().map_or_else(
        || Ok(()),
        |reason| Err(Failure::new(EXIT_UNAVAILABLE, reason.clone()).into()),
    )
}

fn select_check(checks: &[PullRequestCheck], wanted: &str) -> Result<PullRequestCheck> {
    let exact: Vec<&PullRequestCheck> =
        checks.iter().filter(|check| check.name == wanted).collect();
    if let Some(check) = exact.first() {
        return Ok((*check).clone());
    }
    let partial: Vec<&PullRequestCheck> = checks
        .iter()
        .filter(|check| check.name.to_lowercase().contains(&wanted.to_lowercase()))
        .collect();
    match partial.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("no check on this pull request is called `{wanted}`"),
        )
        .hint(format!(
            "the checks are: {}",
            checks
                .iter()
                .map(|check| check.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
        _ => Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("`{wanted}` matches more than one check"),
        )
        .hint(format!(
            "name one of: {}",
            partial
                .iter()
                .map(|check| check.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
    }
}

fn exit_for(checks: &[PullRequestCheck]) -> u8 {
    let unhappy = checks.iter().any(|check| {
        matches!(
            check.status,
            PullRequestCheckStatus::Failed | PullRequestCheckStatus::Pending
        )
    });
    u8::from(unhappy)
}

fn lookup(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<PullRequest> {
    let snapshot = lookup_snapshot(session, out, args)?;
    report_warnings(out, &snapshot);
    Ok(snapshot.pull_request)
}

fn lookup_snapshot(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
) -> Result<PullRequestSnapshot> {
    let repositories = match &args.repo {
        None => Vec::new(),
        Some(_) => {
            out.execute(session, Command::GitHubRepositories { refresh: false })?
                .github_repositories()?
                .0
        }
    };
    let selected = match &args.repo {
        None => None,
        Some(wanted) => {
            let found = repositories
                .iter()
                .find(|repository| {
                    repository.name_with_owner.eq_ignore_ascii_case(wanted)
                        || repository.url.ends_with(wanted.as_str())
                })
                .cloned();
            match found {
                Some(repository) => Some(Box::new(repository)),
                None => {
                    return Err(Failure::new(
                        EXIT_NOT_FOUND,
                        format!("no remote of this checkout points at `{wanted}`"),
                    )
                    .hint("run `quinjet repos` for the repositories it can see")
                    .into());
                }
            }
        }
    };
    out.execute(
        session,
        Command::PullRequestLookup {
            repositories,
            repository: selected,
            number: args.number,
            refresh: args.refresh,
        },
    )?
    .pull_request()
}

fn report_warnings(out: &Emitter, snapshot: &PullRequestSnapshot) {
    for warning in &snapshot.warnings {
        out.note(&format!("warning: {warning}"));
    }
}

fn prepare(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
) -> Result<PullRequestDiffIndex> {
    out.execute(
        session,
        Command::PreparePullRequest {
            workspace: 0,
            pull_request: Box::new(request.clone()),
        },
    )?
    .pull_request_index()
}

fn pull_request_diff(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    path: Option<&Path>,
) -> Result<DiffDocument> {
    let index = prepare(session, out, request)?;
    let paths: Vec<PathBuf> = match path {
        Some(wanted) => {
            if !index.files.iter().any(|file| file.path == wanted) {
                return Err(Failure::new(
                    EXIT_NOT_FOUND,
                    format!("`{}` is not part of this pull request", wanted.display()),
                )
                .hint("run `quinjet pr files <number>` for the files it changes")
                .into());
            }
            vec![wanted.to_path_buf()]
        }
        None => index.files.iter().map(|file| file.path.clone()).collect(),
    };
    let mut loaded = HashMap::new();
    for chunk in paths.chunks(16) {
        for (path, document) in out
            .execute(
                session,
                Command::PullRequestFileBatch {
                    workspace: 0,
                    paths: chunk.to_vec(),
                },
            )?
            .pull_request_diff_batch()?
        {
            drop(loaded.insert(path, document));
        }
    }
    let index = DiffIndex {
        title: format!("PR #{}", request.number),
        files: index
            .files
            .iter()
            .filter(|file| loaded.contains_key(&file.path))
            .map(|file| crate::git::diff::DiffFileIndexEntry {
                path: file.path.clone(),
                old_path: file.old_path.clone(),
                status: render::pull_request_file_label(file.status).to_owned(),
                counts: file.counts,
            })
            .collect(),
        truncated: index.truncated,
        commit_details: None,
    };
    Ok(index.document_with_visibility(&loaded, |_| true))
}

fn whole_document(
    session: &mut Session,
    prepare: Command,
    file: impl Fn(u64, PathBuf) -> Command,
) -> Result<DiffDocument> {
    let index = session.execute(prepare)?.local_diff_index()?;
    let mut loaded = HashMap::new();
    for entry in &index.files {
        let (path, document) = session
            .execute(file(0, entry.path.clone()))?
            .local_diff_file()?;
        drop(loaded.insert(path, document));
    }
    Ok(index.document_with_visibility(&loaded, |_| true))
}

fn operate(session: &mut Session, out: &Emitter, operation: GitOperation) -> Result<u8> {
    let (_, _, message) = session.execute(Command::Operate(operation))?.operation()?;
    out.message(&message)?;
    Ok(0)
}

fn revision_operation(
    session: &mut Session,
    out: &Emitter,
    args: &RevisionArgs,
    action: &str,
    operation: impl FnOnce(String) -> GitOperation,
) -> Result<u8> {
    let revision = revision(session, &args.revision)?;
    if !args.yes {
        out.message(&format!(
            "Would {action} `{revision}`. Pass --yes to {action} it."
        ))?;
        return Ok(0);
    }
    operate(session, out, operation(revision))
}

fn revision(session: &Session, value: &str) -> Result<String> {
    session.repository_revision(value).map_err(|error| {
        Failure::new(EXIT_NOT_FOUND, format!("{error:#}"))
            .hint("run `quinjet log` or `quinjet branch list --all` for what this repository holds")
            .into()
    })
}

fn require_paths(paths: Vec<PathBuf>, verb: &str) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        return Err(Failure::new(
            EXIT_FAILURE,
            format!("{verb} needs paths, or --all for every change"),
        )
        .into());
    }
    Ok(paths)
}

fn matches(path: &Path, filters: &[PathBuf]) -> bool {
    filters.is_empty() || filters.iter().any(|filter| path.starts_with(filter))
}

const fn interval(seconds: u64, floor: u64) -> Duration {
    Duration::from_secs(if seconds < floor { floor } else { seconds })
}

pub(crate) fn open_url(url: &str) -> Result<()> {
    if std::env::var_os("CMUX_SOCKET_PATH").is_some()
        && let Ok(child) = std::process::Command::new("cmux")
            .args(["browser", "open"])
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    {
        drop(child);
        return Ok(());
    }
    open_target(OsStr::new(url), url)
}

#[expect(
    dead_code,
    reason = "filesystem counterpart to open_url, kept for path links"
)]
pub(crate) fn open_path(path: &Path) -> Result<()> {
    open_target(path.as_os_str(), &path.display().to_string())
}

fn open_target(target: &OsStr, display: &str) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    drop(
        std::process::Command::new(opener)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to hand {display} to {opener}"))?,
    );
    Ok(())
}

fn note(text: &str) {
    drop(writeln!(io::stderr().lock(), "{text}"));
}

pub(crate) fn report(error: &anyhow::Error) -> u8 {
    if let Some(broken) = error.downcast_ref::<io::Error>()
        && broken.kind() == io::ErrorKind::BrokenPipe
    {
        return 0;
    }
    let failure = error.downcast_ref::<Failure>();
    note(&format!("error: {error:#}"));
    if let Some(hint) = failure.and_then(|failure| failure.hint.as_deref()) {
        note(&format!("hint: {hint}"));
    }
    failure.map_or(EXIT_FAILURE, |failure| failure.code)
}

pub(crate) fn stdout_is_terminal() -> bool {
    io::stdout().is_terminal()
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use indicatif::InMemoryTerm;

    use super::*;
    use crate::git::github::PullRequestCheck;

    static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestRepository {
        path: PathBuf,
    }

    impl TestRepository {
        fn new() -> Self {
            let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
            let name = format!("quinjet-cli-test-{}-{id}", std::process::id());
            // nosemgrep: rust.lang.security.temp-dir.temp-dir
            let path = std::env::temp_dir().join(name);
            drop(fs::remove_dir_all(&path));
            fs::create_dir_all(&path).unwrap();
            let repository = Self { path };
            repository.git(&["init", "--initial-branch=main"]);
            repository.git(&["config", "user.name", "Quinjet Test"]);
            repository.git(&["config", "user.email", "quinjet@example.com"]);
            fs::write(repository.path.join("README.md"), "one\n").unwrap();
            repository.git(&["add", "README.md"]);
            repository.git(&["commit", "--message=base"]);
            repository
        }

        fn git(&self, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&self.path)
                .args(args)
                .env("LC_ALL", "C")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }

        fn session(&self) -> Session {
            Session::new(Repository::discover(&self.path).unwrap())
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.path));
        }
    }

    fn check(name: &str, status: PullRequestCheckStatus) -> PullRequestCheck {
        PullRequestCheck {
            name: name.to_owned(),
            workflow: "CI".to_owned(),
            state: String::new(),
            status,
            description: String::new(),
            link: String::new(),
            started_at: String::new(),
            completed_at: String::new(),
        }
    }

    #[test]
    fn the_command_tree_is_unambiguous() {
        Cli::command().debug_assert();
    }

    #[test]
    fn progress_requires_human_output_and_an_interactive_stderr() {
        assert!(progress_enabled(false, true));
        assert!(!progress_enabled(true, true));
        assert!(!progress_enabled(false, false));
    }

    #[test]
    fn progress_updates_its_phase_and_clears_when_finished() {
        let terminal = InMemoryTerm::new(2, 80);
        let progress = progress_bar(
            "Loading pull request",
            ProgressDrawTarget::term_like(Box::new(terminal.clone())),
        )
        .unwrap();
        progress.tick();
        assert!(terminal.contents().contains("Loading pull request"));
        let out = Emitter {
            json: false,
            progress: Some(progress),
        };

        out.set_progress("Fetching pull-request checks");
        out.progress.as_ref().unwrap().tick();
        assert!(terminal.contents().contains("Fetching pull-request checks"));
        out.note("warning: using stale metadata");
        assert!(
            terminal
                .contents()
                .contains("warning: using stale metadata")
        );

        out.finish_progress();
        assert_eq!(terminal.contents(), "warning: using stale metadata");
    }

    #[test]
    fn completion_hints_distinguish_paths_from_identifiers() {
        let root = Cli::command();
        let diff = root.find_subcommand("diff").unwrap();
        let diff_path = diff
            .get_arguments()
            .find(|argument| argument.get_id() == "paths")
            .unwrap();
        assert_eq!(diff_path.get_value_hint(), ValueHint::AnyPath);

        let branch = root.find_subcommand("branch").unwrap();
        let switch = branch.find_subcommand("switch").unwrap();
        let branch_name = switch
            .get_arguments()
            .find(|argument| argument.get_id() == "name")
            .unwrap();
        assert_eq!(branch_name.get_value_hint(), ValueHint::Other);

        let status = root.find_subcommand("status").unwrap();
        let interval = status
            .get_arguments()
            .find(|argument| argument.get_id() == "interval")
            .unwrap();
        assert_eq!(interval.get_value_hint(), ValueHint::Other);
    }

    #[test]
    fn a_terminal_path_belongs_to_the_tui_verb() {
        let cli = Cli::try_parse_from(["quinjet", "tui", "/tmp/somewhere"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Verb::Tui(TuiArgs { path, .. })) if path == Path::new("/tmp/somewhere")
        ));
    }

    #[test]
    fn terminal_themes_default_to_quinjet_with_system_appearance() {
        let cli = Cli::try_parse_from(["quinjet", "tui"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Verb::Tui(TuiArgs {
                theme: ThemeName::Quinjet,
                appearance: AppearanceChoice::System,
                ..
            }))
        ));

        let cli = Cli::try_parse_from([
            "quinjet",
            "tui",
            "--theme",
            "rose-pine",
            "--appearance",
            "light",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Some(Verb::Tui(TuiArgs {
                theme: ThemeName::RosePine,
                appearance: AppearanceChoice::Light,
                ..
            }))
        ));
        drop(Cli::try_parse_from(["quinjet", "tui", "--theme", "unknown"]).unwrap_err());
        drop(Cli::try_parse_from(["quinjet", "tui", "--appearance", "unknown"]).unwrap_err());
    }

    #[test]
    fn an_unknown_verb_is_a_usage_error() {
        drop(Cli::try_parse_from(["quinjet", "statsu"]).unwrap_err());
    }

    #[test]
    fn a_verb_is_never_mistaken_for_a_repository_path() {
        let cli = Cli::try_parse_from(["quinjet", "status"]).unwrap();
        assert!(
            matches!(cli.command, Some(Verb::Status(_))),
            "`quinjet status` must reach the status verb rather than open a directory called status"
        );
    }

    #[test]
    fn every_subcommand_answers_to_the_repository_and_json_switches() {
        for argv in [
            vec!["quinjet", "-C", "/tmp/elsewhere", "--json", "status"],
            vec!["quinjet", "status", "-C", "/tmp/elsewhere", "--json"],
            vec![
                "quinjet",
                "pr",
                "checks",
                "1",
                "--json",
                "-C",
                "/tmp/elsewhere",
            ],
        ] {
            let cli = Cli::try_parse_from(&argv).unwrap();
            assert!(cli.json, "{argv:?} must be readable as JSON");
            assert_eq!(cli.repository, PathBuf::from("/tmp/elsewhere"), "{argv:?}");
        }
    }

    #[test]
    fn pull_request_live_and_browser_options_parse_at_their_leaf() {
        let view =
            Cli::try_parse_from(["quinjet", "pr", "view", "24", "--watch", "--interval", "9"])
                .unwrap();
        assert!(matches!(
            view.command,
            Some(Verb::Pr {
                command: PrVerb::View(PrWatchArgs {
                    watch: true,
                    interval: 9,
                    ..
                })
            })
        ));

        let open =
            Cli::try_parse_from(["quinjet", "pr", "open", "24", "--check", "Clippy"]).unwrap();
        assert!(matches!(
            open.command,
            Some(Verb::Pr {
                command: PrVerb::Open(PrOpenArgs {
                    check: Some(name),
                    ..
                })
            }) if name == "Clippy"
        ));

        let merge = Cli::try_parse_from([
            "quinjet",
            "pr",
            "merge",
            "24",
            "--squash",
            "--delete-branch",
            "--yes",
        ])
        .unwrap();
        assert!(matches!(
            merge.command,
            Some(Verb::Pr {
                command: PrVerb::Merge(PrMergeArgs {
                    method: PrMergeMethodArgs {
                        squash: true,
                        merge: false,
                        rebase: false,
                    },
                    delete_branch: true,
                    yes: true,
                    ..
                })
            })
        ));

        let close = Cli::try_parse_from(["quinjet", "pr", "close", "24"]).unwrap();
        assert!(matches!(
            close.command,
            Some(Verb::Pr {
                command: PrVerb::Close(PrMutateArgs { yes: false, .. })
            })
        ));

        assert!(
            Cli::try_parse_from(["quinjet", "pr", "merge", "24"]).is_err(),
            "merge without a method must be rejected"
        );
    }

    #[test]
    fn a_session_answers_a_status_command_with_the_working_tree() {
        let repository = TestRepository::new();
        fs::write(repository.path.join("added.txt"), "new\n").unwrap();
        let mut session = repository.session();

        let status = session.execute(Command::Status).unwrap().status().unwrap();

        assert_eq!(status.branch.head, "main");
        assert!(
            status
                .changes
                .iter()
                .any(|change| change.path == Path::new("added.txt")),
            "the untracked file belongs in the answer: {status:?}"
        );
    }

    #[test]
    fn a_prepared_workspace_answers_only_the_generation_that_asked_for_it() {
        let repository = TestRepository::new();
        fs::write(repository.path.join("README.md"), "two\n").unwrap();
        let mut session = repository.session();
        let status = session.execute(Command::Status).unwrap().status().unwrap();
        session
            .execute(Command::PrepareLocalDiff {
                workspace: 7,
                request: Box::new(LocalDiffRequest::Changes {
                    changes: status.changes,
                    version: 0,
                    expanded: false,
                }),
            })
            .unwrap();

        let mine = session.execute(Command::LocalDiffFile {
            workspace: 7,
            path: PathBuf::from("README.md"),
        });
        let stale = session.execute(Command::LocalDiffFile {
            workspace: 8,
            path: PathBuf::from("README.md"),
        });

        assert!(
            mine.is_ok(),
            "the generation that prepared it must be answered"
        );
        assert!(
            stale.is_err(),
            "a workspace must never answer a generation it was not prepared for"
        );
    }

    #[test]
    fn an_operation_command_reports_what_it_did() {
        let repository = TestRepository::new();
        fs::write(repository.path.join("added.txt"), "new\n").unwrap();
        let mut session = repository.session();

        let (label, changes_history, message) = session
            .execute(Command::Operate(GitOperation::StageAll))
            .unwrap()
            .operation()
            .unwrap();

        assert_eq!(label, "Staging all changes");
        assert!(!changes_history);
        assert_eq!(message, "All changes staged");
        assert!(
            repository
                .git(&["diff", "--cached", "--name-only"])
                .contains("added.txt"),
            "staging through the command layer must reach the index"
        );
    }

    #[test]
    fn a_revision_resolves_from_what_a_person_would_type() {
        let repository = TestRepository::new();
        let head = repository.git(&["rev-parse", "HEAD"]);
        repository.git(&["tag", "v1"]);
        let session = repository.session();

        assert_eq!(session.repository_revision("HEAD").unwrap(), "HEAD");
        assert_eq!(
            session.repository_revision("main").unwrap(),
            "refs/heads/main"
        );
        assert_eq!(session.repository_revision("v1").unwrap(), "refs/tags/v1");
        let short: String = head.chars().take(8).collect();
        assert_eq!(session.repository_revision(&short).unwrap(), head);
    }

    #[test]
    fn a_revision_that_names_nothing_is_a_name_that_was_not_found() {
        let repository = TestRepository::new();
        let session = repository.session();

        let error = revision(&session, "deadbeefdead").unwrap_err();
        let failure = error.downcast_ref::<Failure>().unwrap();

        assert_eq!(
            failure.code, EXIT_NOT_FOUND,
            "a revision that resolves to nothing is a missing name, not a failed command"
        );
        assert!(
            failure.hint.is_some(),
            "exit 3 always says what could be named instead"
        );
    }

    #[test]
    fn a_revision_that_could_be_read_as_an_option_is_refused_before_git_sees_it() {
        let repository = TestRepository::new();
        let session = repository.session();

        for revision in ["--output=/tmp/owned", "-n", ""] {
            assert!(
                session.repository_revision(revision).is_err(),
                "`{revision}` must never reach Git as a revision"
            );
        }
    }

    #[test]
    fn watching_checks_stops_only_once_nothing_is_still_running() {
        let running = [
            check("one", PullRequestCheckStatus::Passed),
            check("two", PullRequestCheckStatus::Pending),
        ];
        let settled = [
            check("one", PullRequestCheckStatus::Passed),
            check("two", PullRequestCheckStatus::Failed),
        ];

        assert!(running.iter().any(|check| check.status.is_running()));
        assert!(!settled.iter().any(|check| check.status.is_running()));
        assert_eq!(exit_for(&running), 1, "a pending check is not a green run");
        assert_eq!(exit_for(&settled), 1, "a failed check is not a green run");
        assert_eq!(exit_for(&[check("one", PullRequestCheckStatus::Passed)]), 0);
    }

    #[test]
    fn naming_a_check_that_matches_nothing_says_which_ones_exist() {
        let checks = [
            check(
                "Format, lint, and test (ubuntu-latest)",
                PullRequestCheckStatus::Passed,
            ),
            check(
                "Format, lint, and test (macos-latest)",
                PullRequestCheckStatus::Passed,
            ),
            check("Package validation", PullRequestCheckStatus::Passed),
        ];

        assert_eq!(
            select_check(&checks, "Package validation").unwrap().name,
            "Package validation"
        );
        assert_eq!(
            select_check(&checks, "package").unwrap().name,
            "Package validation",
            "one partial match is enough to name a check"
        );

        let ambiguous = select_check(&checks, "Format").unwrap_err();
        let ambiguous = ambiguous.downcast_ref::<Failure>().unwrap();
        assert_eq!(ambiguous.code, EXIT_NOT_FOUND);
        assert!(ambiguous.hint.as_ref().unwrap().contains("ubuntu-latest"));

        let missing = select_check(&checks, "nothing").unwrap_err();
        assert_eq!(
            missing.downcast_ref::<Failure>().unwrap().code,
            EXIT_NOT_FOUND
        );
    }

    #[test]
    fn unavailable_logs_use_the_same_exit_in_watch_and_one_shot_modes() {
        let log = CheckRunLog {
            unavailable: Some("no Actions job".to_owned()),
            ..CheckRunLog::default()
        };
        let error = ensure_log_available(&log).unwrap_err();
        assert_eq!(
            error.downcast_ref::<Failure>().unwrap().code,
            EXIT_UNAVAILABLE
        );
        ensure_log_available(&CheckRunLog::default()).unwrap();
    }

    #[test]
    fn a_destructive_verb_changes_nothing_until_it_is_confirmed() {
        let repository = TestRepository::new();
        fs::write(repository.path.join("README.md"), "changed\n").unwrap();
        let mut session = repository.session();
        let out = Emitter::new(true);

        discard(
            &mut session,
            &out,
            &DiscardArgs {
                selection: SelectionArgs {
                    paths: vec![PathBuf::from("README.md")],
                    all: false,
                },
                yes: false,
            },
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(repository.path.join("README.md")).unwrap(),
            "changed\n",
            "a discard without --yes must leave the working tree alone"
        );
    }

    fn resolves(path: &[&str]) -> bool {
        let root = Cli::command();
        let mut command = &root;
        let mut found = Vec::new();
        for name in path {
            let Some(child) = command.find_subcommand(name) else {
                return false;
            };
            found.push(child);
            command = found.last().unwrap();
        }
        true
    }

    macro_rules! operation_routes {
        ($($pattern:pat => $sample:expr => [$($path:literal),+];)+) => {
            const fn verb_for(operation: &GitOperation) -> &'static [&'static str] {
                match operation {
                    $($pattern => &[$($path),+],)+
                }
            }

            #[test]
            fn every_operation_the_interface_performs_has_a_verb() {
                let operations = [$($sample),+];
                for operation in &operations {
                    let path = verb_for(operation);
                    assert!(
                        resolves(path),
                        "{operation:?} names the verb {path:?}, which the command tree does not have"
                    );
                }
                let kinds: HashSet<_> = operations.iter().map(std::mem::discriminant).collect();
                assert_eq!(
                    kinds.len(),
                    operations.len(),
                    "every operation variant must have one route fixture"
                );
            }
        };
    }

    operation_routes! {
        GitOperation::Stage(_) => GitOperation::Stage(Vec::new()) => ["stage"];
        GitOperation::StageAll => GitOperation::StageAll => ["stage"];
        GitOperation::Unstage(_) => GitOperation::Unstage(Vec::new()) => ["unstage"];
        GitOperation::UnstageAll => GitOperation::UnstageAll => ["unstage"];
        GitOperation::Discard(_) => GitOperation::Discard(Vec::new()) => ["discard"];
        GitOperation::Commit { .. } => GitOperation::Commit { message: String::new(), amend: false } => ["commit"];
        GitOperation::Fetch => GitOperation::Fetch => ["fetch"];
        GitOperation::Pull => GitOperation::Pull => ["pull"];
        GitOperation::Push => GitOperation::Push => ["push"];
        GitOperation::Sync => GitOperation::Sync => ["sync"];
        GitOperation::Checkout(_) => GitOperation::Checkout(String::new()) => ["branch", "switch"];
        GitOperation::CreateBranch { .. } => GitOperation::CreateBranch { name: String::new(), start: None } => ["branch", "create"];
        GitOperation::RenameBranch { .. } => GitOperation::RenameBranch { old: String::new(), new: String::new() } => ["branch", "rename"];
        GitOperation::DeleteBranch(_) => GitOperation::DeleteBranch(String::new()) => ["branch", "delete"];
        GitOperation::StashPush { .. } => GitOperation::StashPush { message: String::new(), include_untracked: false, staged: false, paths: Vec::new() } => ["stash", "push"];
        GitOperation::StashApply(_) => GitOperation::StashApply(String::new()) => ["stash", "apply"];
        GitOperation::StashPop(_) => GitOperation::StashPop(None) => ["stash", "pop"];
        GitOperation::StashDrop(_) => GitOperation::StashDrop(String::new()) => ["stash", "drop"];
        GitOperation::StashClear => GitOperation::StashClear => ["stash", "clear"];
        GitOperation::ResolveConflict { .. } => GitOperation::ResolveConflict { path: PathBuf::new(), choice: ConflictChoice::Ours } => ["resolve"];
        GitOperation::CherryPick(_) => GitOperation::CherryPick(String::new()) => ["cherry-pick"];
        GitOperation::Revert(_) => GitOperation::Revert(String::new()) => ["revert"];
    }

    #[test]
    fn the_read_only_views_have_verbs_too() {
        for path in [
            ["status"].as_slice(),
            ["diff"].as_slice(),
            ["log"].as_slice(),
            ["show"].as_slice(),
            ["branch", "list"].as_slice(),
            ["branch", "compare"].as_slice(),
            ["stash", "list"].as_slice(),
            ["stash", "show"].as_slice(),
            ["worktree", "list"].as_slice(),
            ["repos"].as_slice(),
            ["pr", "view"].as_slice(),
            ["pr", "files"].as_slice(),
            ["pr", "diff"].as_slice(),
            ["pr", "checks"].as_slice(),
            ["pr", "conversation"].as_slice(),
            ["pr", "logs"].as_slice(),
            ["pr", "open"].as_slice(),
            ["tui"].as_slice(),
            ["completions"].as_slice(),
            ["man"].as_slice(),
            ["capabilities"].as_slice(),
        ] {
            assert!(resolves(path), "the command tree is missing {path:?}");
        }
    }
}
