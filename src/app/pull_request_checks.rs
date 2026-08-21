#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    /// `refresh` separates a live poll from merely arriving in the section: the
    /// latter reuses what is already loaded rather than spending a request.
    pub(super) fn request_pull_request_checks(
        &mut self,
        refresh: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.pull_request_checks_loading || (!refresh && !self.pull_request_checks.is_empty()) {
            return;
        }
        let Some(pull_request) = self.pull_request.clone() else {
            return;
        };
        self.pull_request_checks_generation = self.pull_request_checks_generation.wrapping_add(1);
        self.pull_request_checks_loading = true;
        if self.pull_request_checks.is_empty() {
            self.invalidate_pull_request_content_rows();
        }
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestChecks {
                generation: self.pull_request_checks_generation,
                pull_request: Box::new(pull_request),
                refresh,
            },
        )));
    }

    /// The overview sidebar is the pull request itself, then status sections and
    /// their checks, so the cursor walks that composed list.
    pub(super) fn move_check_cursor(&mut self, amount: isize) {
        let targets = self.check_list_targets();
        if targets.is_empty() {
            return;
        }
        let current = self.selected_check_list_target();
        let index = targets
            .iter()
            .position(|target| *target == current)
            .unwrap_or(0);
        let next = if amount < 0 {
            index.saturating_sub(amount.unsigned_abs())
        } else {
            index.saturating_add(count(amount)).min(targets.len() - 1)
        };
        if let Some(target) = targets.get(next).copied() {
            self.select_check_list_target(target);
        }
    }

    /// Every row in the overview sidebar shows a different document on the right,
    /// so a new selection always starts at the top of it.
    pub(super) fn set_check_cursor(&mut self, cursor: Option<usize>) -> bool {
        let next = cursor.filter(|index| *index < self.pull_request_checks.len());
        if next.is_some() {
            self.selected_check_section = None;
        }
        if self.pull_request_check_cursor == next {
            return false;
        }
        self.pull_request_check_cursor = next;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.invalidate_check_run_log();
        self.invalidate_pull_request_content_rows();
        true
    }

    /// The sidebar viewport for this frame. Wheel scrolling detaches the
    /// window from the selection so the list can be browsed without changing
    /// the preview; any selection movement reattaches it.
    pub(crate) fn sidebar_viewport(&mut self, cursor: usize, height: usize, length: usize) {
        if self.sidebar_last_cursor != Some(cursor) {
            self.sidebar_last_cursor = Some(cursor);
            self.sidebar_free_scroll = false;
        }
        if height == 0 || length == 0 {
            self.sidebar_offset = 0;
            return;
        }
        if !self.sidebar_free_scroll {
            if cursor < self.sidebar_offset {
                self.sidebar_offset = cursor;
            } else if cursor >= self.sidebar_offset.saturating_add(height) {
                self.sidebar_offset = cursor.saturating_add(1).saturating_sub(height);
            }
        }
        self.sidebar_offset = self.sidebar_offset.min(length.saturating_sub(height));
    }

    pub(crate) const fn reset_sidebar_scroll(&mut self) {
        self.sidebar_offset = 0;
        self.sidebar_free_scroll = false;
        self.sidebar_last_cursor = None;
    }

    pub(crate) fn set_document(&mut self, document: DiffDocument) {
        self.document = document;
        self.invalidate_diff_rows();
    }

    pub(crate) const fn invalidate_diff_rows(&mut self) {
        self.document_layout_generation = self.document_layout_generation.wrapping_add(1);
        self.diff_rows_key = None;
    }

    pub(super) const fn invalidate_pull_request_content_rows(&mut self) {
        self.pull_request_content_generation = self.pull_request_content_generation.wrapping_add(1);
        self.pull_request_content_rows_key = None;
    }

    pub(super) fn invalidate_check_run_log(&mut self) {
        self.pull_request_check_log = None;
        self.pull_request_check_log_error = None;
        self.pull_request_check_log_target = None;
        self.pull_request_check_log_loading = false;
        self.pull_request_log_read_at = None;
        self.expanded_check_steps.clear();
        self.pull_request_step_cursor = 0;
        self.pull_request_check_log_generation =
            self.pull_request_check_log_generation.wrapping_add(1);
    }

    pub(crate) fn check_log_visible(&self) -> bool {
        self.view == View::PullRequests
            && self.pull_request_section == PullRequestSection::Overview
            && self.pull_request_check_cursor.is_some()
    }

    pub(super) fn check_step_numbers(&self) -> Vec<usize> {
        self.pull_request_check_log
            .as_ref()
            .map(|log| log.steps.iter().map(|step| step.number).collect())
            .unwrap_or_default()
    }

    pub(crate) fn check_step_expanded(&self, step: usize) -> bool {
        self.expanded_check_steps.contains(&step)
    }

    pub(super) fn toggle_check_step(&mut self, step: usize) {
        toggle_membership(&mut self.expanded_check_steps, step);
        self.reveal_check_step(step);
        self.invalidate_pull_request_content_rows();
    }

    pub(super) fn toggle_all_check_steps(&mut self) {
        let steps = self.check_step_numbers();
        if self.expanded_check_steps.is_empty() {
            self.expanded_check_steps.extend(steps);
        } else {
            self.expanded_check_steps.clear();
        }
        self.content_scroll = 0;
        self.invalidate_pull_request_content_rows();
    }

    /// Move between steps the way `[` and `]` move between diff hunks, so a long
    /// log can be walked without scrolling through it.
    /// Move the step selection and ask the next draw to bring it into view.
    pub(super) const fn reveal_check_step(&mut self, step: usize) {
        self.pull_request_step_cursor = step;
        self.pull_request_step_reveal = true;
    }

    pub(super) fn move_check_step_cursor(&mut self, amount: isize) -> bool {
        let steps = self.check_step_numbers();
        if steps.is_empty() {
            return false;
        }
        let current = steps
            .iter()
            .position(|step| *step == self.pull_request_step_cursor)
            .unwrap_or_default();
        let next = if amount < 0 {
            current.saturating_sub(amount.unsigned_abs())
        } else {
            current.saturating_add(count(amount)).min(steps.len() - 1)
        };
        let Some(step) = steps.get(next).copied() else {
            return false;
        };
        self.reveal_check_step(step);
        true
    }

    pub(super) fn select_pull_request_check(
        &mut self,
        cursor: Option<usize>,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.set_check_cursor(cursor) {
            self.request_check_run_log(false, effects);
        }
    }

    /// Fetch the selected check's steps and log. A selection change starts from a
    /// clean slate; a live refresh of the same run updates in place so the reader
    /// keeps their scroll position while a job is still writing output. A log
    /// already held for the selected run is only re-read when `refresh` asks for
    /// it, so redrawing or re-entering the section costs nothing.
    pub(super) fn request_check_run_log(&mut self, refresh: bool, effects: &mut Vec<AppEffect>) {
        let (Some(pull_request), Some(check)) = (
            self.pull_request.clone(),
            self.selected_pull_request_check().cloned(),
        ) else {
            self.pull_request_check_log = None;
            self.pull_request_check_log_error = None;
            self.pull_request_check_log_loading = false;
            self.pull_request_check_log_target = None;
            self.pull_request_check_log_generation =
                self.pull_request_check_log_generation.wrapping_add(1);
            return;
        };
        let target = check.identity();
        if self.pull_request_check_log_target.as_ref() == Some(&target) {
            if self.pull_request_check_log_loading {
                return;
            }
            let held = self.pull_request_check_log.is_some()
                || self.pull_request_check_log_error.is_some();
            if held && !refresh {
                return;
            }
        } else {
            self.pull_request_check_log = None;
            self.pull_request_check_log_error = None;
            self.expanded_check_steps.clear();
            self.pull_request_check_log_target = Some(target);
            self.pull_request_log_read_at = None;
        }
        self.pull_request_check_log_generation =
            self.pull_request_check_log_generation.wrapping_add(1);
        self.pull_request_check_log_loading = true;
        if self.pull_request_check_log.is_none() {
            self.invalidate_pull_request_content_rows();
        }
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadCheckRunLog {
            generation: self.pull_request_check_log_generation,
            pull_request: Box::new(pull_request),
            check: Box::new(check),
        })));
    }

    /// Warm every finished run's log once per pull request. Selecting a check
    /// then costs a disk read rather than a round trip, which is the difference
    /// between the list being browsable and being a series of waits.
    pub(super) fn request_check_log_prefetch(&mut self, effects: &mut Vec<AppEffect>) {
        let Some(pull_request) = self.pull_request.clone() else {
            return;
        };
        let settled: Vec<PullRequestCheck> = self
            .pull_request_checks
            .iter()
            .filter(|check| !check.status.is_running() && check.job_id().is_some())
            .filter(|check| {
                !self
                    .pull_request_prefetched_logs
                    .contains(&check.identity())
            })
            .take(32_usize.saturating_sub(self.pull_request_prefetched_logs.len()))
            .cloned()
            .collect();
        if settled.is_empty() {
            return;
        }
        self.pull_request_prefetched_logs
            .extend(settled.iter().map(PullRequestCheck::identity));
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::PrefetchCheckRunLogs {
                generation: 0,
                pull_request: Box::new(pull_request),
                checks: settled,
            },
        )));
    }

    pub(super) fn request_pull_request_conversation(
        &mut self,
        refresh: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.pull_request_conversation_loading {
            self.pull_request_conversation_refresh_again |= refresh;
            return;
        }
        if !refresh && !self.pull_request_conversation.entries.is_empty() {
            return;
        }
        let Some(pull_request) = self.pull_request.clone() else {
            return;
        };
        self.pull_request_conversation_generation =
            self.pull_request_conversation_generation.wrapping_add(1);
        self.pull_request_conversation_loading = true;
        if self.pull_request_conversation.entries.is_empty() {
            self.invalidate_pull_request_content_rows();
        }
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestConversation {
                generation: self.pull_request_conversation_generation,
                pull_request: Box::new(pull_request),
            },
        )));
    }
}
