use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::diff::DiffDocument;
use super::github::{
    CheckRunLog, PullRequest, PullRequestCheck, PullRequestChecks, PullRequestCommits,
    PullRequestConversation, PullRequestDiffIndex, PullRequestOperation, PullRequestProgress,
    PullRequestReviewOperation, PullRequestReviewSnapshot, PullRequestSnapshot, PullRequestStack,
    PullRequestStackMemberIdentity, PullRequestStackSnapshot,
};
use super::history::Commit;
use super::status::RepoStatus;
use super::{
    Branch, GitOperation, HistoryBranch, LocalDiffRequest, ProjectGroup, Repository, Stash,
    Worktree,
};
use crate::cli::{Command, Outcome, Session};
use crate::search::{SearchHits, SearchRequest, SearchTarget};

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
    LoadPullRequestStackMember {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    LoadPullRequestStackMemberChecks {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    LoadPullRequestStackTipChecks {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    LoadPullRequestStackMemberConversation {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    LoadPullRequestCommits {
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    LoadPullRequestStackMemberCommits {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        pull_request: Box<PullRequest>,
    },
    PreparePullRequest {
        commit: Option<String>,
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
        commit: Option<String>,
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
    PrefetchPullRequestStackMembers {
        generation: u64,
        pull_requests: Vec<PullRequest>,
    },
    LoadAccounts {
        generation: u64,
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
    Search {
        generation: u64,
        request: Box<SearchRequest>,
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
    PullRequestStackMember {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        result: Result<PullRequestSnapshot, String>,
    },
    PullRequestStackMemberChecks {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        result: Result<PullRequestChecks, String>,
    },
    PullRequestStackTipChecks {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        result: Result<PullRequestChecks, String>,
    },
    PullRequestStackMemberConversation {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        result: Result<PullRequestConversation, String>,
    },
    PullRequestStackMemberCommits {
        identity: PullRequestStackMemberIdentity,
        generation: u64,
        result: Result<PullRequestCommits, String>,
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
    PullRequestCommits {
        generation: u64,
        result: Result<PullRequestCommits, String>,
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
    Accounts {
        generation: u64,
        result: Result<super::accounts::Accounts, String>,
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
    Search {
        generation: u64,
        result: Result<SearchHits, String>,
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
    stack_member: Option<WorkerCommand>,
    stack_member_checks: Option<WorkerCommand>,
    stack_tip_checks: Option<WorkerCommand>,
    stack_member_conversation: Option<WorkerCommand>,
    stack_member_commits: Option<WorkerCommand>,
    review: Option<WorkerCommand>,
    check_log: Option<WorkerCommand>,
    warm: Option<WorkerCommand>,
    stack_warm: Option<WorkerCommand>,
    search: Option<WorkerCommand>,
    shutdown: bool,
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
    StackWarm,
}

const fn worker_lane(command: &WorkerCommand) -> WorkerLane {
    match command {
        WorkerCommand::PrepareLocalDiff { .. } | WorkerCommand::LoadLocalDiffFile { .. } => {
            WorkerLane::LocalPreview
        }
        WorkerCommand::LoadGitHubRepositories { .. }
        | WorkerCommand::LookupPullRequest { .. }
        | WorkerCommand::LoadPullRequestStack { .. }
        | WorkerCommand::LoadPullRequestStackMember { .. }
        | WorkerCommand::LoadPullRequestChecks { .. }
        | WorkerCommand::LoadPullRequestStackMemberChecks { .. }
        | WorkerCommand::LoadPullRequestStackTipChecks { .. }
        | WorkerCommand::LoadCheckRunLog { .. } => WorkerLane::GitHubMetadata,
        WorkerCommand::LoadPullRequestConversation { .. }
        | WorkerCommand::LoadPullRequestStackMemberConversation { .. }
        | WorkerCommand::LoadPullRequestStackMemberCommits { .. }
        | WorkerCommand::LoadPullRequestCommits { .. } => WorkerLane::Conversation,
        WorkerCommand::LoadPullRequestReview { .. }
        | WorkerCommand::OperatePullRequestReview { .. } => WorkerLane::Review,
        WorkerCommand::PrefetchCheckRunLogs { .. } => WorkerLane::Warm,
        WorkerCommand::PrefetchPullRequestStackMembers { .. } => WorkerLane::StackWarm,
        WorkerCommand::PreparePullRequest { .. }
        | WorkerCommand::PreparePullRequestStack { .. }
        | WorkerCommand::LoadPullRequestFile { .. }
        | WorkerCommand::LoadPullRequestFileBatch { .. } => WorkerLane::PullRequestPreview,
        WorkerCommand::Search { request, .. } => match request.target {
            SearchTarget::PullRequest { .. } => WorkerLane::PullRequestPreview,
            SearchTarget::Changes { .. } | SearchTarget::History { .. } => WorkerLane::Background,
        },
        _ => WorkerLane::Background,
    }
}

mod lifecycle;
mod mailbox;
mod runtime;
mod stack_runtime;

pub(crate) use lifecycle::GitWorker;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use runtime::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use stack_runtime::*;

#[cfg(test)]
mod stack_tests;
#[cfg(test)]
mod tests;
