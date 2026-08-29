use std::path::PathBuf;

use anyhow::Result;

use crate::git::diff::{DiffDocument, DiffIndex};
use crate::git::github::{
    CheckRunLog, ContextPurpose, GitHubRepository, MergeGate, PullRequest, PullRequestAnnotations,
    PullRequestArtifacts, PullRequestCheck, PullRequestChecks, PullRequestCommits,
    PullRequestContext, PullRequestConversation, PullRequestDependencies, PullRequestDeployments,
    PullRequestDiffIndex, PullRequestFeedback, PullRequestOperation, PullRequestReviewOperation,
    PullRequestReviewSnapshot, PullRequestSecurity, PullRequestSnapshot, PullRequestStack,
    PullRequestStackSnapshot, PullRequestSuggestions, PullRequestWorkflowRuns, ReviewProgress,
    ReviewSinceRequest, StackGate, Suggestion, SuggestionPlan, WorkflowArtifact, WorkflowOperation,
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
    PullRequestGate {
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    PullRequestAnnotations {
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    PullRequestWorkflowRuns {
        pull_request: Box<PullRequest>,
        refresh: bool,
    },
    PullRequestFeedback {
        pull_request: Box<PullRequest>,
        gate: Option<Box<MergeGate>>,
        review: Box<PullRequestReviewSnapshot>,
        annotations: Option<Box<PullRequestAnnotations>>,
        viewer: String,
    },
    PullRequestSuggestions {
        pull_request: Box<PullRequest>,
        review: Box<PullRequestReviewSnapshot>,
    },
    PullRequestDependencies {
        pull_request: Box<PullRequest>,
    },
    PullRequestSecurity {
        pull_request: Box<PullRequest>,
    },
    PullRequestContext {
        pull_request: Box<PullRequest>,
        request: Box<ContextRequest>,
    },
    PlanSuggestions {
        suggestions: Vec<Suggestion>,
    },
    ApplySuggestions {
        pull_request: Box<PullRequest>,
        plan: Box<SuggestionPlan>,
        message: Option<String>,
    },
    PullRequestArtifacts {
        pull_request: Box<PullRequest>,
        runs: Box<PullRequestWorkflowRuns>,
    },
    DownloadArtifact {
        pull_request: Box<PullRequest>,
        artifact: Box<WorkflowArtifact>,
        directory: PathBuf,
    },
    PullRequestDeployments {
        pull_request: Box<PullRequest>,
        runs: Box<PullRequestWorkflowRuns>,
    },
    OperateWorkflow {
        pull_request: Box<PullRequest>,
        operation: Box<WorkflowOperation>,
    },
    PullRequestStackGate {
        stack: Box<PullRequestStack>,
        refresh: bool,
    },
    PreparePullRequest {
        workspace: u64,
        pull_request: Box<PullRequest>,
    },
    PreparePullRequestSince {
        workspace: u64,
        pull_request: Box<PullRequest>,
        since: String,
    },
    PullRequestReviewProgress {
        pull_request: Box<PullRequest>,
        index: Box<PullRequestDiffIndex>,
        since: ReviewSinceRequest,
    },
    RecordReviewVisit {
        pull_request: Box<PullRequest>,
    },
    MarkReviewFiles {
        pull_request: Box<PullRequest>,
        paths: Vec<PathBuf>,
        viewed: bool,
    },
    ForgetReviewProgress {
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
    PullRequestCommits {
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
    WarmPullRequestStackMembers {
        pull_requests: Vec<PullRequest>,
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
            Self::PullRequestGate { .. } => "Evaluating the merge gate",
            Self::PullRequestAnnotations { .. } => "Fetching check annotations",
            Self::PullRequestWorkflowRuns { .. } => "Fetching workflow runs",
            Self::PullRequestFeedback { .. } => "Collecting outstanding feedback",
            Self::PullRequestSuggestions { .. } => "Reading suggested changes",
            Self::PullRequestDependencies { .. } => "Comparing dependencies",
            Self::PullRequestSecurity { .. } => "Reading security findings",
            Self::PullRequestContext { .. } => "Assembling the context bundle",
            Self::PlanSuggestions { .. } => "Planning suggested changes",
            Self::ApplySuggestions { .. } => "Applying suggested changes",
            Self::PullRequestArtifacts { .. } => "Fetching workflow artifacts",
            Self::DownloadArtifact { .. } => "Downloading a workflow artifact",
            Self::PullRequestDeployments { .. } => "Fetching deployments",
            Self::OperateWorkflow { operation, .. } => operation.label(),
            Self::PullRequestStackGate { .. } => "Evaluating the stack merge gate",
            Self::PreparePullRequest { .. }
            | Self::PreparePullRequestSince { .. }
            | Self::PreparePullRequestStack { .. } => "Preparing pull-request diff",
            Self::PullRequestReviewProgress { .. } => "Measuring review progress",
            Self::RecordReviewVisit { .. }
            | Self::MarkReviewFiles { .. }
            | Self::ForgetReviewProgress { .. } => "Recording review progress",
            Self::PullRequestFile { .. } | Self::PullRequestFileBatch { .. } => {
                "Loading pull-request patches"
            }
            Self::PullRequestChecks { .. } => "Fetching pull-request checks",
            Self::PullRequestConversation { .. } => "Fetching pull-request conversation",
            Self::PullRequestCommits { .. } => "Fetching pull-request commits",
            Self::PullRequestReview { .. } => "Fetching pull-request review threads",
            Self::CheckRunLog { .. } => "Fetching check-run log",
            Self::WarmCheckRunLogs { .. } => "Caching check-run logs",
            Self::WarmPullRequestStackMembers { .. } => "Caching stack member details",
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
    Gate(Box<MergeGate>),
    Annotations(Box<PullRequestAnnotations>),
    WorkflowRuns(Box<PullRequestWorkflowRuns>),
    Feedback(Box<PullRequestFeedback>),
    Suggestions(Box<PullRequestSuggestions>),
    Dependencies(Box<PullRequestDependencies>),
    Security(Box<PullRequestSecurity>),
    Context(Box<PullRequestContext>),
    SuggestionPlan(Box<SuggestionPlan>),
    Artifacts(Box<PullRequestArtifacts>),
    DownloadedArtifact(PathBuf),
    Deployments(Box<PullRequestDeployments>),
    ReviewProgress(Box<ReviewProgress>),
    StackGate(Box<StackGate>),
    PullRequestIndex(Box<PullRequestDiffIndex>),
    PullRequestDiff(Box<DiffDocument>),
    PullRequestDiffBatch(Vec<(PathBuf, DiffDocument)>),
    Checks(Box<PullRequestChecks>),
    Conversation(Box<PullRequestConversation>),
    Commits(Box<PullRequestCommits>),
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
    gate, Gate -> MergeGate, |value: Box<MergeGate>| *value;
    annotations, Annotations -> PullRequestAnnotations,
        |value: Box<PullRequestAnnotations>| *value;
    workflow_runs, WorkflowRuns -> PullRequestWorkflowRuns,
        |value: Box<PullRequestWorkflowRuns>| *value;
    feedback, Feedback -> PullRequestFeedback, |value: Box<PullRequestFeedback>| *value;
    suggestions, Suggestions -> PullRequestSuggestions,
        |value: Box<PullRequestSuggestions>| *value;
    suggestion_plan, SuggestionPlan -> SuggestionPlan, |value: Box<SuggestionPlan>| *value;
    dependencies, Dependencies -> PullRequestDependencies,
        |value: Box<PullRequestDependencies>| *value;
    security, Security -> PullRequestSecurity, |value: Box<PullRequestSecurity>| *value;
    context, Context -> PullRequestContext, |value: Box<PullRequestContext>| *value;
    artifacts, Artifacts -> PullRequestArtifacts, |value: Box<PullRequestArtifacts>| *value;
    downloaded_artifact, DownloadedArtifact -> PathBuf, |value| value;
    deployments, Deployments -> PullRequestDeployments,
        |value: Box<PullRequestDeployments>| *value;
    review_progress, ReviewProgress -> ReviewProgress, |value: Box<ReviewProgress>| *value;
    stack_gate, StackGate -> StackGate, |value: Box<StackGate>| *value;
    pull_request_index, PullRequestIndex -> PullRequestDiffIndex,
        |value: Box<PullRequestDiffIndex>| *value;
    pull_request_diff, PullRequestDiff -> DiffDocument, |value: Box<DiffDocument>| *value;
    pull_request_diff_batch, PullRequestDiffBatch -> Vec<(PathBuf, DiffDocument)>, |value| value;
    checks, Checks -> PullRequestChecks, |value: Box<PullRequestChecks>| *value;
    conversation, Conversation -> PullRequestConversation,
        |value: Box<PullRequestConversation>| *value;
    commits, Commits -> PullRequestCommits, |value: Box<PullRequestCommits>| *value;
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

#[doc = " What a context bundle should contain, gathered from the command line so"]
#[doc = " the session can do the fetching it implies."]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextRequest {
    pub purpose: ContextPurpose,
    pub budget: usize,
    pub path: Option<PathBuf>,
}
