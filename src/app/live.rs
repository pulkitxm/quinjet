#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(crate) fn tick(&mut self, now: Instant) -> (Vec<AppEffect>, bool) {
        let toast_expired = self
            .toast
            .as_ref()
            .is_some_and(|toast| now >= toast.expires_at);
        let mut changed = toast_expired;
        if toast_expired {
            self.toast = None;
        }
        if self
            .pending_g
            .is_some_and(|pressed| now.duration_since(pressed) >= Duration::from_millis(500))
        {
            self.pending_g = None;
        }

        let mut effects = Vec::new();
        if self.preview_due.is_some_and(|due| now >= due) {
            self.preview_due = None;
            self.request_preview(&mut effects);
            changed = true;
        }
        if self.pull_request_poll_due.is_some_and(|due| now >= due) {
            self.refresh_pull_request_live(now, false, &mut effects);
            changed = true;
        }
        if self.busy.is_some() {
            self.operation_frame = self.operation_frame.wrapping_add(1) % OPERATION_SPINNER.len();
            changed = true;
        }
        (effects, changed)
    }

    pub(crate) fn operation_spinner(&self) -> &'static str {
        OPERATION_SPINNER
            .get(self.operation_frame % OPERATION_SPINNER.len())
            .copied()
            .unwrap_or("◐")
    }

    #[doc = " A GitHub webhook was forwarded to this session. The payload is only a"]
    #[doc = " hint that something changed, so the poller runs immediately rather than"]
    #[doc = " trying to apply the delivery itself."]
    pub(crate) fn webhook_delivered(&mut self, now: Instant) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        if self.pull_request.is_some() {
            self.refresh_pull_request_live(now, true, &mut effects);
        }
        effects
    }

    #[doc = " Whether any pull-request read is on its way. The view shows one status"]
    #[doc = " label for all of them, because the reader cares that it is refreshing,"]
    #[doc = " not which of four endpoints is answering."]
    pub(crate) const fn pull_request_refreshing(&self) -> bool {
        self.pull_request_loading
            || self.pull_request_checks_loading
            || self.pull_request_conversation_loading
            || self.pull_request_review_loading
            || self.pull_request_check_log_loading
    }

    #[doc = " Whether the pull request itself was answered from disk rather than the"]
    #[doc = " network. Check state is deliberately held for only thirty seconds, so"]
    #[doc = " including it here made the answer almost always false and the label"]
    #[doc = " never appeared at all."]
    pub(crate) const fn pull_request_served_from_cache(&self) -> bool {
        self.pull_request.is_some()
            && self.pull_request_from_cache
            && self.pull_request_conversation.from_cache
            && !self.pull_request_refreshing()
    }

    #[doc = " Watch a running pull request closely and a settled one loosely. The"]
    #[doc = " interval also stretches when the reader is somewhere else, so a loaded"]
    #[doc = " pull request stays fresh without spending requests on an unseen pane."]
    pub(super) fn pull_request_poll_interval(&self) -> Duration {
        if !self.tab_active || self.view != View::PullRequests {
            return PULL_REQUEST_BACKGROUND_POLL;
        }
        if self
            .pull_request_checks
            .iter()
            .any(|check| check.status.is_running())
        {
            PULL_REQUEST_ACTIVE_POLL
        } else {
            PULL_REQUEST_IDLE_POLL
        }
    }

    pub(super) fn schedule_pull_request_poll(&mut self, now: Instant) {
        self.pull_request_poll_due = self
            .pull_request
            .is_some()
            .then(|| now + self.pull_request_poll_interval());
    }

    #[doc = " Run whichever live reads are due. `force` is a webhook delivery saying"]
    #[doc = " something definitely changed, so every stream reads at once."]
    #[doc = ""]
    #[doc = " Each read is independent, so a single failing endpoint never stalls the"]
    #[doc = " others, and every one of them coalesces if a previous poll is still in"]
    #[doc = " flight."]
    pub(super) fn refresh_pull_request_live(
        &mut self,
        now: Instant,
        force: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        self.schedule_pull_request_poll(now);
        let Some(number) = self.pull_request_exact_number else {
            return;
        };
        if self.pull_request.is_none() {
            return;
        }
        let due = |last: Option<Instant>, interval: Duration| {
            force || last.is_none_or(|last| now.duration_since(last) >= interval)
        };

        if due(
            self.pull_request_checks_read_at,
            self.pull_request_poll_interval(),
        ) {
            let issued = effects.len();
            self.request_pull_request_checks(true, effects);
            if effects.len() > issued {
                self.pull_request_checks_read_at = Some(now);
            }
        }
        let settled = self
            .pull_request
            .as_ref()
            .is_some_and(|pull_request| matches!(pull_request.state.as_str(), "MERGED" | "CLOSED"));
        if settled && !force {
            return;
        }
        if due(self.pull_request_detail_read_at, PULL_REQUEST_DETAIL_POLL) {
            let issued = effects.len();
            self.request_pull_request_lookup(number, true, true, effects);
            self.request_pull_request_conversation(true, effects);
            self.request_pull_request_review(true, effects);
            if effects.len() > issued {
                self.pull_request_detail_read_at = Some(now);
            }
        }
        let running = self
            .selected_pull_request_check()
            .is_some_and(|check| check.status.is_running());
        if running && due(self.pull_request_log_read_at, PULL_REQUEST_LOG_POLL) {
            let issued = effects.len();
            self.request_check_run_log(true, effects);
            if effects.len() > issued {
                self.pull_request_log_read_at = Some(now);
            }
        }
    }

    pub(crate) fn filesystem_changed(&mut self, effects: &mut Vec<AppEffect>) {
        self.changes_diff_version = self.changes_diff_version.wrapping_add(1);
        if self.view == View::Changes && self.auxiliary_preview.is_none() {
            self.invalidate_preview();
            self.local_diff_loading_path = None;
            self.local_diff_pending_paths.clear();
        }
        self.request_refresh(effects);
    }

    #[doc = " The repository heartbeat. Pull-request liveness is separate because it"]
    #[doc = " paces itself against GitHub rather than the local working tree."]
    pub(crate) fn periodic_refresh(&mut self, effects: &mut Vec<AppEffect>) {
        self.request_refresh(effects);
    }
}
