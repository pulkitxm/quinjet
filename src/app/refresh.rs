#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn open_selected_pull_request_in_browser(&self, effects: &mut Vec<AppEffect>) {
        let Some(url) = self
            .selected_pull_request()
            .map(|pull_request| pull_request.url.clone())
        else {
            return;
        };
        effects.push(AppEffect::Open(OpenTarget::Browser(url)));
    }

    pub(super) fn request_active_refresh(&mut self, effects: &mut Vec<AppEffect>) {
        if self.view == View::Changes && self.auxiliary_preview.is_none() {
            self.changes_diff_version = self.changes_diff_version.wrapping_add(1);
            self.invalidate_preview();
            self.local_diff_loading_path = None;
        }
        self.request_refresh(effects);
        if !self.history_branches_loading {
            self.request_history_branches(effects);
        }
        if self.view == View::PullRequests
            && let Some(number) = self
                .pull_request_exact_number
                .or_else(|| self.pull_request_lookup.value.trim().parse::<u64>().ok())
        {
            self.request_pull_request_lookup(number, true, false, effects);
        }
    }

    pub(super) fn request_refresh(&mut self, effects: &mut Vec<AppEffect>) {
        if self.refreshing {
            self.refresh_again = true;
            return;
        }
        self.status_generation = self.status_generation.wrapping_add(1);
        self.refreshing = true;
        effects.push(AppEffect::Git(Box::new(WorkerCommand::Refresh {
            generation: self.status_generation,
        })));
        if matches!(self.modal, Some(Modal::Projects { .. })) {
            self.request_recent_projects(effects);
        } else {
            self.request_worktrees(effects);
        }
    }

    /// A `silent` lookup is a background poll: it keeps the loaded pull request,
    /// its section, its cursors and its diff on screen, and only replaces them
    /// once fresher metadata actually arrives.
    pub(super) fn request_pull_request_lookup(
        &mut self,
        number: u64,
        refresh: bool,
        silent: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        if silent && self.pull_request_loading {
            return;
        }
        self.pull_request_generation = self.pull_request_generation.wrapping_add(1);
        self.pull_request_loading = true;
        self.pull_request_exact_number = Some(number);
        if !silent {
            self.pull_request_error = None;
            self.invalidate_preview();
            self.pull_request_progress = Some(PullRequestProgress::LoadingMetadata);
            self.pull_request_warnings.clear();
            self.pull_request = None;
            self.reset_pull_request_runtime();
            self.set_document(DiffDocument::empty(
                format!("Opening Pull Request #{number}"),
                PullRequestProgress::LoadingMetadata.label(),
            ));
        }
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LookupPullRequest {
            generation: self.pull_request_generation,
            repositories: self.github_repositories.clone(),
            repository: self.pull_request_repository.clone().map(Box::new),
            number,
            refresh,
        })));
    }

    pub(super) fn request_history(&mut self, reset: bool, effects: &mut Vec<AppEffect>) {
        if self.history_loading {
            self.history_refresh_again |= reset;
            return;
        }
        self.history_generation = self.history_generation.wrapping_add(1);
        self.history_loading = true;
        if reset {
            self.history_complete = false;
        }
        let skip = if reset { 0 } else { self.history.len() };
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadHistory {
            generation: self.history_generation,
            revision: self.history_revision(),
            skip,
            limit: HISTORY_PAGE_SIZE,
        })));
    }

    pub(super) fn local_diff_request_for_view(&self) -> Option<LocalDiffRequest> {
        match self.view {
            View::Changes => {
                if let Some(preview) = self.auxiliary_preview.clone() {
                    return Some(match preview {
                        AuxiliaryPreview::Branch(branch) => LocalDiffRequest::Branch {
                            branch: Box::new(branch),
                            current: if self.status.branch.head.is_empty() {
                                "HEAD".to_owned()
                            } else {
                                self.status.branch.head.clone()
                            },
                            current_oid: self.status.branch.oid.clone(),
                            expanded: self.expanded_diff,
                        },
                        AuxiliaryPreview::Stash(stash) => LocalDiffRequest::Stash {
                            stash: Box::new(stash),
                            expanded: self.expanded_diff,
                        },
                    });
                }
                let changes = if self.selected_change_section.is_some() {
                    self.selected_section_changes()
                } else {
                    self.selected_change().cloned().into_iter().collect()
                };
                Some(LocalDiffRequest::Changes {
                    changes,
                    version: self.changes_diff_version,
                    expanded: self.expanded_diff,
                })
            }
            View::History => {
                self.selected_commit()
                    .cloned()
                    .map(|commit| LocalDiffRequest::Commit {
                        commit: Box::new(commit),
                        expanded: self.expanded_diff,
                    })
            }
            View::PullRequests => None,
        }
    }

    pub(super) fn prepare_local_diff(
        &mut self,
        request: LocalDiffRequest,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.local_diff_request.as_ref() == Some(&request)
            && (self.local_diff_workspace_generation.is_some() || self.document_loading)
        {
            return;
        }
        let preserve_document = same_changes_preview(self.local_diff_request.as_ref(), &request)
            && self.document.file_count() > 0;
        let title = match &request {
            LocalDiffRequest::Changes { changes, .. } => changes
                .first()
                .map_or_else(|| "Working Tree".to_owned(), Change::display_path),
            LocalDiffRequest::Commit { commit, .. } => {
                format!("{} — {}", commit.short_id, commit.subject)
            }
            LocalDiffRequest::Branch { branch, .. } => {
                format!("Compare HEAD with {}", branch.name)
            }
            LocalDiffRequest::Stash { stash, .. } => {
                format!("{} — {}", stash.reference, stash.message)
            }
        };
        self.diff_generation = self.diff_generation.wrapping_add(1);
        let generation = self.diff_generation;
        self.reset_local_diff_runtime();
        self.local_diff_request = Some(request.clone());
        self.document_loading = true;
        if !preserve_document {
            self.selected_preview_file = None;
            self.preview_file_cursor = 0;
            self.collapsed_preview_files.clear();
            self.expanded_preview_files.clear();
            self.set_document(DiffDocument::empty(title, "Indexing changed files…"));
        }
        effects.push(AppEffect::Git(Box::new(WorkerCommand::PrepareLocalDiff {
            generation,
            request: Box::new(request),
        })));
    }

    #[expect(
        clippy::unreachable,
        reason = "the branch is impossible for the states that reach it"
    )]
    pub(super) fn request_preview(&mut self, effects: &mut Vec<AppEffect>) {
        if let Some(request) = self.local_diff_request_for_view() {
            self.prepare_local_diff(request, effects);
            if self.view == View::History {
                let visible_len = self.visible_commit_indices().len();
                if self.history_cursor + 20 >= visible_len
                    && !self.history_loading
                    && !self.history_complete
                    && self.filter.is_empty()
                {
                    self.request_history(false, effects);
                }
            }
            return;
        }

        if self.view == View::History {
            self.reset_local_diff_runtime();
            self.document_loading = false;
            self.set_document(DiffDocument::empty(
                "Commit History",
                if self.history.is_empty() {
                    "No commits in this repository"
                } else {
                    "No commits match the current filter"
                },
            ));
            return;
        }

        match self.view {
            View::Changes | View::History => unreachable!("local diff views returned above"),
            View::PullRequests => {
                let Some(pull_request) = self.selected_pull_request().cloned() else {
                    self.document_loading = false;
                    if self.pull_request_section == PullRequestSection::Files {
                        self.set_document(DiffDocument::empty(
                            "Open Pull Request",
                            if self.pull_request_loading {
                                "Fetching pull-request metadata…"
                            } else {
                                "Enter a pull-request number and press Enter"
                            },
                        ));
                    }
                    return;
                };
                let preparing = self.prepare_pull_request_workspace(&pull_request, effects);
                if self.pull_request_section == PullRequestSection::Overview {
                    self.request_check_run_log(false, effects);
                    return;
                }
                if preparing {
                    self.set_document(pull_request_loading_document(
                        &pull_request,
                        PullRequestProgress::PreparingRepository.label(),
                    ));
                    return;
                }

                match self.pull_request_file_view {
                    PullRequestFileView::AllFiles => {
                        self.show_pull_request_all_files();
                        self.request_pull_request_prefetch(effects);
                    }
                    PullRequestFileView::SingleFile => {
                        let Some(path) = self
                            .selected_pull_request_file()
                            .map(|file| file.path.clone())
                        else {
                            self.show_pull_request_all_files();
                            return;
                        };
                        self.request_pull_request_diff_file(path, true, effects);
                    }
                }
            }
        }
    }

    /// Queue the isolated diff workspace unless one is already prepared or in
    /// flight. Returns whether the caller still has to wait for its index.
    pub(super) fn prepare_pull_request_workspace(
        &mut self,
        pull_request: &PullRequest,
        effects: &mut Vec<AppEffect>,
    ) -> bool {
        if self.pull_request_workspace_generation.is_some() {
            return false;
        }
        if self.document_loading && self.pull_request_progress.is_some() {
            return true;
        }
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.document_loading = true;
        self.pull_request_progress = Some(PullRequestProgress::PreparingRepository);
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::PreparePullRequest {
                generation: self.diff_generation,
                pull_request: Box::new(pull_request.clone()),
            },
        )));
        true
    }

    pub(super) const fn invalidate_preview(&mut self) {
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.document_loading = false;
        self.preview_due = None;
    }

    pub(super) fn schedule_preview(&mut self, now: Instant) {
        self.invalidate_preview();
        if self.view != View::PullRequests {
            self.reset_local_diff_runtime();
            self.set_document(self.loading_document_for_view(self.view));
        }
        self.preview_due = Some(now + PREVIEW_DEBOUNCE);
    }

    pub(super) fn normalize_selection(&mut self) {
        let targets = self.change_targets();
        if self.selected_change_target().is_none()
            && let Some(first) = targets.first().copied()
        {
            self.select_change_target(first);
        }
        self.change_cursor = self
            .change_cursor
            .min(self.visible_change_indices().len().saturating_sub(1));
        self.history_cursor = self
            .history_cursor
            .min(self.visible_commit_indices().len().saturating_sub(1));
        self.reset_sidebar_scroll();
    }

    pub(super) fn restore_change_selection(&mut self, selected: Option<&Change>) {
        let visible = self.visible_change_indices();
        if let Some(selected) = selected
            && let Some(cursor) = visible.iter().position(|index| {
                self.status.changes.get(*index).is_some_and(|change| {
                    change.path == selected.path && change.area == selected.area
                })
            })
        {
            self.selected_change_section = None;
            self.change_cursor = cursor;
            return;
        }
        if self.selected_change_target().is_none() {
            let has_changes = visible.iter().any(|index| {
                self.status
                    .changes
                    .get(*index)
                    .is_some_and(|change| ChangeSection::Unstaged.matches(change))
            });
            if has_changes {
                self.selected_change_section = Some(ChangeSection::Unstaged);
            } else if let Some(target) = self.change_targets().first().copied() {
                self.select_change_target(target);
            }
        }
        self.change_cursor = self.change_cursor.min(visible.len().saturating_sub(1));
    }
}

fn same_changes_preview(current: Option<&LocalDiffRequest>, next: &LocalDiffRequest) -> bool {
    matches!(
        (current, next),
        (
            Some(LocalDiffRequest::Changes {
                changes: current_changes,
                expanded: current_expanded,
                ..
            }),
            LocalDiffRequest::Changes {
                changes: next_changes,
                expanded: next_expanded,
                ..
            }
        ) if current_changes == next_changes && current_expanded == next_expanded
    )
}
