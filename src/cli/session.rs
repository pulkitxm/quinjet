use anyhow::Result;

use super::command::{Command, Outcome};
use crate::git::github::{PreparedPullRequest, PullRequestProgress};
use crate::git::{LocalDiffRequest, PreparedLocalDiff, Repository};

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

#[derive(Clone, Copy)]
enum LocalDiffWorkspaceKind {
    Changes,
    History,
}

impl LocalDiffWorkspaceKind {
    const fn from_request(request: &LocalDiffRequest) -> Self {
        match request {
            LocalDiffRequest::Commit { .. } => Self::History,
            LocalDiffRequest::Changes { .. }
            | LocalDiffRequest::Branch { .. }
            | LocalDiffRequest::Stash { .. } => Self::Changes,
        }
    }
}

struct LocalDiffWorkspaces<T> {
    changes: Option<(u64, T)>,
    history: Option<(u64, T)>,
}

impl<T> LocalDiffWorkspaces<T> {
    const fn new() -> Self {
        Self {
            changes: None,
            history: None,
        }
    }

    fn store(&mut self, kind: LocalDiffWorkspaceKind, workspace: u64, prepared: T) {
        let slot = match kind {
            LocalDiffWorkspaceKind::Changes => &mut self.changes,
            LocalDiffWorkspaceKind::History => &mut self.history,
        };
        *slot = Some((workspace, prepared));
    }

    fn get(&self, workspace: u64) -> Option<&T> {
        [&self.changes, &self.history]
            .into_iter()
            .flatten()
            .find(|(candidate, _)| *candidate == workspace)
            .map(|(_, prepared)| prepared)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::cell::Cell;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    use super::{Command, Outcome, Session};
    use super::{LocalDiffWorkspaceKind, LocalDiffWorkspaces};
    #[cfg(unix)]
    use crate::git::github::{GitHubRepository, PullRequest};
    #[cfg(unix)]
    use crate::git::tests::TestRepository;

    #[test]
    fn paused_changes_workspace_survives_history_browsing() {
        let mut workspaces = LocalDiffWorkspaces::new();
        workspaces.store(LocalDiffWorkspaceKind::Changes, 11, 110);

        for generation in 12..100 {
            workspaces.store(LocalDiffWorkspaceKind::History, generation, generation * 10);
        }

        assert_eq!(workspaces.get(11), Some(&110));
        assert_eq!(workspaces.get(98), None);
        assert_eq!(workspaces.get(99), Some(&990));
    }

    #[test]
    fn each_view_replaces_only_its_own_workspace() {
        let mut workspaces = LocalDiffWorkspaces::new();
        workspaces.store(LocalDiffWorkspaceKind::Changes, 21, 210);
        workspaces.store(LocalDiffWorkspaceKind::History, 22, 220);
        workspaces.store(LocalDiffWorkspaceKind::Changes, 23, 230);

        assert_eq!(workspaces.get(21), None);
        assert_eq!(workspaces.get(22), Some(&220));
        assert_eq!(workspaces.get(23), Some(&230));
    }

    #[cfg(unix)]
    #[test]
    fn stack_warming_continues_after_a_member_read_fails() {
        let fixture = TestRepository::with_branch("main");
        let repository = fixture.repository();
        let executable = repository.root().join("gh");
        let calls = repository.root().join("calls");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf 'call\\n' >> '{}'\nprintf 'failed\\n' >&2\nexit 1\n",
                calls.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        let mut session = Session::new(fixture.repository_with_github_cli(executable));
        let pull_requests = [41, 42]
            .map(|number| PullRequest {
                number,
                head_oid: format!("head-{number}"),
                base_repository: GitHubRepository {
                    name_with_owner: "acme/widget".to_owned(),
                    url: format!("https://example.test/warm/{number}"),
                    remotes: Vec::new(),
                },
                ..PullRequest::default()
            })
            .into_iter()
            .collect();
        let wanted_calls = Cell::new(0);

        let outcome = session
            .execute_with(
                Command::WarmPullRequestStackMembers { pull_requests },
                &mut |_| {},
                &|| {
                    let current = wanted_calls.get();
                    wanted_calls.set(current + 1);
                    current < 2
                },
            )
            .unwrap();

        assert!(matches!(outcome, Outcome::Warmed));
        assert_eq!(fs::read_to_string(calls).unwrap(), "call\ncall\n");
    }
}
