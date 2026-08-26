use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::diff::DiffDocument;
use super::github::{
    CheckRunLog, PullRequest, PullRequestCheck, PullRequestChecks, PullRequestConversation,
    PullRequestDiffIndex, PullRequestOperation, PullRequestProgress, PullRequestReviewOperation,
    PullRequestReviewSnapshot, PullRequestSnapshot, PullRequestStack, PullRequestStackSnapshot,
};
use super::history::Commit;
use super::status::RepoStatus;
use super::{
    Branch, GitOperation, HistoryBranch, LocalDiffRequest, ProjectGroup, Repository, Stash,
    Worktree,
};
use crate::cli::{Command, Outcome, Session};

#[derive(Debug)]
pub(crate) enum WorkerCommand {
    Refresh {
        generation: u64,
    },
    PrepareLocalDiff {
        generation: u64,
        request: Box<LocalDiffRequest>,
    },
    LoadLocalDiffFile {
        generation: u64,
        workspace_generation: u64,
        path: PathBuf,
    },
    LoadHistory {
        generation: u64,
        revision: String,
        skip: usize,
        limit: usize,
    },
    LoadGitHubRepositories {
        generation: u64,
        refresh: bool,
    },
    LoadLocalGitHubRepository,
    LookupPullRequest {
        generation: u64,
        repositories: Vec<super::github::GitHubRepository>,
        repository: Option<Box<super::github::GitHubRepository>>,
        number: u64,
        refresh: bool,
    },
    LoadPullRequestStack {
        generation: u64,
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    PreparePullRequest {
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    PreparePullRequestStack {
        generation: u64,
        stack: Box<PullRequestStack>,
        from: usize,
        to: usize,
    },
    LoadPullRequestFile {
        generation: u64,
        workspace_generation: u64,
        path: PathBuf,
    },
    #[doc = " Background fill for the rest of a prepared pull request. It carries no"]
    #[doc = " preview generation because it never replaces what the reader is looking"]
    #[doc = " at; the workspace it was prepared against is the only thing that can"]
    #[doc = " make its results stale."]
    LoadPullRequestFileBatch {
        workspace_generation: u64,
        paths: Vec<PathBuf>,
    },
    LoadPullRequestChecks {
        generation: u64,
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    LoadPullRequestConversation {
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    LoadPullRequestReview {
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    OperatePullRequestReview {
        generation: u64,
        pull_request: Box<PullRequest>,
        operation: PullRequestReviewOperation,
    },
    LoadCheckRunLog {
        generation: u64,
        pull_request: Box<PullRequest>,
        check: Box<PullRequestCheck>,
    },
    #[doc = " Warm every finished run's log so that opening any of them is instant."]
    #[doc = " It carries no generation because it changes nothing on screen."]
    PrefetchCheckRunLogs {
        generation: u64,
        pull_request: Box<PullRequest>,
        checks: Vec<PullRequestCheck>,
    },
    LoadBranches {
        generation: u64,
    },
    LoadHistoryBranches {
        generation: u64,
    },
    LoadStashes {
        generation: u64,
    },
    LoadWorktrees {
        generation: u64,
    },
    LoadRecentProjects {
        generation: u64,
    },
    Operate {
        id: u64,
        operation: GitOperation,
    },
    OperatePullRequest {
        id: u64,
        pull_request: Box<PullRequest>,
        operation: PullRequestOperation,
    },
    Shutdown,
}

#[derive(Debug)]
pub(crate) enum WorkerEvent {
    Status {
        generation: u64,
        result: Result<RepoStatus, String>,
    },
    LocalDiffIndex {
        generation: u64,
        result: Result<super::diff::DiffIndex, String>,
    },
    LocalDiffFile {
        generation: u64,
        workspace_generation: u64,
        path: PathBuf,
        result: Result<DiffDocument, String>,
    },
    History {
        generation: u64,
        skip: usize,
        result: Result<Vec<Commit>, String>,
    },
    GitHubRepositories {
        generation: u64,
        result: Result<(Vec<super::github::GitHubRepository>, Vec<String>), String>,
    },
    LocalGitHubRepository {
        result: Result<Option<super::github::GitHubRepository>, String>,
    },
    PullRequestLookup {
        generation: u64,
        result: Result<PullRequestSnapshot, String>,
    },
    PullRequestStack {
        generation: u64,
        result: Result<PullRequestStackSnapshot, String>,
    },
    PullRequestProgress {
        generation: u64,
        diff: bool,
        progress: PullRequestProgress,
    },
    PullRequestIndex {
        generation: u64,
        result: Result<PullRequestDiffIndex, String>,
    },
    PullRequestDiff {
        generation: u64,
        result: Result<DiffDocument, String>,
    },
    PullRequestDiffBatch {
        workspace_generation: u64,
        result: Result<Vec<(PathBuf, DiffDocument)>, String>,
    },
    PullRequestChecks {
        generation: u64,
        result: Result<PullRequestChecks, String>,
    },
    PullRequestConversation {
        generation: u64,
        result: Result<PullRequestConversation, String>,
    },
    PullRequestReview {
        generation: u64,
        result: Result<PullRequestReviewSnapshot, String>,
    },
    CheckRunLog {
        generation: u64,
        result: Result<CheckRunLog, String>,
    },
    Branches {
        generation: u64,
        result: Result<Vec<Branch>, String>,
    },
    HistoryBranches {
        generation: u64,
        result: Result<Vec<HistoryBranch>, String>,
    },
    Stashes {
        generation: u64,
        result: Result<Vec<Stash>, String>,
    },
    Worktrees {
        generation: u64,
        result: Result<Vec<Worktree>, String>,
    },
    RecentProjects {
        generation: u64,
        result: Result<Vec<ProjectGroup>, String>,
    },
    OperationFinished {
        id: u64,
        label: String,
        changes_history: bool,
        refresh_pull_request: bool,
        result: Result<String, String>,
    },
}

#[derive(Default)]
struct Mailbox {
    operations: VecDeque<WorkerCommand>,
    branches: Option<WorkerCommand>,
    projects: Option<WorkerCommand>,
    refresh: Option<WorkerCommand>,
    preview: Option<WorkerCommand>,
    history: Option<WorkerCommand>,
    pull_request: Option<WorkerCommand>,
    repositories: Option<WorkerCommand>,
    prefetch: Option<WorkerCommand>,
    checks: Option<WorkerCommand>,
    conversation: Option<WorkerCommand>,
    review: Option<WorkerCommand>,
    check_log: Option<WorkerCommand>,
    warm: Option<WorkerCommand>,
    shutdown: bool,
}

impl Mailbox {
    fn push(&mut self, command: WorkerCommand) {
        match command {
            command @ (WorkerCommand::Operate { .. }
            | WorkerCommand::OperatePullRequest { .. }
            | WorkerCommand::OperatePullRequestReview { .. }) => {
                self.operations.push_back(command);
            }
            command @ (WorkerCommand::LoadBranches { .. }
            | WorkerCommand::LoadHistoryBranches { .. }
            | WorkerCommand::LoadStashes { .. }) => self.branches = Some(command),
            command @ (WorkerCommand::LoadWorktrees { .. }
            | WorkerCommand::LoadRecentProjects { .. }) => self.projects = Some(command),
            command @ WorkerCommand::Refresh { .. } => self.refresh = Some(command),
            command @ (WorkerCommand::PrepareLocalDiff { .. }
            | WorkerCommand::LoadLocalDiffFile { .. }
            | WorkerCommand::PreparePullRequest { .. }
            | WorkerCommand::PreparePullRequestStack { .. }
            | WorkerCommand::LoadPullRequestFile { .. }) => {
                self.preview = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestFileBatch { .. } => {
                self.prefetch = Some(command);
            }
            command @ WorkerCommand::LoadHistory { .. } => self.history = Some(command),
            command @ (WorkerCommand::LoadGitHubRepositories { .. }
            | WorkerCommand::LoadLocalGitHubRepository) => {
                self.repositories = Some(command);
            }
            command @ (WorkerCommand::LookupPullRequest { .. }
            | WorkerCommand::LoadPullRequestStack { .. }) => {
                self.pull_request = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestChecks { .. } => self.checks = Some(command),
            command @ WorkerCommand::LoadPullRequestConversation { .. } => {
                self.conversation = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestReview { .. } => self.review = Some(command),
            command @ WorkerCommand::LoadCheckRunLog { .. } => self.check_log = Some(command),
            command @ WorkerCommand::PrefetchCheckRunLogs { .. } => {
                self.warm = Some(command);
            }
            WorkerCommand::Shutdown => self.shutdown = true,
        }
    }

    fn pop(&mut self) -> Option<WorkerCommand> {
        self.operations
            .pop_front()
            .or_else(|| self.branches.take())
            .or_else(|| self.projects.take())
            .or_else(|| self.preview.take())
            .or_else(|| self.repositories.take())
            .or_else(|| self.pull_request.take())
            .or_else(|| self.refresh.take())
            .or_else(|| self.check_log.take())
            .or_else(|| self.checks.take())
            .or_else(|| self.conversation.take())
            .or_else(|| self.review.take())
            .or_else(|| self.history.take())
            .or_else(|| self.prefetch.take())
            .or_else(|| self.warm.take())
    }
}

struct SharedMailbox {
    state: Mutex<Mailbox>,
    ready: Condvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerLane {
    Background,
    GitHubMetadata,
    Conversation,
    LocalPreview,
    PullRequestPreview,
    Review,
    Warm,
}

const fn worker_lane(command: &WorkerCommand) -> WorkerLane {
    match command {
        WorkerCommand::PrepareLocalDiff { .. } | WorkerCommand::LoadLocalDiffFile { .. } => {
            WorkerLane::LocalPreview
        }
        WorkerCommand::LoadGitHubRepositories { .. }
        | WorkerCommand::LookupPullRequest { .. }
        | WorkerCommand::LoadPullRequestStack { .. }
        | WorkerCommand::LoadPullRequestChecks { .. }
        | WorkerCommand::LoadCheckRunLog { .. } => WorkerLane::GitHubMetadata,
        WorkerCommand::LoadPullRequestConversation { .. } => WorkerLane::Conversation,
        WorkerCommand::LoadPullRequestReview { .. }
        | WorkerCommand::OperatePullRequestReview { .. } => WorkerLane::Review,
        WorkerCommand::PrefetchCheckRunLogs { .. } => WorkerLane::Warm,
        WorkerCommand::PreparePullRequest { .. }
        | WorkerCommand::PreparePullRequestStack { .. }
        | WorkerCommand::LoadPullRequestFile { .. }
        | WorkerCommand::LoadPullRequestFileBatch { .. } => WorkerLane::PullRequestPreview,
        _ => WorkerLane::Background,
    }
}

mod lifecycle;
mod runtime;

pub(crate) use lifecycle::GitWorker;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use runtime::*;

#[cfg(test)]
mod tests;
