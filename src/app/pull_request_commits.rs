#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::git::github::{PullRequestCommit, PullRequestCommits};

#[derive(Debug, Default)]
pub(crate) struct CommitBrowser {
    pub owner: Option<PullRequest>,
    pub snapshot: PullRequestCommits,
    pub loaded: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub generation: u64,
    pub cursor: usize,
    pub selected: Option<usize>,
}

impl CommitBrowser {
    pub(super) fn reset(&mut self) {
        let generation = self.generation.wrapping_add(1);
        *self = Self {
            generation,
            ..Self::default()
        };
    }
}

pub(super) fn pr_commit_details(commit: &PullRequestCommit) -> crate::git::diff::CommitDetails {
    crate::git::diff::CommitDetails {
        id: commit.oid.clone(),
        subject: commit.subject.clone(),
        author: commit.author.clone(),
        authored_at: commit.authored_at.clone(),
        committed_at: commit.committed_at.clone(),
        ..crate::git::diff::CommitDetails::default()
    }
}

impl App {
    pub(super) fn pr_commit_prepare_command(
        &self,
        index: usize,
        pull_request: &PullRequest,
    ) -> WorkerCommand {
        WorkerCommand::PreparePullRequest {
            commit: self
                .pr_commits
                .snapshot
                .commits
                .get(index)
                .map(|commit| commit.oid.clone()),
            generation: self.diff_generation,
            pull_request: Box::new(pull_request.clone()),
        }
    }

    pub(crate) fn selected_pr_commit(&self) -> Option<&PullRequestCommit> {
        self.pr_commits
            .snapshot
            .commits
            .get(self.pr_commits.selected?)
    }

    pub(super) fn request_pr_commits(&mut self, effects: &mut Vec<AppEffect>) {
        if self.pr_commits.loading || self.pr_commits.loaded {
            return;
        }
        let Some(request) = self.selected_pull_request().cloned() else {
            return;
        };
        self.pr_commits.generation = self.pr_commits.generation.wrapping_add(1);
        self.pr_commits.loading = true;
        self.pr_commits.error = None;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestCommits {
                generation: self.pr_commits.generation,
                pull_request: Box::new(request),
            },
        )));
    }

    pub(super) fn reconcile_pr_commit_owner(&mut self, effects: &mut Vec<AppEffect>) {
        let Some(owner) = self.pr_commits.owner.as_ref() else {
            return;
        };
        let current = self.pull_request_stack.as_ref().and_then(|stack| {
            if !stack
                .repository
                .url
                .eq_ignore_ascii_case(&owner.base_repository.url)
            {
                return None;
            }
            let member = stack
                .members
                .iter()
                .find(|member| member.number == owner.number)?;
            stack.member_pull_request(member.position)
        });
        if current.as_ref().is_some_and(|current| {
            current.base_oid == owner.base_oid && current.head_oid == owner.head_oid
        }) {
            self.pr_commits.owner = current;
            return;
        }
        self.pr_commits.reset();
        self.pr_commits.owner = current;
        self.clear_pr_commit_checks();
        self.invalidate_preview();
        self.reset_pull_request_diff_runtime();
        if self.pull_request_section == PullRequestSection::Commits {
            self.request_pr_commits(effects);
        }
        self.request_preview(effects);
        self.request_pull_request_checks(true, effects);
    }

    pub(super) fn apply_pr_commits(
        &mut self,
        generation: u64,
        result: Result<PullRequestCommits, String>,
        effects: &mut Vec<AppEffect>,
    ) {
        if generation != self.pr_commits.generation {
            return;
        }
        self.pr_commits.loading = false;
        match result {
            Ok(mut snapshot) => {
                if self.selected_pull_request().is_none_or(|request| {
                    request.base_oid != snapshot.base_oid || request.head_oid != snapshot.head_oid
                }) {
                    return;
                }
                snapshot.commits.reverse();
                self.pr_commits.snapshot = snapshot;
                self.pr_commits.loaded = true;
                self.pr_commits.error = None;
                self.pr_commits.cursor = self
                    .pr_commits
                    .cursor
                    .min(self.pr_commits.snapshot.commits.len().saturating_sub(1));
            }
            Err(error) => self.pr_commits.error = Some(error),
        }
        if self.pull_request_section == PullRequestSection::Commits {
            self.request_preview(effects);
        }
    }

    pub(super) fn clear_pr_commit_checks(&mut self) {
        self.pull_request_checks_generation = self.pull_request_checks_generation.wrapping_add(1);
        self.pull_request_checks.clear();
        self.pull_request_checks_loading = false;
        self.pull_request_checks_error = None;
        self.pull_request_checks_from_cache = false;
        self.pull_request_check_cursor = None;
        self.selected_check_section = None;
        self.collapsed_check_sections.clear();
        self.pull_request_prefetched_logs.clear();
        self.pull_request_checks_read_at = None;
        self.invalidate_check_run_log();
        self.invalidate_pull_request_content_rows();
    }

    pub(super) fn inspect_stack_commit(&mut self, index: usize, effects: &mut Vec<AppEffect>) {
        let Some(owner) = self.selected_pull_request().cloned() else {
            return;
        };
        let snapshot = &self.stack_inspector.commits;
        if snapshot.commits.get(index).is_none() {
            return;
        }
        let selected = snapshot.commits.len().saturating_sub(index + 1);
        let mut snapshot = snapshot.clone();
        snapshot.commits.reverse();
        self.pr_commits.reset();
        self.pr_commits.owner = Some(owner);
        self.pr_commits.snapshot = snapshot;
        self.pr_commits.loaded = true;
        self.inspect_pr_commit(Some(selected), effects);
    }

    pub(super) fn inspect_pr_commit(&mut self, index: Option<usize>, effects: &mut Vec<AppEffect>) {
        if index.is_some_and(|index| self.pr_commits.snapshot.commits.get(index).is_none()) {
            return;
        }
        self.pr_commits.selected = index;
        if let Some(index) = index {
            self.pr_commits.cursor = index;
        }
        self.invalidate_preview();
        self.reset_pull_request_diff_runtime();
        self.pull_request_files.clear();
        self.pull_request_tree.clear();
        self.pull_request_total_files = 0;
        self.collapsed_pull_request_directories.clear();
        self.clear_pr_commit_checks();
        self.pull_request_section = PullRequestSection::Files;
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.reset_sidebar_scroll();
        self.request_preview(effects);
        self.request_pull_request_checks(true, effects);
    }

    pub(super) fn move_pr_commit_cursor(&mut self, amount: isize) {
        self.pr_commits.cursor = if amount < 0 {
            self.pr_commits.cursor.saturating_sub(amount.unsigned_abs())
        } else {
            self.pr_commits
                .cursor
                .saturating_add(count(amount))
                .min(self.pr_commits.snapshot.commits.len().saturating_sub(1))
        };
        self.show_pr_commit_summary();
    }

    pub(super) const fn show_pr_commit_summary(&mut self) {
        self.invalidate_pull_request_content_rows();
    }
}
