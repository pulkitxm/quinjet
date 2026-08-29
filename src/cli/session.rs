use std::path::PathBuf;

use anyhow::{Result, bail};

mod actions;
mod context;
mod feedback;
mod progress;
mod workspaces;
use actions::operate_workflow;
use progress::{forget_review_progress, mark_review_files, record_review_visit};
use workspaces::{LocalDiffWorkspaceKind, LocalDiffWorkspaces};

use super::command::{Command, ContextRequest, Outcome};
use crate::git::github::{
    ContextInputs, ContextPurpose, FeedbackInputs, MergeGate, PreparedPullRequest, PullRequest,
    PullRequestAnnotations, PullRequestCommits, PullRequestContext, PullRequestDiffIndex,
    PullRequestProgress, PullRequestReviewSnapshot, PullRequestSuggestions, ReviewSince,
    ReviewSinceRequest, SuggestionPlan, WorkflowOperation, build_context, build_feedback,
    collect_suggestions,
};
use crate::git::{LocalDiffRequest, PreparedLocalDiff, Repository};
use crate::state::ReviewProgressRecord;

pub(crate) struct Session {
    repository: Repository,
    local_diffs: LocalDiffWorkspaces<PreparedLocalDiff>,
    pull_request_diff: Option<(u64, PreparedPullRequest)>,
}

impl Session {
    pub(crate) const fn new(repository: Repository) -> Self {
        Self {
            repository,
            local_diffs: LocalDiffWorkspaces::new(),
            pull_request_diff: None,
        }
    }

    pub(crate) fn execute(&mut self, command: Command) -> Result<Outcome> {
        self.execute_with(command, &mut |_| {}, &|| true)
    }

    pub(crate) fn repository_revision(&self, revision: &str) -> Result<String> {
        self.repository.resolve_revision(revision)
    }

    #[doc = " Refuse to plan an application against the wrong commit. The check"]
    #[doc = " happens before the plan so a caller on another branch is told which"]
    #[doc = " branch to check out rather than that there is nothing to apply."]
    pub(crate) fn ensure_suggestion_checkout(&self, pull_request: &PullRequest) -> Result<()> {
        self.repository
            .ensure_suggestions_apply_cleanly(pull_request, &[])
    }

    #[doc = " Who is reading. The feedback queue reports every row relative to"]
    #[doc = " this login, so an author and a reviewer see different owners."]
    pub(crate) fn viewer_login(&self, pull_request: &PullRequest) -> Result<String> {
        Ok(self
            .repository
            .pull_request_viewer_review(pull_request)?
            .login)
    }

    #[doc = " Resolve which commit a review delta is measured from. This is a read"]
    #[doc = " with no outcome of its own, in the same shape as revision lookup."]
    pub(crate) fn resolve_review_since(
        &self,
        pull_request: &PullRequest,
        request: &ReviewSinceRequest,
        record: &ReviewProgressRecord,
        commits: &PullRequestCommits,
    ) -> Result<ReviewSince> {
        self.repository
            .resolve_review_since(pull_request, request, record, commits)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the command match is the whole session vocabulary and reads better as one table"
    )]
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
            Command::Worktrees => Ok(Outcome::Worktrees(self.repository.worktrees()?)),
            Command::RecentProjects => Ok(Outcome::RecentProjects(
                crate::state::load_recent_projects(self.repository.root()),
            )),
            Command::PrepareLocalDiff { workspace, request } => {
                let kind = LocalDiffWorkspaceKind::from_request(&request);
                let prepared = self.repository.prepare_local_diff(&request)?;
                let index = prepared.index();
                self.local_diffs.store(kind, workspace, prepared);
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
            Command::LocalGitHubRepository => Ok(Outcome::LocalGitHubRepository(
                self.repository.local_github_repository()?.map(Box::new),
            )),
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
            Command::PullRequestStack {
                pull_request,
                refresh,
            } => Ok(Outcome::PullRequestStack(Box::new(
                self.repository.pull_request_stack(&pull_request, refresh)?,
            ))),
            Command::PullRequestGate {
                pull_request,
                refresh,
            } => Ok(Outcome::Gate(Box::new(
                self.repository.pull_request_gate(&pull_request, refresh)?,
            ))),
            Command::PullRequestAnnotations {
                pull_request,
                refresh,
            } => Ok(Outcome::Annotations(Box::new(
                self.repository
                    .pull_request_annotations(&pull_request, refresh)?,
            ))),
            Command::PullRequestWorkflowRuns {
                pull_request,
                refresh,
            } => Ok(Outcome::WorkflowRuns(Box::new(
                self.repository
                    .pull_request_workflow_runs(&pull_request, refresh)?,
            ))),
            Command::PullRequestArtifacts { pull_request, runs } => Ok(Outcome::Artifacts(
                Box::new(self.repository.pull_request_artifacts(&pull_request, &runs)),
            )),
            Command::DownloadArtifact {
                pull_request,
                artifact,
                directory,
            } => Ok(Outcome::DownloadedArtifact(
                self.repository
                    .download_artifact(&pull_request, &artifact, &directory)?,
            )),
            Command::PullRequestDeployments { pull_request, runs } => {
                Ok(Outcome::Deployments(Box::new(
                    self.repository
                        .pull_request_deployments(&pull_request, &runs),
                )))
            }
            Command::OperateWorkflow {
                pull_request,
                operation,
            } => operate_workflow(&self.repository, &pull_request, &operation),
            Command::PullRequestFeedback {
                pull_request,
                gate,
                review,
                annotations,
                viewer,
            } => Ok(feedback::feedback(
                &pull_request,
                gate.as_deref(),
                &review,
                annotations.as_deref(),
                &viewer,
            )),
            Command::PullRequestSuggestions {
                pull_request,
                review,
            } => Ok(feedback::suggestions(&pull_request, &review)),
            Command::PlanSuggestions { suggestions } => Ok(Outcome::SuggestionPlan(Box::new(
                self.repository
                    .plan_suggestions(&suggestions.iter().collect::<Vec<_>>()),
            ))),
            Command::ApplySuggestions {
                pull_request,
                plan,
                message,
            } => feedback::apply(&self.repository, &pull_request, &plan, message.as_deref()),
            Command::PullRequestDependencies { pull_request } => Ok(Outcome::Dependencies(
                Box::new(self.repository.pull_request_dependencies(&pull_request)?),
            )),
            Command::PullRequestSecurity { pull_request } => Ok(Outcome::Security(Box::new(
                self.repository.pull_request_security(&pull_request),
            ))),
            Command::PullRequestContext {
                pull_request,
                request,
            } => {
                let prepared = self
                    .repository
                    .prepare_pull_request_diff(&pull_request, progress)?;
                let index = prepared.index();
                let paths: Vec<PathBuf> =
                    index.files.iter().map(|file| file.path.clone()).collect();
                let selected = match request.path.as_ref() {
                    Some(path) => {
                        if !paths.iter().any(|candidate| candidate == path) {
                            bail!("{} is not part of this pull request", path.display());
                        }
                        vec![path.clone()]
                    }
                    None => paths,
                };
                let (patch, truncated) = prepared.patch_text(&selected)?;
                let merge_base = prepared.merge_base_oid().to_owned();
                let mut bundle =
                    self.pull_request_context(&pull_request, &request, &index, &patch, &merge_base);
                if truncated {
                    bundle.warnings.push(
                        "The patch was larger than Quinjet reads and was cut short".to_owned(),
                    );
                }
                Ok(Outcome::Context(Box::new(bundle)))
            }
            Command::PullRequestStackGate { stack, refresh } => Ok(Outcome::StackGate(Box::new(
                self.repository.pull_request_stack_gate(&stack, refresh),
            ))),
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
            Command::PreparePullRequestSince {
                workspace,
                pull_request,
                since,
            } => {
                let prepared = self.repository.prepare_pull_request_since_diff(
                    &pull_request,
                    &since,
                    progress,
                )?;
                let index = prepared.index();
                self.pull_request_diff = Some((workspace, prepared));
                Ok(Outcome::PullRequestIndex(Box::new(index)))
            }
            Command::PullRequestReviewProgress {
                pull_request,
                index,
                since,
            } => Ok(Outcome::ReviewProgress(Box::new(
                self.repository
                    .pull_request_review_progress(&pull_request, &index, &since)?,
            ))),
            Command::RecordReviewVisit { pull_request } => Ok(record_review_visit(&pull_request)),
            Command::MarkReviewFiles {
                pull_request,
                paths,
                viewed,
            } => Ok(mark_review_files(&pull_request, &paths, viewed)),
            Command::ForgetReviewProgress { pull_request } => {
                Ok(forget_review_progress(&pull_request))
            }
            Command::PreparePullRequestStack {
                workspace,
                stack,
                from,
                to,
            } => {
                let prepared = self
                    .repository
                    .prepare_pull_request_stack_diff(&stack, from, to, progress)?;
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
            Command::PullRequestCommits { pull_request } => Ok(Outcome::Commits(Box::new(
                self.repository.pull_request_commits(&pull_request)?,
            ))),
            Command::PullRequestReview { pull_request } => Ok(Outcome::Review(Box::new(
                self.repository.pull_request_review(&pull_request)?,
            ))),
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
            Command::WarmPullRequestStackMembers { pull_requests } => {
                for pull_request in &pull_requests {
                    if !wanted() {
                        return Ok(Outcome::Warmed);
                    }
                    drop(self.repository.pull_request_checks(pull_request, false));
                }
                for pull_request in &pull_requests {
                    if !wanted() {
                        return Ok(Outcome::Warmed);
                    }
                    drop(self.repository.pull_request_conversation(pull_request));
                }
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
            Command::OperatePullRequest {
                pull_request,
                operation,
            } => {
                let label = operation.label().to_owned();
                let message = self
                    .repository
                    .perform_pull_request_operation(&pull_request, &operation)?;
                Ok(Outcome::Operation {
                    label,
                    changes_history: false,
                    message,
                })
            }
            Command::OperatePullRequestReview {
                pull_request,
                operation,
            } => {
                let label = operation.label().to_owned();
                let message = self
                    .repository
                    .perform_pull_request_review_operation(&pull_request, &operation)?;
                Ok(Outcome::Operation {
                    label,
                    changes_history: false,
                    message,
                })
            }
        }
    }

    fn local_workspace(&self, workspace: u64) -> Result<&PreparedLocalDiff> {
        self.local_diffs
            .get(workspace)
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
