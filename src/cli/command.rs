use std::path::PathBuf;

use anyhow::Result;

use crate::git::diff::{DiffDocument, DiffIndex};
use crate::git::github::{
    CheckRunLog, GitHubRepository, PreparedPullRequest, PullRequest, PullRequestCheck,
    PullRequestChecks, PullRequestConversation, PullRequestDiffIndex, PullRequestProgress,
    PullRequestSnapshot,
};
use crate::git::history::Commit;
use crate::git::status::RepoStatus;
use crate::git::{
    Branch, GitOperation, HistoryBranch, LocalDiffRequest, PreparedLocalDiff, Repository, Stash,
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
    PullRequestLookup {
        repositories: Vec<GitHubRepository>,
        repository: Option<Box<GitHubRepository>>,
        number: u64,
        refresh: bool,
    },
    PreparePullRequest {
        workspace: u64,
        pull_request: Box<PullRequest>,
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
    CheckRunLog {
        pull_request: Box<PullRequest>,
        check: Box<PullRequestCheck>,
    },
    WarmCheckRunLogs {
        pull_request: Box<PullRequest>,
        checks: Vec<PullRequestCheck>,
    },
    Operate(GitOperation),
}

impl Command {
    pub(crate) const fn progress_label(&self) -> &'static str {
        match self {
            Self::Status => "Reading repository status",
            Self::History { .. } => "Reading commit history",
            Self::Branches | Self::HistoryBranches => "Reading branches",
            Self::Stashes => "Reading stashes",
            Self::PrepareLocalDiff { .. } => "Preparing local diff",
            Self::LocalDiffFile { .. } => "Loading file patch",
            Self::GitHubRepositories { .. } => "Discovering GitHub repositories",
            Self::PullRequestLookup { .. } => "Fetching pull-request metadata",
            Self::PreparePullRequest { .. } => "Preparing pull-request diff",
            Self::PullRequestFile { .. } | Self::PullRequestFileBatch { .. } => {
                "Loading pull-request patches"
            }
            Self::PullRequestChecks { .. } => "Fetching pull-request checks",
            Self::PullRequestConversation { .. } => "Fetching pull-request conversation",
            Self::CheckRunLog { .. } => "Fetching check-run log",
            Self::WarmCheckRunLogs { .. } => "Caching check-run logs",
            Self::Operate(operation) => operation.label(),
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
    LocalDiffIndex(Box<DiffIndex>),
    LocalDiffFile {
        path: PathBuf,
        document: Box<DiffDocument>,
    },
    GitHubRepositories {
        repositories: Vec<GitHubRepository>,
        warnings: Vec<String>,
    },
    PullRequest(Box<PullRequestSnapshot>),
    PullRequestIndex(Box<PullRequestDiffIndex>),
    PullRequestDiff(Box<DiffDocument>),
    PullRequestDiffBatch(Vec<(PathBuf, DiffDocument)>),
    Checks(Box<PullRequestChecks>),
    Conversation(Box<PullRequestConversation>),
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
    local_diff_index, LocalDiffIndex -> DiffIndex, |value: Box<DiffIndex>| *value;
    pull_request, PullRequest -> PullRequestSnapshot, |value: Box<PullRequestSnapshot>| *value;
    pull_request_index, PullRequestIndex -> PullRequestDiffIndex,
        |value: Box<PullRequestDiffIndex>| *value;
    pull_request_diff, PullRequestDiff -> DiffDocument, |value: Box<DiffDocument>| *value;
    pull_request_diff_batch, PullRequestDiffBatch -> Vec<(PathBuf, DiffDocument)>, |value| value;
    checks, Checks -> PullRequestChecks, |value: Box<PullRequestChecks>| *value;
    conversation, Conversation -> PullRequestConversation,
        |value: Box<PullRequestConversation>| *value;
    check_log, CheckLog -> CheckRunLog, |value: Box<CheckRunLog>| *value;
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

pub(crate) struct Session {
    repository: Repository,
    local_diff: Option<(u64, PreparedLocalDiff)>,
    pull_request_diff: Option<(u64, PreparedPullRequest)>,
}

impl Session {
    pub(crate) const fn new(repository: Repository) -> Self {
        Self {
            repository,
            local_diff: None,
            pull_request_diff: None,
        }
    }

    pub(crate) fn execute(&mut self, command: Command) -> Result<Outcome> {
        self.execute_with(command, &mut |_| {}, &|| true)
    }

    pub(crate) fn repository_revision(&self, revision: &str) -> Result<String> {
        self.repository.resolve_revision(revision)
    }

    pub(crate) fn execute_with(
        &mut self,
        command: Command,
        progress: &mut dyn FnMut(PullRequestProgress),
        wanted: &dyn Fn() -> bool,
    ) -> Result<Outcome> {
        match command {
            Command::Status => Ok(Outcome::Status(Box::new(self.repository.status()?))),
            Command::History {
                revision,
                skip,
                limit,
            } => Ok(Outcome::History(
                self.repository.history(&revision, skip, limit)?,
            )),
            Command::Branches => Ok(Outcome::Branches(self.repository.branches()?)),
            Command::HistoryBranches => Ok(Outcome::HistoryBranches(
                self.repository.history_branches()?,
            )),
            Command::Stashes => Ok(Outcome::Stashes(self.repository.stashes()?)),
            Command::PrepareLocalDiff { workspace, request } => {
                let prepared = self.repository.prepare_local_diff(&request)?;
                let index = prepared.index();
                self.local_diff = Some((workspace, prepared));
                Ok(Outcome::LocalDiffIndex(Box::new(index)))
            }
            Command::LocalDiffFile { workspace, path } => {
                let document = self.local_workspace(workspace)?.diff_file(&path)?;
                Ok(Outcome::LocalDiffFile {
                    path,
                    document: Box::new(document),
                })
            }
            Command::GitHubRepositories { refresh } => {
                let (repositories, warnings) = self.repository.github_repositories(refresh)?;
                Ok(Outcome::GitHubRepositories {
                    repositories,
                    warnings,
                })
            }
            Command::PullRequestLookup {
                repositories,
                repository,
                number,
                refresh,
            } => {
                progress(PullRequestProgress::LoadingMetadata);
                Ok(Outcome::PullRequest(Box::new(
                    self.repository.pull_request_lookup(
                        &repositories,
                        repository.as_deref(),
                        number,
                        refresh,
                    )?,
                )))
            }
            Command::PreparePullRequest {
                workspace,
                pull_request,
            } => {
                let prepared = self
                    .repository
                    .prepare_pull_request_diff(&pull_request, progress)?;
                let index = prepared.index();
                self.pull_request_diff = Some((workspace, prepared));
                Ok(Outcome::PullRequestIndex(Box::new(index)))
            }
            Command::PullRequestFile { workspace, path } => Ok(Outcome::PullRequestDiff(Box::new(
                self.pull_request_workspace(workspace)?.diff_file(&path)?,
            ))),
            Command::PullRequestFileBatch { workspace, paths } => {
                Ok(Outcome::PullRequestDiffBatch(
                    self.pull_request_workspace(workspace)?.diff_files(&paths)?,
                ))
            }
            Command::PullRequestChecks {
                pull_request,
                refresh,
            } => Ok(Outcome::Checks(Box::new(
                self.repository
                    .pull_request_checks(&pull_request, refresh)?,
            ))),
            Command::PullRequestConversation { pull_request } => Ok(Outcome::Conversation(
                Box::new(self.repository.pull_request_conversation(&pull_request)?),
            )),
            Command::CheckRunLog {
                pull_request,
                check,
            } => Ok(Outcome::CheckLog(Box::new(
                self.repository
                    .pull_request_check_log(&pull_request, &check)?,
            ))),
            Command::WarmCheckRunLogs {
                pull_request,
                checks,
            } => {
                let _warmed =
                    self.repository
                        .prefetch_check_run_logs(&pull_request, &checks, wanted);
                Ok(Outcome::Warmed)
            }
            Command::Operate(operation) => {
                let label = operation.label().to_owned();
                let changes_history = operation.changes_history();
                let message = self.repository.perform(&operation)?;
                Ok(Outcome::Operation {
                    label,
                    changes_history,
                    message,
                })
            }
        }
    }

    fn local_workspace(&self, workspace: u64) -> Result<&PreparedLocalDiff> {
        self.local_diff
            .as_ref()
            .filter(|(prepared, _)| *prepared == workspace)
            .map(|(_, prepared)| prepared)
            .ok_or_else(|| anyhow::anyhow!("Local diff workspace is no longer available"))
    }

    fn pull_request_workspace(&self, workspace: u64) -> Result<&PreparedPullRequest> {
        self.pull_request_diff
            .as_ref()
            .filter(|(prepared, _)| *prepared == workspace)
            .map(|(_, prepared)| prepared)
            .ok_or_else(|| anyhow::anyhow!("Pull-request diff workspace is no longer available"))
    }
}
