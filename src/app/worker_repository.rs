#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::excessive_nesting,
        reason = "the event handler mirrors the result and modal states it decodes"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive event match keeps each state transition together"
    )]
    pub(super) fn handle_repository_worker_event(
        &mut self,
        event: WorkerEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match event {
            WorkerEvent::History {
                generation,
                skip,
                result,
            } => {
                if generation != self.history_generation {
                    return effects;
                }
                self.history_loading = false;
                match result {
                    Ok(commits) => {
                        let received = commits.len();
                        if skip == 0 {
                            self.history = commits;
                            self.history_cursor = self
                                .history_cursor
                                .min(self.visible_commit_indices().len().saturating_sub(1));
                        } else if skip == self.history.len() {
                            self.history.extend(commits);
                        }
                        self.history_complete = received < HISTORY_PAGE_SIZE;
                        if self.view == View::History {
                            self.schedule_preview(now);
                        }
                    }
                    Err(error) => {
                        if self.history_branch.take().is_some() {
                            self.show_toast(
                                format!(
                                    "Viewed branch is unavailable; returning to HEAD history: {error}"
                                ),
                                ToastLevel::Error,
                                now,
                            );
                            self.request_history(true, &mut effects);
                        } else {
                            self.show_toast(error, ToastLevel::Error, now);
                        }
                    }
                }
                if self.history_refresh_again {
                    self.history_refresh_again = false;
                    self.request_history(true, &mut effects);
                }
            }
            WorkerEvent::GitHubRepositories { generation, result } => {
                if generation != self.repository_generation {
                    return effects;
                }
                match result {
                    Ok((repositories, warnings)) => {
                        self.github_repositories = repositories;
                        self.pull_request_warnings = warnings;
                        if let Some(Modal::PullRequestRepositories {
                            items,
                            selected,
                            loading,
                            ..
                        }) = self.modal.as_mut()
                        {
                            items.clone_from(&self.github_repositories);
                            *selected = self
                                .pull_request_repository
                                .as_ref()
                                .and_then(|current| {
                                    items.iter().position(|repository| {
                                        repository.url.eq_ignore_ascii_case(&current.url)
                                    })
                                })
                                .unwrap_or_default();
                            *loading = false;
                        }
                    }
                    Err(error) => {
                        if matches!(self.modal, Some(Modal::PullRequestRepositories { .. })) {
                            self.modal = None;
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::LocalGitHubRepository {
                result: Ok(repository),
            } => {
                self.recent_pull_requests = repository
                    .as_ref()
                    .map_or_else(Vec::new, cached_recent_pull_requests_for);
                self.recent_pull_request_cursor = 0;
                self.local_github_repository = repository;
            }
            WorkerEvent::PullRequestLookup { generation, result } => {
                if generation != self.pull_request_generation {
                    return effects;
                }
                self.pull_request_loading = false;
                match result {
                    Ok(snapshot) => {
                        if !snapshot.repositories.is_empty() {
                            self.github_repositories = snapshot.repositories;
                        }
                        let current = snapshot.pull_request;
                        let newly_opened = self.pull_request.is_none();
                        let previous = self.pull_request.take();
                        if previous.as_ref() != Some(&current) {
                            self.invalidate_pull_request_content_rows();
                        }
                        let same = previous.as_ref().is_some_and(|previous| {
                            previous.number == current.number
                                && previous
                                    .base_repository
                                    .url
                                    .eq_ignore_ascii_case(&current.base_repository.url)
                        });
                        let head_moved =
                            previous.is_some_and(|previous| previous.head_oid != current.head_oid);
                        if newly_opened {
                            self.recent_pull_requests =
                                updated_recent_pull_requests(&self.recent_pull_requests, &current);
                            self.recent_pull_request_cursor = 0;
                        }
                        self.pull_request = Some(current);
                        self.pull_request_repository = snapshot.selected_repository;
                        self.pull_request_warnings = snapshot.warnings;
                        self.pull_request_exact_number = snapshot.exact_number;
                        self.pull_request_from_cache = snapshot.from_cache;
                        if !same {
                            self.reset_pull_request_runtime();
                        } else if head_moved {
                            self.reset_pull_request_diff_runtime();
                        }
                        self.pull_request_progress = None;
                        self.pull_request_error = None;
                        self.schedule_pull_request_poll(now);
                        self.request_pull_request_stack(
                            self.pull_request_lookup_refresh,
                            &mut effects,
                        );
                        if self.pull_request_stack.is_none() {
                            self.request_pull_request_checks(true, &mut effects);
                            self.request_pull_request_conversation(true, &mut effects);
                        }
                        if !same || head_moved {
                            if self.view == View::PullRequests && self.pull_request_stack.is_none()
                            {
                                self.preview_due = None;
                                self.request_preview(&mut effects);
                            } else {
                                self.mark_view_preview_for_resume(View::PullRequests);
                            }
                        }
                    }
                    Err(error) => {
                        self.pull_request_stack_loading = false;
                        self.pull_request_progress = None;
                        self.pull_request_error = Some(error.clone());
                        if self.pull_request_stack.is_some() {
                            let warning = format!("Unable to refresh pull request: {error}");
                            self.pull_request_warnings.retain(|warning| {
                                !warning.starts_with("Unable to refresh pull request:")
                            });
                            self.pull_request_warnings.push(warning);
                            self.pull_request_stack_error = Some(error.clone());
                        } else {
                            self.invalidate_pull_request_content_rows();
                            self.set_view_document(
                                View::PullRequests,
                                DiffDocument::empty("Pull Requests", error.clone()),
                            );
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::PullRequestStack { generation, result } => {
                if generation != self.pull_request_generation {
                    return effects;
                }
                self.pull_request_stack_loading = false;
                match result {
                    Ok(snapshot) => {
                        let stale_error = snapshot
                            .warnings
                            .iter()
                            .find(|warning| {
                                warning.starts_with(
                                    "GitHub is unavailable; showing stale cached stack data",
                                )
                            })
                            .cloned();
                        self.pull_request_warnings.retain(|warning| {
                            !warning.starts_with("Unable to load pull-request stack:")
                                && !warning.starts_with(
                                    "GitHub is unavailable; showing stale cached stack data",
                                )
                        });
                        for warning in snapshot.warnings {
                            if !self.pull_request_warnings.contains(&warning) {
                                self.pull_request_warnings.push(warning);
                            }
                        }
                        if stale_error.is_none() || self.pull_request_stack.is_none() {
                            self.apply_pull_request_stack_snapshot(snapshot.stack, &mut effects);
                        }
                        if let Some(error) = stale_error {
                            self.pull_request_stack_error = Some(error);
                        }
                    }
                    Err(error) => {
                        let warning = format!("Unable to load pull-request stack: {error}");
                        self.pull_request_warnings.retain(|warning| {
                            !warning.starts_with("Unable to load pull-request stack:")
                        });
                        self.pull_request_warnings.push(warning);
                        self.pull_request_stack_error = Some(error);
                    }
                }
            }
            WorkerEvent::PullRequestProgress {
                generation,
                diff,
                progress,
            } => {
                let current = if diff {
                    generation == self.diff_generation
                } else {
                    generation == self.pull_request_generation
                };
                if current {
                    self.pull_request_progress = Some(progress);
                }
            }
            WorkerEvent::Branches { generation, result } => {
                if generation != self.branch_generation {
                    return effects;
                }
                match result {
                    Ok(items) => {
                        if let Some(Modal::Branches {
                            items: modal_items,
                            selected,
                            loading,
                            ..
                        }) = self.modal.as_mut()
                        {
                            *modal_items = items;
                            *loading = false;
                            *selected = 0;
                        }
                    }
                    Err(error) => {
                        self.modal = None;
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::HistoryBranches { generation, result } => {
                if generation != self.history_branch_generation {
                    return effects;
                }
                self.history_branches_loading = false;
                match result {
                    Ok(items) => {
                        self.history_branches_loaded = true;
                        self.history_branches = items;
                        match self.modal.as_mut() {
                            Some(Modal::HistoryBranches {
                                items: modal_items,
                                selected,
                                loading,
                                ..
                            }) => {
                                *selected = self
                                    .history_branches
                                    .iter()
                                    .position(|branch| {
                                        self.history_branch
                                            .as_ref()
                                            .map_or(branch.current, |selected| {
                                                selected.reference == branch.reference
                                            })
                                    })
                                    .unwrap_or_default();
                                modal_items.clone_from(&self.history_branches);
                                *loading = false;
                            }
                            Some(Modal::CompareBranches {
                                items: modal_items,
                                selected,
                                loading,
                                ..
                            }) => {
                                modal_items.clone_from(&self.history_branches);
                                *selected = 0;
                                *loading = false;
                            }
                            _ => {}
                        }
                    }
                    Err(error) => {
                        if matches!(
                            self.modal,
                            Some(Modal::HistoryBranches { .. } | Modal::CompareBranches { .. })
                        ) {
                            self.modal = None;
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::Stashes { generation, result } => {
                if generation != self.stash_generation {
                    return effects;
                }
                match result {
                    Ok(items) => {
                        if let Some(Modal::Stashes {
                            items: modal_items,
                            selected,
                            loading,
                            ..
                        }) = self.modal.as_mut()
                        {
                            *modal_items = items;
                            *selected = 0;
                            *loading = false;
                        }
                    }
                    Err(error) => {
                        if matches!(self.modal, Some(Modal::Stashes { .. })) {
                            self.modal = None;
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::Worktrees { generation, result } => {
                if generation != self.worktree_generation {
                    return effects;
                }
                match result {
                    Ok(items) => {
                        self.worktrees = items;
                    }
                    Err(error) => {
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::RecentProjects { generation, result } => {
                if generation != self.project_generation {
                    return effects;
                }
                match result {
                    Ok(groups) => {
                        self.project_groups.clone_from(&groups);
                        self.apply_current_worktrees(&groups);
                        if let Some(Modal::Projects {
                            groups: modal_groups,
                            selected,
                            query,
                            collapsed,
                            loading,
                            ..
                        }) = self.modal.as_mut()
                        {
                            let initial = modal_groups.is_empty();
                            let visible =
                                Self::filtered_project_rows(&groups, &query.value, collapsed);
                            *selected = if initial {
                                Self::first_project_worktree_index(&groups, &query.value, collapsed)
                            } else {
                                (*selected).min(visible.len().saturating_sub(1))
                            };
                            *modal_groups = groups;
                            *loading = false;
                        }
                    }
                    Err(error) => {
                        if matches!(self.modal, Some(Modal::Projects { .. })) {
                            self.modal = None;
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::OperationFinished {
                id,
                label,
                changes_history,
                refresh_pull_request,
                result,
            } => {
                if id != self.operation_id {
                    return effects;
                }
                self.busy = None;
                match result {
                    Ok(message) => {
                        self.show_toast(message, ToastLevel::Success, now);
                        self.request_refresh(&mut effects);
                        if changes_history {
                            self.request_history(true, &mut effects);
                        }
                    }
                    Err(error) => {
                        self.show_toast(format!("{label}: {error}"), ToastLevel::Error, now);
                        self.request_refresh(&mut effects);
                    }
                }
                if refresh_pull_request {
                    self.refresh_loaded_pull_request(&mut effects);
                }
            }
            _ => {}
        }
        effects
    }
}
