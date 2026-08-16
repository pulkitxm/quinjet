pub(crate) mod command;
mod render;
mod watch;

use std::collections::HashMap;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use clap_mangen::Man;
pub(crate) use command::{Command, Outcome, Session};
use serde::Serialize;

use crate::git::diff::{DiffDocument, DiffIndex};
use crate::git::github::{
    GitHubRepository, PullRequest, PullRequestCheck, PullRequestCheckStatus, PullRequestDiffIndex,
};
use crate::git::status::{Change, ChangeArea};
use crate::git::{ConflictChoice, GitOperation, LocalDiffRequest, Repository};

pub(crate) const EXIT_FAILURE: u8 = 1;
pub(crate) const EXIT_NOT_FOUND: u8 = 3;
pub(crate) const EXIT_UNAVAILABLE: u8 = 4;

/// The name the binary is invoked by, and the name its generated shell
/// completions and manual pages are written under.
const PROGRAM: &str = "quinjet";

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
struct Cli {
    #[command(subcommand)]
    command: Option<Verb>,

    /// Git repository to open in the terminal interface
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Disable mouse capture (all features remain keyboard-accessible)
    #[arg(long)]
    no_mouse: bool,

    /// Refresh the open pull request the moment a forwarded GitHub webhook
    /// arrives, given a port or host:port to listen on. Pair with
    /// `gh webhook forward --repo <repo> --events '*' --url http://127.0.0.1:<port>`.
    /// Only loopback connections are accepted.
    #[arg(long, value_name = "ADDRESS")]
    webhook_listen: Option<String>,

    /// Repository to run a subcommand against
    #[arg(
        short = 'C',
        long = "path",
        value_name = "DIR",
        default_value = ".",
        global = true
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
    /// Apply a commit onto the current branch
    CherryPick(RevisionArgs),
    /// Record a commit that undoes another
    Revert(RevisionArgs),
    /// Take one side of a merge conflict
    Resolve(ResolveArgs),
    /// List the GitHub repositories this checkout points at
    Repos(ReposArgs),
    /// Read a pull request, its files, its conversation and its checks
    Pr {
        #[command(subcommand)]
        command: PrVerb,
    },
    /// Print a shell completion script
    Completions(CompletionsArgs),
    /// Print the manual page, or write one page per command
    Man(ManArgs),
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    /// Shell to write a completion script for
    #[arg(value_enum)]
    shell: Shell,
}

#[derive(Debug, Args)]
struct ManArgs {
    /// Write one page per command into this directory instead of printing one
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TuiArgs {
    /// Git repository to open
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Disable mouse capture
    #[arg(long)]
    no_mouse: bool,
    /// Listen for forwarded GitHub webhooks on a port or host:port
    #[arg(long, value_name = "ADDRESS")]
    webhook_listen: Option<String>,
}

#[derive(Debug, Args)]
struct WatchableArgs {
    /// Keep the reading on screen and refresh it
    #[arg(long)]
    watch: bool,
    /// Seconds between refreshes
    #[arg(long, value_name = "SECONDS", default_value_t = 2)]
    interval: u64,
}

#[derive(Debug, Args)]
struct DiffArgs {
    /// Limit the diff to these paths
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
struct SelectionArgs {
    /// Paths to act on
    paths: Vec<PathBuf>,
    /// Act on every change instead
    #[arg(long, conflicts_with = "paths")]
    all: bool,
}

#[derive(Debug, Args)]
struct DiscardArgs {
    /// Paths whose changes are thrown away
    paths: Vec<PathBuf>,
    /// Throw away every change instead
    #[arg(long, conflicts_with = "paths")]
    all: bool,
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
    #[arg(default_value = "HEAD")]
    revision: String,
    /// Commits to skip
    #[arg(long, default_value_t = 0)]
    skip: usize,
    /// Commits to print
    #[arg(long, short = 'n', default_value_t = 30)]
    limit: usize,
}

#[derive(Debug, Args)]
struct ShowArgs {
    /// Commit to show
    #[arg(default_value = "HEAD")]
    revision: String,
    /// Print whole files instead of three lines of context
    #[arg(long)]
    expanded: bool,
}

#[derive(Debug, Args)]
struct RevisionArgs {
    /// Commit to apply
    revision: String,
}

#[derive(Debug, Args)]
struct ResolveArgs {
    /// Conflicted path
    path: PathBuf,
    /// Keep the version already on this branch
    #[arg(long, group = "side")]
    ours: bool,
    /// Keep the version being merged in
    #[arg(long, group = "side")]
    theirs: bool,
    /// Accept the file as it stands and stage it
    #[arg(long, group = "side")]
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
    Switch { name: String },
    /// Create a branch and switch to it
    Create {
        name: String,
        /// Commit to branch from
        start: Option<String>,
    },
    /// Rename a branch
    Rename { old: String, new: String },
    /// Delete a branch
    Delete {
        name: String,
        /// Confirm; without it the command reports what it would delete
        #[arg(long)]
        yes: bool,
    },
    /// Diff a branch against the current one without checking anything out
    Compare {
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
    },
    /// Apply a stash and keep it
    Apply { reference: String },
    /// Apply a stash and drop it
    Pop { reference: Option<String> },
    /// Drop a stash
    Drop {
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
        reference: String,
        /// Print whole files instead of three lines of context
        #[arg(long)]
        expanded: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PrVerb {
    /// Print a pull request's metadata and description
    View(PrArgs),
    /// List the files a pull request changes
    Files(PrArgs),
    /// Print a pull request's patch
    Diff(PrDiffArgs),
    /// Print a pull request's timeline and review comments
    Conversation(PrArgs),
    /// List a pull request's checks
    Checks(PrChecksArgs),
    /// Print one check run's steps and log
    Logs(PrLogsArgs),
    /// Open a pull request in a browser
    Open(PrArgs),
}

#[derive(Debug, Args)]
struct PrArgs {
    /// Pull-request number
    number: u64,
    /// Repository the number belongs to, as owner/name
    #[arg(long, value_name = "OWNER/NAME")]
    repo: Option<String>,
    /// Ask GitHub again instead of answering from the cache
    #[arg(long)]
    refresh: bool,
}

#[derive(Debug, Args)]
struct PrDiffArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Limit the patch to one path
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
    #[arg(long, value_name = "SECONDS", default_value_t = CHECK_WATCH_INTERVAL)]
    interval: u64,
    /// Exit 1 when a check has not passed
    #[arg(long)]
    exit_code: bool,
}

#[derive(Debug, Args)]
struct PrLogsArgs {
    #[command(flatten)]
    pull_request: PrArgs,
    /// Check run to read, by name
    check: String,
    /// Keep reading while the run is still going
    #[arg(long)]
    watch: bool,
    /// Seconds between reads while watching
    #[arg(long, value_name = "SECONDS", default_value_t = LOG_WATCH_INTERVAL)]
    interval: u64,
}

pub(crate) fn dispatch() -> Result<Launch> {
    let cli = Cli::parse();
    let out = Emitter { json: cli.json };
    let verb = match cli.command {
        None => {
            return Ok(Launch::Terminal(Box::new(TerminalOptions {
                path: cli.path,
                no_mouse: cli.no_mouse,
                webhook_listen: cli.webhook_listen,
            })));
        }
        Some(Verb::Tui(args)) => {
            return Ok(Launch::Terminal(Box::new(TerminalOptions {
                path: args.path,
                no_mouse: args.no_mouse,
                webhook_listen: args.webhook_listen,
            })));
        }
        Some(Verb::Completions(args)) => {
            return completions(&out, &args).map(Launch::Finished);
        }
        Some(Verb::Man(args)) => return manual(&out, &args).map(Launch::Finished),
        Some(other) => other,
    };
    let repository = Repository::discover(&cli.repository)?;
    let mut session = Session::new(repository);
    run(&mut session, &out, verb).map(Launch::Finished)
}

/// Write the completion script for one shell.
///
/// Generated rather than committed, so a verb or flag added to `Verb` is
/// offered by the shell the moment it exists.
fn completions(out: &Emitter, args: &CompletionsArgs) -> Result<u8> {
    let mut command = Cli::command();
    let mut script = Vec::new();
    generate(args.shell, &mut command, PROGRAM, &mut script);
    let script = String::from_utf8(script).context("the completion script was not valid UTF-8")?;
    out.emit(
        &CompletionScript {
            shell: args.shell.to_string(),
            script: &script,
        },
        || script.clone(),
    )?;
    Ok(0)
}

/// Write the manual, either as one page or as a directory of them.
fn manual(out: &Emitter, args: &ManArgs) -> Result<u8> {
    let command = Cli::command();
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

/// Render one command's manual page under the name it is invoked by.
fn render_page(command: &clap::Command, name: &str) -> Result<Vec<u8>> {
    let mut page = Vec::new();
    Man::new(command.clone().display_name(name.to_owned()))
        .title(name.to_uppercase())
        .render(&mut page)
        .with_context(|| format!("failed to render the manual page for {name}"))?;
    Ok(page)
}

/// Write a page for this command and for every command under it.
///
/// Subcommand pages are named the way `man` expects to find them, so
/// `quinjet branch create` is `quinjet-branch-create.1`.
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
struct ManualPage<'a> {
    page: &'a str,
}

#[derive(Serialize)]
struct ManualPages<'a> {
    pages: &'a [String],
}

struct Emitter {
    json: bool,
}

impl Emitter {
    fn emit<T: Serialize>(&self, value: &T, text: impl FnOnce() -> String) -> Result<()> {
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
        Verb::Completions(_) | Verb::Man(_) => Err(Failure::new(
            EXIT_FAILURE,
            "the generated references are written before a repository is opened",
        )
        .into()),
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
        Verb::CherryPick(args) => {
            let revision = revision(session, &args.revision)?;
            operate(session, out, GitOperation::CherryPick(revision))
        }
        Verb::Revert(args) => {
            let revision = revision(session, &args.revision)?;
            operate(session, out, GitOperation::Revert(revision))
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
        } => operate(
            session,
            out,
            GitOperation::StashPush {
                message,
                include_untracked,
                staged,
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
        .filter(|change| args.all || matches(&change.path, &args.paths))
        .cloned()
        .collect();
    if !args.all && args.paths.is_empty() {
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
    let operation = if args.stage {
        GitOperation::Stage(vec![args.path])
    } else if args.ours {
        GitOperation::ResolveConflict {
            path: args.path,
            choice: ConflictChoice::Ours,
        }
    } else if args.theirs {
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
            let request = lookup(session, &args)?;
            out.emit(&request, || render::pull_request(&request))?;
            Ok(0)
        }
        PrVerb::Files(args) => {
            let request = lookup(session, &args)?;
            let index = prepare(session, &request)?;
            out.emit(&index, || render::pull_request_files(&index))?;
            Ok(0)
        }
        PrVerb::Diff(args) => {
            let request = lookup(session, &args.pull_request)?;
            let document = pull_request_diff(session, &request, args.path.as_deref())?;
            out.emit(&document, || render::diff(&document))?;
            Ok(0)
        }
        PrVerb::Conversation(args) => {
            let request = lookup(session, &args)?;
            let conversation = session
                .execute(Command::PullRequestConversation {
                    pull_request: Box::new(request),
                })?
                .conversation()?;
            out.emit(&conversation, || render::conversation(&conversation))?;
            Ok(0)
        }
        PrVerb::Checks(args) => checks(session, out, &args),
        PrVerb::Logs(args) => logs(session, out, &args),
        PrVerb::Open(args) => {
            let request = lookup(session, &args)?;
            open_url(&request.url)?;
            out.message(&format!("Opened {}", request.url))?;
            Ok(0)
        }
    }
}

fn checks(session: &mut Session, out: &Emitter, args: &PrChecksArgs) -> Result<u8> {
    let request = lookup(session, &args.pull_request)?;
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
    let checks = session
        .execute(Command::PullRequestChecks {
            pull_request: Box::new(request),
            refresh: args.pull_request.refresh,
        })?
        .checks()?;
    out.emit(&checks, || render::checks(&checks.checks))?;
    Ok(if args.exit_code {
        exit_for(&checks.checks)
    } else {
        0
    })
}

fn logs(session: &mut Session, out: &Emitter, args: &PrLogsArgs) -> Result<u8> {
    let request = lookup(session, &args.pull_request)?;
    let listing = session
        .execute(Command::PullRequestChecks {
            pull_request: Box::new(request.clone()),
            refresh: args.pull_request.refresh,
        })?
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
            Ok(watch::Frame {
                text: render::check_log(&check, &log),
                finished: !check.status.is_running(),
                code: u8::from(check.status == PullRequestCheckStatus::Failed),
                value: log,
            })
        });
    }
    let log = session
        .execute(Command::CheckRunLog {
            pull_request: Box::new(request),
            check: Box::new(check.clone()),
        })?
        .check_log()?;
    if let Some(reason) = &log.unavailable {
        return Err(Failure::new(EXIT_UNAVAILABLE, reason.clone()).into());
    }
    out.emit(&log, || render::check_log(&check, &log))?;
    Ok(0)
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

fn lookup(session: &mut Session, args: &PrArgs) -> Result<PullRequest> {
    let repositories = match &args.repo {
        None => Vec::new(),
        Some(_) => {
            session
                .execute(Command::GitHubRepositories { refresh: false })?
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
    let snapshot = session
        .execute(Command::PullRequestLookup {
            repositories,
            repository: selected,
            number: args.number,
            refresh: args.refresh,
        })?
        .pull_request()?;
    for warning in &snapshot.warnings {
        note(&format!("warning: {warning}"));
    }
    Ok(snapshot.pull_request)
}

fn prepare(session: &mut Session, request: &PullRequest) -> Result<PullRequestDiffIndex> {
    session
        .execute(Command::PreparePullRequest {
            workspace: 0,
            pull_request: Box::new(request.clone()),
        })?
        .pull_request_index()
}

fn pull_request_diff(
    session: &mut Session,
    request: &PullRequest,
    path: Option<&Path>,
) -> Result<DiffDocument> {
    let index = prepare(session, request)?;
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
        for (path, document) in session
            .execute(Command::PullRequestFileBatch {
                workspace: 0,
                paths: chunk.to_vec(),
            })?
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
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    drop(
        std::process::Command::new(opener)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to hand {url} to {opener}"))?,
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
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
    fn a_repository_path_with_no_verb_opens_the_terminal_interface() {
        let cli = Cli::try_parse_from(["quinjet", "/tmp/somewhere"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.path, PathBuf::from("/tmp/somewhere"));
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
    fn a_destructive_verb_changes_nothing_until_it_is_confirmed() {
        let repository = TestRepository::new();
        fs::write(repository.path.join("README.md"), "changed\n").unwrap();
        let mut session = repository.session();
        let out = Emitter { json: true };

        discard(
            &mut session,
            &out,
            &DiscardArgs {
                paths: vec![PathBuf::from("README.md")],
                all: false,
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
}
