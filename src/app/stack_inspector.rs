use std::time::Instant;

use crate::git::github::{
    PullRequest, PullRequestChecks, PullRequestCommits, PullRequestConversation,
    PullRequestStackMemberIdentity,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum StackMemberSection {
    #[default]
    Files,
    Summary,
    Conversation,
    Checks,
    Commits,
}

#[derive(Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag tracks an independent worker stream state"
)]
pub(crate) struct StackInspector {
    pub selected_identity: Option<PullRequestStackMemberIdentity>,
    pub selected_locator: Option<PullRequest>,
    pub selected_pull_request: Option<PullRequest>,
    pub selected_from_cache: bool,
    pub selected_loading: bool,
    pub selected_refresh_again: bool,
    pub selected_error: Option<String>,
    pub selected_generation: u64,
    pub section: StackMemberSection,
    pub diff_open: bool,
    pub content_generation: u64,
    pub conversation: PullRequestConversation,
    pub conversation_loaded: bool,
    pub conversation_loading: bool,
    pub conversation_refresh_again: bool,
    pub conversation_error: Option<String>,
    pub conversation_generation: u64,
    pub checks: PullRequestChecks,
    pub checks_loaded: bool,
    pub checks_loading: bool,
    pub checks_refresh_again: bool,
    pub checks_error: Option<String>,
    pub checks_generation: u64,
    pub commits: PullRequestCommits,
    pub commits_loaded: bool,
    pub commits_loading: bool,
    pub commits_error: Option<String>,
    pub commits_generation: u64,
    pub detail_read_at: Option<Instant>,
    pub checks_read_at: Option<Instant>,
    pub tip_identity: Option<PullRequestStackMemberIdentity>,
    pub tip_locator: Option<PullRequest>,
    pub tip_checks: PullRequestChecks,
    pub tip_checks_loaded: bool,
    pub tip_checks_loading: bool,
    pub tip_checks_refresh_again: bool,
    pub tip_checks_error: Option<String>,
    pub tip_checks_generation: u64,
    pub tip_checks_read_at: Option<Instant>,
    pub sync_due: bool,
}

impl StackInspector {
    pub(crate) fn selected_uses_tip_checks(&self) -> bool {
        self.selected_identity.is_some() && self.selected_identity == self.tip_identity
    }

    pub(crate) fn selected_checks(&self) -> &PullRequestChecks {
        if self.selected_uses_tip_checks() {
            &self.tip_checks
        } else {
            &self.checks
        }
    }

    pub(crate) fn selected_checks_loaded(&self) -> bool {
        if self.selected_uses_tip_checks() {
            self.tip_checks_loaded
        } else {
            self.checks_loaded
        }
    }

    pub(crate) fn selected_checks_loading(&self) -> bool {
        if self.selected_uses_tip_checks() {
            self.tip_checks_loading
        } else {
            self.checks_loading
        }
    }

    pub(crate) fn selected_checks_error(&self) -> Option<&str> {
        if self.selected_uses_tip_checks() {
            self.tip_checks_error.as_deref()
        } else {
            self.checks_error.as_deref()
        }
    }

    pub(crate) fn select(
        &mut self,
        identity: PullRequestStackMemberIdentity,
        locator: PullRequest,
    ) -> bool {
        if self.selected_identity.as_ref() == Some(&identity) {
            if self.selected_locator.as_ref() != Some(&locator) {
                self.clear_selected();
                self.selected_identity = Some(identity);
                self.selected_locator = Some(locator);
                self.sync_due = true;
                return true;
            }
            return false;
        }
        self.clear_selected();
        self.selected_identity = Some(identity);
        self.selected_locator = Some(locator);
        self.sync_due = true;
        true
    }

    pub(crate) fn select_tip(
        &mut self,
        identity: PullRequestStackMemberIdentity,
        locator: PullRequest,
    ) -> bool {
        if self.tip_identity.as_ref() == Some(&identity) {
            if self.tip_locator.as_ref() != Some(&locator) {
                self.clear_tip();
                self.tip_identity = Some(identity);
                self.tip_locator = Some(locator);
                self.sync_due = true;
                return true;
            }
            return false;
        }
        self.clear_tip();
        self.tip_identity = Some(identity);
        self.tip_locator = Some(locator);
        self.sync_due = true;
        true
    }

    pub(crate) fn clear(&mut self) {
        self.clear_selected();
        self.clear_tip();
        self.section = StackMemberSection::Files;
        self.diff_open = false;
        self.sync_due = false;
    }

    fn clear_selected(&mut self) {
        self.selected_identity = None;
        self.selected_locator = None;
        self.selected_pull_request = None;
        self.selected_from_cache = false;
        self.selected_loading = false;
        self.selected_refresh_again = false;
        self.selected_error = None;
        self.selected_generation = self.selected_generation.wrapping_add(1);
        self.conversation = PullRequestConversation::default();
        self.conversation_loaded = false;
        self.conversation_loading = false;
        self.conversation_refresh_again = false;
        self.conversation_error = None;
        self.conversation_generation = self.conversation_generation.wrapping_add(1);
        self.checks = PullRequestChecks::default();
        self.checks_loaded = false;
        self.checks_loading = false;
        self.checks_refresh_again = false;
        self.checks_error = None;
        self.checks_generation = self.checks_generation.wrapping_add(1);
        self.commits = PullRequestCommits::default();
        self.commits_loaded = false;
        self.commits_loading = false;
        self.commits_error = None;
        self.commits_generation = self.commits_generation.wrapping_add(1);
        self.content_generation = self.content_generation.wrapping_add(1);
        self.detail_read_at = None;
        self.checks_read_at = None;
    }

    fn clear_tip(&mut self) {
        self.tip_identity = None;
        self.tip_locator = None;
        self.tip_checks = PullRequestChecks::default();
        self.tip_checks_loaded = false;
        self.tip_checks_loading = false;
        self.tip_checks_refresh_again = false;
        self.tip_checks_error = None;
        self.tip_checks_generation = self.tip_checks_generation.wrapping_add(1);
        self.tip_checks_read_at = None;
    }
}

impl super::App {
    pub(super) const fn invalidate_stack_inspector_content_rows(&mut self) {
        self.stack_inspector.content_generation =
            self.stack_inspector.content_generation.wrapping_add(1);
        self.stack_inspector_content_rows_key = None;
    }

    pub(super) fn reconcile_stack_inspector(&mut self) {
        if self.pull_request_stack.is_none() {
            self.stack_inspector.clear();
            return;
        }
        let selected = self.pull_request_stack.as_ref().and_then(|stack| {
            let position = self.pull_request_stack_cursor?;
            Some((
                stack.member_identity(position)?,
                stack.member_pull_request(position)?,
            ))
        });
        let tip = self.pull_request_stack.as_ref().and_then(|stack| {
            let position = stack.tip()?.position;
            Some((
                stack.member_identity(position)?,
                stack.member_pull_request(position)?,
            ))
        });
        match selected {
            Some((identity, locator)) => {
                let _ = self.stack_inspector.select(identity, locator);
            }
            None => self.stack_inspector.clear_selected(),
        }
        match tip {
            Some((identity, locator)) => {
                let _ = self.stack_inspector.select_tip(identity, locator);
            }
            None => self.stack_inspector.clear_tip(),
        }
    }

    pub(super) fn request_stack_inspector(
        &mut self,
        refresh: bool,
        effects: &mut Vec<super::AppEffect>,
    ) {
        self.stack_inspector.sync_due = false;
        self.request_stack_tip_checks(refresh, effects);
        match self.stack_inspector.section {
            StackMemberSection::Files if self.view == super::View::PullRequests => {
                self.request_preview(effects);
                self.request_pull_request_review(refresh, effects);
            }
            StackMemberSection::Files => {}
            StackMemberSection::Summary => self.request_stack_member(refresh, effects),
            StackMemberSection::Conversation => {
                self.request_stack_member_conversation(refresh, effects);
            }
            StackMemberSection::Checks
                if self.stack_inspector.selected_identity != self.stack_inspector.tip_identity =>
            {
                self.request_stack_member_checks(refresh, effects);
            }
            StackMemberSection::Checks => {}
            StackMemberSection::Commits => self.request_stack_member_commits(effects),
        }
    }

    pub(super) fn refresh_stack_inspector_live(
        &mut self,
        now: Instant,
        force: bool,
        effects: &mut Vec<super::AppEffect>,
    ) {
        let due = |last: Option<Instant>, interval: std::time::Duration| {
            force || last.is_none_or(|last| now.duration_since(last) >= interval)
        };
        if due(
            self.stack_inspector.tip_checks_read_at,
            self.pull_request_poll_interval(),
        ) {
            let issued = effects.len();
            self.request_stack_tip_checks(true, effects);
            if effects.len() > issued {
                self.stack_inspector.tip_checks_read_at = Some(now);
            }
        }
        match self.stack_inspector.section {
            StackMemberSection::Files => {}
            StackMemberSection::Summary | StackMemberSection::Conversation
                if due(
                    self.stack_inspector.detail_read_at,
                    super::PULL_REQUEST_DETAIL_POLL,
                ) =>
            {
                let issued = effects.len();
                match self.stack_inspector.section {
                    StackMemberSection::Files => {}
                    StackMemberSection::Summary => self.request_stack_member(true, effects),
                    StackMemberSection::Conversation => {
                        self.request_stack_member_conversation(true, effects);
                    }
                    StackMemberSection::Checks | StackMemberSection::Commits => {}
                }
                if effects.len() > issued {
                    self.stack_inspector.detail_read_at = Some(now);
                }
            }
            StackMemberSection::Checks
                if due(
                    self.stack_inspector.checks_read_at,
                    self.pull_request_poll_interval(),
                ) && self.stack_inspector.selected_identity
                    != self.stack_inspector.tip_identity =>
            {
                let issued = effects.len();
                self.request_stack_member_checks(true, effects);
                if effects.len() > issued {
                    self.stack_inspector.checks_read_at = Some(now);
                }
            }
            StackMemberSection::Summary
            | StackMemberSection::Conversation
            | StackMemberSection::Checks
            | StackMemberSection::Commits => {}
        }
    }

    pub(super) fn select_stack_member_section(
        &mut self,
        section: StackMemberSection,
        effects: &mut Vec<super::AppEffect>,
    ) {
        if self.stack_inspector.section != section {
            self.stack_inspector.section = section;
            self.invalidate_stack_inspector_content_rows();
            self.reset_view_content_position(super::View::PullRequests);
        }
        self.stack_inspector.diff_open = false;
        if section == StackMemberSection::Files {
            self.pull_request_stack_anchor = self.pull_request_stack_cursor;
        }
        self.request_stack_inspector(false, effects);
    }

    fn stack_member_request(&self) -> Option<(PullRequestStackMemberIdentity, PullRequest)> {
        Some((
            self.stack_inspector.selected_identity.clone()?,
            self.stack_inspector.selected_locator.clone()?,
        ))
    }

    pub(super) fn request_stack_member(
        &mut self,
        refresh: bool,
        effects: &mut Vec<super::AppEffect>,
    ) {
        if self.stack_inspector.selected_loading {
            self.stack_inspector.selected_refresh_again |= refresh;
            return;
        }
        if !refresh && self.stack_inspector.selected_pull_request.is_some() {
            return;
        }
        let Some((identity, pull_request)) = self.stack_member_request() else {
            return;
        };
        self.stack_inspector.selected_generation =
            self.stack_inspector.selected_generation.wrapping_add(1);
        self.stack_inspector.selected_loading = true;
        effects.push(super::AppEffect::Git(Box::new(
            crate::git::worker::WorkerCommand::LoadPullRequestStackMember {
                identity,
                generation: self.stack_inspector.selected_generation,
                pull_request: Box::new(pull_request),
                refresh,
            },
        )));
    }

    pub(super) fn request_stack_member_conversation(
        &mut self,
        refresh: bool,
        effects: &mut Vec<super::AppEffect>,
    ) {
        if self.stack_inspector.conversation_loading {
            self.stack_inspector.conversation_refresh_again |= refresh;
            return;
        }
        if !refresh && self.stack_inspector.conversation_loaded {
            return;
        }
        let Some((identity, pull_request)) = self.stack_member_request() else {
            return;
        };
        self.stack_inspector.conversation_generation =
            self.stack_inspector.conversation_generation.wrapping_add(1);
        self.stack_inspector.conversation_loading = true;
        effects.push(super::AppEffect::Git(Box::new(
            crate::git::worker::WorkerCommand::LoadPullRequestStackMemberConversation {
                identity,
                generation: self.stack_inspector.conversation_generation,
                pull_request: Box::new(pull_request),
            },
        )));
    }

    pub(super) fn request_stack_member_checks(
        &mut self,
        refresh: bool,
        effects: &mut Vec<super::AppEffect>,
    ) {
        if self.stack_inspector.checks_loading {
            self.stack_inspector.checks_refresh_again |= refresh;
            return;
        }
        if !refresh && self.stack_inspector.checks_loaded {
            return;
        }
        let Some((identity, pull_request)) = self.stack_member_request() else {
            return;
        };
        self.stack_inspector.checks_generation =
            self.stack_inspector.checks_generation.wrapping_add(1);
        self.stack_inspector.checks_loading = true;
        effects.push(super::AppEffect::Git(Box::new(
            crate::git::worker::WorkerCommand::LoadPullRequestStackMemberChecks {
                identity,
                generation: self.stack_inspector.checks_generation,
                pull_request: Box::new(pull_request),
                refresh,
            },
        )));
    }

    fn request_stack_member_commits(&mut self, effects: &mut Vec<super::AppEffect>) {
        if self.stack_inspector.commits_loading || self.stack_inspector.commits_loaded {
            return;
        }
        let Some((identity, pull_request)) = self.stack_member_request() else {
            return;
        };
        self.stack_inspector.commits_generation =
            self.stack_inspector.commits_generation.wrapping_add(1);
        self.stack_inspector.commits_loading = true;
        effects.push(super::AppEffect::Git(Box::new(
            crate::git::worker::WorkerCommand::LoadPullRequestStackMemberCommits {
                identity,
                generation: self.stack_inspector.commits_generation,
                pull_request: Box::new(pull_request),
            },
        )));
    }

    pub(super) fn request_stack_tip_checks(
        &mut self,
        refresh: bool,
        effects: &mut Vec<super::AppEffect>,
    ) {
        if self.stack_inspector.tip_checks_loading {
            self.stack_inspector.tip_checks_refresh_again |= refresh;
            return;
        }
        if !refresh && self.stack_inspector.tip_checks_loaded {
            return;
        }
        let (Some(identity), Some(pull_request)) = (
            self.stack_inspector.tip_identity.clone(),
            self.stack_inspector.tip_locator.clone(),
        ) else {
            return;
        };
        self.stack_inspector.tip_checks_generation =
            self.stack_inspector.tip_checks_generation.wrapping_add(1);
        self.stack_inspector.tip_checks_loading = true;
        effects.push(super::AppEffect::Git(Box::new(
            crate::git::worker::WorkerCommand::LoadPullRequestStackTipChecks {
                identity,
                generation: self.stack_inspector.tip_checks_generation,
                pull_request: Box::new(pull_request),
                refresh,
            },
        )));
    }
}
