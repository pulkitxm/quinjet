#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive event match keeps each state transition together"
    )]
    pub(super) fn handle_content_worker_event(
        &mut self,
        event: WorkerEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match event {
            WorkerEvent::Status { generation, result } => {
                if generation != self.status_generation {
                    return effects;
                }
                self.refreshing = false;
                match result {
                    Ok(status) => {
                        let selected = self
                            .selected_change_section
                            .is_none()
                            .then(|| self.selected_change().cloned())
                            .flatten();
                        let branch_was_known =
                            !self.status.branch.head.is_empty() || self.status.branch.oid.is_some();
                        let branch_changed = self.status.branch.head != status.branch.head
                            || self.status.branch.oid != status.branch.oid;
                        self.status = status;
                        self.checked_change_paths.retain(|path| {
                            self.status
                                .changes
                                .iter()
                                .any(|change| &change.path == path)
                        });
                        self.restore_change_selection(selected.as_ref());
                        self.last_refresh = Some(now);
                        if branch_changed
                            && self.history_branch.is_none()
                            && (branch_was_known || !self.history_loading)
                        {
                            self.request_history(true, &mut effects);
                        }
                        if self.view == View::Changes && !self.refresh_again {
                            self.preview_due = None;
                            self.request_preview(&mut effects);
                        }
                    }
                    Err(error) => {
                        if self.local_diff_preserving_document && !self.document_loading {
                            self.abort_preserved_local_diff_refresh();
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
                if self.refresh_again {
                    self.refresh_again = false;
                    self.request_refresh(&mut effects);
                }
            }
            WorkerEvent::LocalDiffIndex { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
                let preserve_document = self.local_diff_preserving_document;
                if !preserve_document {
                    self.document_loading = false;
                }
                self.local_diff_loading_path = None;
                match result {
                    Ok(index) => {
                        let selected_path = self.selected_preview_file.clone().filter(|selected| {
                            index.files.iter().any(|file| &file.path == selected)
                        });
                        let paths = index
                            .files
                            .iter()
                            .map(|file| file.path.clone())
                            .collect::<Vec<_>>();
                        self.local_diff_workspace_generation = Some(generation);
                        self.local_diff_documents.clear();
                        self.local_diff_index = Some(index);
                        if !preserve_document {
                            self.reset_preview_file_folds(&paths);
                            self.selected_preview_file =
                                selected_path.clone().or_else(|| paths.first().cloned());
                            self.preview_file_cursor = self
                                .selected_preview_file
                                .as_ref()
                                .and_then(|selected| paths.iter().position(|path| path == selected))
                                .unwrap_or_default();
                        }
                        let first_path = selected_path.or_else(|| paths.first().cloned());
                        if !preserve_document {
                            self.rebuild_local_diff_document();
                            self.content_scroll = 0;
                            self.horizontal_scroll = 0;
                        }
                        let mut load_paths = paths
                            .iter()
                            .filter(|path| {
                                if preserve_document {
                                    !self.refreshed_preview_file_collapsed(path, paths.len())
                                } else {
                                    !self.preview_file_collapsed(&path.to_string_lossy())
                                }
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        if let Some(path) = first_path
                            && !load_paths.contains(&path)
                        {
                            load_paths.insert(0, path);
                        }
                        for path in load_paths {
                            self.request_local_diff_file(path, &mut effects);
                        }
                        if self.local_diff_loading_path.is_none()
                            && self.local_diff_pending_paths.is_empty()
                        {
                            self.request_next_local_diff_file(&mut effects);
                        }
                    }
                    Err(error) => {
                        if preserve_document {
                            self.abort_preserved_local_diff_refresh();
                        } else {
                            self.document_loading = false;
                            self.reset_local_diff_runtime();
                            self.set_document(DiffDocument::empty("Preview Error", error.clone()));
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::LocalDiffFile {
                generation,
                workspace_generation,
                path,
                result,
            } => {
                if generation != self.diff_generation
                    || self.local_diff_workspace_generation != Some(workspace_generation)
                    || self.local_diff_loading_path.as_ref() != Some(&path)
                    || !self
                        .local_diff_index
                        .as_ref()
                        .is_some_and(|index| index.files.iter().any(|file| file.path == path))
                {
                    return effects;
                }
                self.local_diff_loading_path = None;
                match result {
                    Ok(document) => self.store_local_diff_document(path, document),
                    Err(error) if self.local_diff_preserving_document => {
                        self.abort_preserved_local_diff_refresh();
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                    Err(error) => {
                        let title = self.local_diff_index.as_ref().map_or_else(
                            || "Preview Error".to_owned(),
                            |index| index.title.clone(),
                        );
                        self.store_local_diff_document(
                            path,
                            DiffDocument::empty(&title, format!("Unable to load diff: {error}")),
                        );
                        self.local_diff_request = None;
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
                self.request_next_local_diff_file(&mut effects);
            }
            WorkerEvent::PullRequestIndex { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
                self.pull_request_progress = None;
                match result {
                    Ok(index) => {
                        self.apply_pull_request_index(index);
                        self.pull_request_workspace_generation = Some(generation);
                        self.reset_sidebar_scroll();
                        self.content_scroll = 0;
                        self.horizontal_scroll = 0;
                        self.document_loading = false;
                        self.request_pull_request_prefetch(&mut effects);
                    }
                    Err(error) => {
                        self.document_loading = false;
                        self.pull_request_workspace_generation = None;
                        self.set_document(DiffDocument::empty("Preview Error", error.clone()));
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::PullRequestDiff { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
                self.document_loading = false;
                self.pull_request_progress = None;
                let requested_path = self.pull_request_loading_path.take();
                match result {
                    Ok(document) => {
                        let path = requested_path.or_else(|| {
                            document
                                .pull_request_details
                                .as_ref()
                                .and_then(|details| details.selected_file.as_ref())
                                .map(PathBuf::from)
                        });
                        if let Some(path) = path.as_deref() {
                            let _ = self.backfill_pull_request_counts(path, &document);
                        }
                        match self.pull_request_file_view {
                            PullRequestFileView::AllFiles => {
                                if let Some(path) = path {
                                    self.cache_pull_request_document(path, document);
                                    self.rebuild_pull_request_all_files_document();
                                } else {
                                    self.set_document(document);
                                }
                            }
                            PullRequestFileView::SingleFile => {
                                self.cache_current_pull_request_single_document();
                                self.set_document(document);
                                self.pull_request_single_file = path;
                                self.selected_preview_file = None;
                                self.preview_file_cursor = 0;
                                self.content_scroll = 0;
                                self.horizontal_scroll = 0;
                                self.decorate_pull_request_review();
                            }
                        }
                    }
                    Err(error) => self.show_toast(error, ToastLevel::Error, now),
                }
                self.request_pull_request_prefetch(&mut effects);
            }
            WorkerEvent::PullRequestDiffBatch {
                workspace_generation,
                result,
            } => {
                if Some(workspace_generation) != self.pull_request_workspace_generation {
                    return effects;
                }
                self.pull_request_prefetching = false;
                match result {
                    Ok(documents) => {
                        self.pull_request_prefetch_retrying = false;
                        let mut arrived_visible = false;
                        let mut counts_changed = false;
                        for (path, document) in documents {
                            if !self.pull_request_documents.contains_key(&path) {
                                arrived_visible = arrived_visible
                                    || !self.preview_file_collapsed(&path.to_string_lossy());
                                counts_changed |=
                                    self.backfill_pull_request_counts(&path, &document);
                                self.cache_pull_request_document(path, document);
                            }
                        }
                        if (arrived_visible || counts_changed)
                            && self.pull_request_file_view == PullRequestFileView::AllFiles
                        {
                            self.rebuild_pull_request_all_files_document();
                        }
                        self.request_pull_request_prefetch(&mut effects);
                    }
                    Err(_) if !self.pull_request_prefetch_retrying => {
                        self.pull_request_prefetch_retrying = true;
                        self.request_pull_request_prefetch(&mut effects);
                    }
                    Err(_) => {
                        self.pull_request_prefetch_retrying = false;
                    }
                }
            }
            WorkerEvent::PullRequestChecks { generation, result } => {
                if generation != self.pull_request_checks_generation {
                    return effects;
                }
                self.pull_request_checks_loading = false;
                match result {
                    Ok(snapshot) => {
                        let selected = self
                            .selected_pull_request_check()
                            .map(PullRequestCheck::identity);
                        let was_running = self
                            .selected_pull_request_check()
                            .is_some_and(|check| check.status.is_running());
                        let changed = self.pull_request_checks_error.is_some()
                            || self.pull_request_checks_from_cache != snapshot.from_cache
                            || self.pull_request_checks != snapshot.checks
                            || snapshot.checks.is_empty();
                        self.pull_request_checks_from_cache = snapshot.from_cache;
                        self.pull_request_checks = snapshot.checks;
                        if changed {
                            self.invalidate_pull_request_content_rows();
                        }
                        let cursor = selected.and_then(|selected| {
                            self.pull_request_checks
                                .iter()
                                .position(|check| check.identity() == selected)
                        });
                        let _ = self.set_check_cursor(cursor);
                        self.pull_request_checks_error = None;
                        if was_running {
                            self.request_check_run_log(true, &mut effects);
                        }
                        self.request_check_log_prefetch(&mut effects);
                    }
                    Err(error) => {
                        if self.pull_request_checks.is_empty()
                            || self.pull_request_checks_error.as_ref() != Some(&error)
                        {
                            self.pull_request_checks_error = Some(error);
                            self.invalidate_pull_request_content_rows();
                        }
                    }
                }
            }
            WorkerEvent::CheckRunLog { generation, result } => {
                if generation != self.pull_request_check_log_generation {
                    return effects;
                }
                self.pull_request_check_log_loading = false;
                let following = self.content_at_bottom
                    && self
                        .selected_pull_request_check()
                        .is_some_and(|check| check.status.is_running());
                match result {
                    Ok(log) => {
                        let auto_expanded = if self.expanded_check_steps.is_empty()
                            && let Some(step) = log.failed_step().or_else(|| log.running_step())
                        {
                            let number = step.number;
                            let _ = self.expanded_check_steps.insert(number);
                            self.reveal_check_step(number);
                            true
                        } else {
                            false
                        };
                        if self.pull_request_step_cursor == 0
                            && let Some(step) = log.steps.first()
                        {
                            let number = step.number;
                            self.reveal_check_step(number);
                        }
                        let changed = auto_expanded
                            || log.running_step().is_some()
                            || self.pull_request_check_log_error.is_some()
                            || self.pull_request_check_log.as_ref() != Some(&log);
                        self.pull_request_check_log = Some(log);
                        if changed {
                            self.invalidate_pull_request_content_rows();
                        }
                        self.pull_request_check_log_error = None;
                        if following {
                            self.content_scroll = usize::MAX;
                        }
                    }
                    Err(error) => {
                        let changed = self.pull_request_check_log.take().is_some()
                            || self.pull_request_check_log_error.as_ref() != Some(&error);
                        self.pull_request_check_log_error = Some(error);
                        if changed {
                            self.invalidate_pull_request_content_rows();
                        }
                    }
                }
            }
            WorkerEvent::PullRequestConversation { generation, result } => {
                if generation != self.pull_request_conversation_generation {
                    return effects;
                }
                self.pull_request_conversation_loading = false;
                match result {
                    Ok(conversation) => {
                        if self.pull_request_conversation_error.is_some()
                            || conversation.entries.is_empty()
                            || self.pull_request_conversation != conversation
                        {
                            self.pull_request_conversation = conversation;
                            self.pull_request_conversation_error = None;
                            self.invalidate_pull_request_content_rows();
                        }
                    }
                    Err(error) => {
                        if self.pull_request_conversation_error.as_ref() != Some(&error) {
                            self.pull_request_conversation_error = Some(error);
                            self.invalidate_pull_request_content_rows();
                        }
                    }
                }
                if self.pull_request_conversation_refresh_again {
                    self.pull_request_conversation_refresh_again = false;
                    self.request_pull_request_conversation(true, &mut effects);
                }
            }
            WorkerEvent::PullRequestReview { generation, result } => {
                if generation != self.pull_request_review_generation {
                    return effects;
                }
                self.pull_request_review_loading = false;
                let mutated = std::mem::take(&mut self.pull_request_review_mutating);
                match result {
                    Ok(review) => {
                        self.pull_request_review = review;
                        self.pull_request_review_error = None;
                        self.decorate_pull_request_review();
                        if mutated {
                            self.show_toast(
                                "Pull request review updated".to_owned(),
                                ToastLevel::Success,
                                now,
                            );
                        }
                    }
                    Err(error) => {
                        self.pull_request_review_error = Some(error.clone());
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            _ => {}
        }
        effects
    }
}
