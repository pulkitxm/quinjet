use std::path::PathBuf;

use anyhow::Result;

use crate::git::diff::{DiffDocument, DiffIndex};
use crate::git::github::{
    CheckRunLog, GitHubRepository, PullRequest, PullRequestCheck, PullRequestChecks,
    PullRequestConversation, PullRequestDiffIndex, PullRequestOperation,
    PullRequestReviewOperation, PullRequestReviewSnapshot, PullRequestSnapshot, PullRequestStack,
    PullRequestStackSnapshot,
};
use crate::git::history::Commit;
use crate::git::status::RepoStatus;
use crate::git::{
    Branch, GitOperation, HistoryBranch, LocalDiffRequest, ProjectGroup, Stash, Worktree,
};

#[derive(Debug)]
pub(crate) enum Command {
    Status,
    History {
        revision: String,
        skip: usize,
        limit: usize,
    },
    Branches,
    HistoryBranches,
    Stashes,
    Worktrees,
    RecentProjects,
    PrepareLocalDiff {
        workspace: u64,
        request: Box<LocalDiffRequest>,
    },
    LocalDiffFile {
        workspace: u64,
        path: PathBuf,
    },
    GitHubRepositories {
        refresh: bool,
    },
    LocalGitHubRepository,
    PullRequestLookup {
        repositories: Vec<GitHubRepository>,
        repository: Option<Box<GitHubRepository>>,
        number: u64,
        refresh: bool,
    },
    PullRequestStack {
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    PreparePullRequest {
        workspace: u64,
        pull_request: Box<PullRequest>,
    },
    PreparePullRequestStack {
        workspace: u64,
        stack: Box<PullRequestStack>,
        from: usize,
        to: usize,
    },
    PullRequestFile {
        workspace: u64,
        path: PathBuf,
    },
    PullRequestFileBatch {
        workspace: u64,
        paths: Vec<PathBuf>,
    },
    PullRequestChecks {
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    PullRequestConversation {
        pull_request: Box<PullRequest>,
    },
    PullRequestReview {
        pull_request: Box<PullRequest>,
    },
    CheckRunLog {
        pull_request: Box<PullRequest>,
        check: Box<PullRequestCheck>,
    },
    WarmCheckRunLogs {
        pull_request: Box<PullRequest>,
        checks: Vec<PullRequestCheck>,
    },
    Operate(GitOperation),
    OperatePullRequest {
        pull_request: Box<PullRequest>,
        operation: PullRequestOperation,
    },
    OperatePullRequestReview {
        pull_request: Box<PullRequest>,
        operation: PullRequestReviewOperation,
    },
}

impl Command {
    pub(crate) const fn progress_label(&self) -> &'static str {
        match self {
            Self::Status => "Reading repository status",
            Self::History { .. } => "Reading commit history",
            Self::Branches | Self::HistoryBranches => "Reading branches",
            Self::Stashes => "Reading stashes",
            Self::Worktrees | Self::RecentProjects => "Reading worktrees",
            Self::PrepareLocalDiff { .. } => "Preparing local diff",
            Self::LocalDiffFile { .. } => "Loading file patch",
            Self::GitHubRepositories { .. } => "Discovering GitHub repositories",
            Self::LocalGitHubRepository => "Reading repository link",
            Self::PullRequestLookup { .. } => "Fetching pull-request metadata",
            Self::PullRequestStack { .. } => "Fetching pull-request stack",
            Self::PreparePullRequest { .. } | Self::PreparePullRequestStack { .. } => {
                "Preparing pull-request diff"
            }
            Self::PullRequestFile { .. } | Self::PullRequestFileBatch { .. } => {
                "Loading pull-request patches"
            }
            Self::PullRequestChecks { .. } => "Fetching pull-request checks",
            Self::PullRequestConversation { .. } => "Fetching pull-request conversation",
            Self::PullRequestReview { .. } => "Fetching pull-request review threads",
            Self::CheckRunLog { .. } => "Fetching check-run log",
            Self::WarmCheckRunLogs { .. } => "Caching check-run logs",
            Self::Operate(operation) => operation.label(),
            Self::OperatePullRequest { operation, .. } => operation.label(),
            Self::OperatePullRequestReview { operation, .. } => operation.label(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Status(Box<RepoStatus>),
    History(Vec<Commit>),
    Branches(Vec<Branch>),
    HistoryBranches(Vec<HistoryBranch>),
    Stashes(Vec<Stash>),
    Worktrees(Vec<Worktree>),
    RecentProjects(Vec<ProjectGroup>),
    LocalDiffIndex(Box<DiffIndex>),
    LocalDiffFile {
        path: PathBuf,
        document: Box<DiffDocument>,
    },
    GitHubRepositories {
        repositories: Vec<GitHubRepository>,
        warnings: Vec<String>,
    },
    LocalGitHubRepository(Option<Box<GitHubRepository>>),
    PullRequest(Box<PullRequestSnapshot>),
    PullRequestStack(Box<PullRequestStackSnapshot>),
    PullRequestIndex(Box<PullRequestDiffIndex>),
    PullRequestDiff(Box<DiffDocument>),
    PullRequestDiffBatch(Vec<(PathBuf, DiffDocument)>),
    Checks(Box<PullRequestChecks>),
    Conversation(Box<PullRequestConversation>),
    Review(Box<PullRequestReviewSnapshot>),
    CheckLog(Box<CheckRunLog>),
    Warmed,
    Operation {
        label: String,
        changes_history: bool,
        message: String,
    },
}

macro_rules! answers {
    ($($method:ident, $variant:ident -> $answer:ty, $unwrap:expr;)*) => {
        impl Outcome {
            $(
                pub(crate) fn $method(self) -> Result<$answer> {
                    match self {
                        Self::$variant(value) => Ok(($unwrap)(value)),
                        other => Err(unexpected(&other, stringify!($variant))),
                    }
                }
            )*
        }
    };
}

answers! {
    status, Status -> RepoStatus, |value: Box<RepoStatus>| *value;
    history, History -> Vec<Commit>, |value| value;
    branches, Branches -> Vec<Branch>, |value| value;
    history_branches, HistoryBranches -> Vec<HistoryBranch>, |value| value;
    stashes, Stashes -> Vec<Stash>, |value| value;
    worktrees, Worktrees -> Vec<Worktree>, |value| value;
    recent_projects, RecentProjects -> Vec<ProjectGroup>, |value| value;
    local_diff_index, LocalDiffIndex -> DiffIndex, |value: Box<DiffIndex>| *value;
    pull_request, PullRequest -> PullRequestSnapshot, |value: Box<PullRequestSnapshot>| *value;
    pull_request_stack, PullRequestStack -> PullRequestStackSnapshot,
        |value: Box<PullRequestStackSnapshot>| *value;
    pull_request_index, PullRequestIndex -> PullRequestDiffIndex,
        |value: Box<PullRequestDiffIndex>| *value;
    pull_request_diff, PullRequestDiff -> DiffDocument, |value: Box<DiffDocument>| *value;
    pull_request_diff_batch, PullRequestDiffBatch -> Vec<(PathBuf, DiffDocument)>, |value| value;
    checks, Checks -> PullRequestChecks, |value: Box<PullRequestChecks>| *value;
    conversation, Conversation -> PullRequestConversation,
        |value: Box<PullRequestConversation>| *value;
    review, Review -> PullRequestReviewSnapshot,
        |value: Box<PullRequestReviewSnapshot>| *value;
    check_log, CheckLog -> CheckRunLog, |value: Box<CheckRunLog>| *value;
    local_github_repository, LocalGitHubRepository -> Option<GitHubRepository>,
        |value: Option<Box<GitHubRepository>>| value.map(|repository| *repository);
}

impl Outcome {
    pub(crate) fn local_diff_file(self) -> Result<(PathBuf, DiffDocument)> {
        match self {
            Self::LocalDiffFile { path, document } => Ok((path, *document)),
            other => Err(unexpected(&other, "LocalDiffFile")),
        }
    }

    pub(crate) fn github_repositories(self) -> Result<(Vec<GitHubRepository>, Vec<String>)> {
        match self {
            Self::GitHubRepositories {
                repositories,
                warnings,
            } => Ok((repositories, warnings)),
            other => Err(unexpected(&other, "GitHubRepositories")),
        }
    }

    pub(crate) fn operation(self) -> Result<(String, bool, String)> {
        match self {
            Self::Operation {
                label,
                changes_history,
                message,
            } => Ok((label, changes_history, message)),
            other => Err(unexpected(&other, "Operation")),
        }
    }
}

fn unexpected(outcome: &Outcome, wanted: &str) -> anyhow::Error {
    anyhow::anyhow!("Expected a {wanted} answer but the command layer returned {outcome:?}")
}
