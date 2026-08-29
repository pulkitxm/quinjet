#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Subcommand)]
pub(super) enum WorkVerb {
    #[doc = " Start a session against a pull request"]
    Start(WorkStartArgs),
    #[doc = " List the recorded work sessions"]
    List,
    #[doc = " Print one session's tasks, checkpoints and boundaries"]
    Inspect(WorkIdArgs),
    #[doc = " Print what a session has changed since it started"]
    Diff(WorkIdArgs),
    #[doc = " Run a verification command inside a session's worktree"]
    Verify(WorkVerifyArgs),
    #[doc = " Record a session's work as one commit on its own branch"]
    Publish(WorkPublishArgs),
    #[doc = " Remove a session's worktree and forget it"]
    Abort(WorkAbortArgs),
}

#[doc = " Where a session's task list comes from, as the command line spells it."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum WorkFrom {
    #[doc = " The unresolved threads and requested changes"]
    Feedback,
    #[doc = " The failing checks and their annotations"]
    FailedChecks,
    #[doc = " The change itself, with no task list"]
    Whole,
}

impl WorkFrom {
    pub(super) const fn source(self) -> WorkSource {
        match self {
            Self::Feedback => WorkSource::Feedback,
            Self::FailedChecks => WorkSource::FailedChecks,
            Self::Whole => WorkSource::Whole,
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct WorkStartArgs {
    #[doc = " The pull request to work on"]
    #[arg(long = "pr", value_name = "NUMBER")]
    pub(super) number: u64,
    #[doc = " What the session's task list is drawn from"]
    #[arg(long = "from", value_name = "SOURCE", default_value = "feedback")]
    pub(super) from: WorkFrom,
    #[doc = " Give the session its own checkout at the pull request's head"]
    #[arg(long)]
    pub(super) worktree: bool,
    #[doc = " Where the checkout goes; implies --worktree"]
    #[arg(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub(super) into: Option<PathBuf>,
    #[doc = " Chooses which discovered repository the number belongs to"]
    #[arg(long, value_name = "OWNER/NAME")]
    pub(super) repo: Option<String>,
    #[doc = " Skips the pull-request metadata cache for this read"]
    #[arg(long)]
    pub(super) refresh: bool,
}

#[derive(Debug, Args)]
pub(super) struct WorkIdArgs {
    #[doc = " The session identifier"]
    #[arg(value_name = "SESSION", value_hint = ValueHint::Other)]
    pub(super) id: String,
}

#[derive(Debug, Args)]
pub(super) struct WorkVerifyArgs {
    #[doc = " The session identifier"]
    #[arg(value_name = "SESSION", value_hint = ValueHint::Other)]
    pub(super) id: String,
    #[doc = " The command to run, after `--`. Without one, the commands already recorded are re-run"]
    #[arg(value_name = "COMMAND", last = true, value_hint = ValueHint::CommandWithArguments)]
    pub(super) command: Vec<String>,
    #[doc = " Exit 1 when any recorded verification has failed"]
    #[arg(long)]
    pub(super) exit_code: bool,
}

#[derive(Debug, Args)]
pub(super) struct WorkPublishArgs {
    #[doc = " The session identifier"]
    #[arg(value_name = "SESSION", value_hint = ValueHint::Other)]
    pub(super) id: String,
    #[doc = " The commit message; one is derived from the session when omitted"]
    #[arg(long, short = 'm', value_name = "MESSAGE")]
    pub(super) message: Option<String>,
    #[doc = " Confirm; without it the command reports what it would commit"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
pub(super) struct WorkAbortArgs {
    #[doc = " The session identifier"]
    #[arg(value_name = "SESSION", value_hint = ValueHint::Other)]
    pub(super) id: String,
    #[doc = " Confirm; without it the command reports what it would remove"]
    #[arg(long)]
    pub(super) yes: bool,
}
