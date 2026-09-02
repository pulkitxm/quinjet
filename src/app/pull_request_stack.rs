#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(crate) fn pull_request_stack_range(&self) -> Option<(usize, usize)> {
        let stack = self.pull_request_stack.as_ref()?;
        let anchor = self
            .pull_request_stack_anchor
            .unwrap_or(stack.selected_position);
        let cursor = self
            .pull_request_stack_cursor
            .unwrap_or(stack.selected_position);
        if stack.member(anchor).is_none() || stack.member(cursor).is_none() {
            return None;
        }
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    pub(super) fn pull_request_diff_source_for_section(&self) -> Option<PullRequestDiffSource> {
        match self.pull_request_section {
            PullRequestSection::Overview | PullRequestSection::Files => {
                Some(PullRequestDiffSource::PullRequest)
            }
            PullRequestSection::Stack => self
                .pull_request_stack_range()
                .map(|(from, to)| PullRequestDiffSource::Stack { from, to }),
        }
    }

    pub(super) fn pull_request_for_diff_source(
        &self,
        source: PullRequestDiffSource,
    ) -> Option<PullRequest> {
        match source {
            PullRequestDiffSource::PullRequest => self.pull_request.clone(),
            PullRequestDiffSource::Stack { from, to } => {
                self.pull_request_stack.as_ref()?.comparison(from, to).ok()
            }
        }
    }

    pub(super) fn apply_pull_request_stack_snapshot(
        &mut self,
        stack: Option<PullRequestStack>,
        effects: &mut Vec<AppEffect>,
    ) {
        let previous_selection = self
            .pull_request_stack_anchor
            .zip(self.pull_request_stack_cursor);
        let previous = self.pull_request_stack.take();
        let changed = previous.as_ref() != stack.as_ref();
        self.pull_request_stack = stack;
        let workspace_changed = previous.as_ref().map(|stack| &stack.node_id)
            != self.pull_request_stack.as_ref().map(|stack| &stack.node_id);
        let selection = self.pull_request_stack.as_ref().map(|current| {
            previous_selection
                .filter(|(anchor, cursor)| {
                    current.member(*anchor).is_some() && current.member(*cursor).is_some()
                })
                .filter(|_| {
                    previous
                        .as_ref()
                        .is_some_and(|old| old.node_id == current.node_id)
                })
                .unwrap_or((current.selected_position, current.selected_position))
        });
        self.pull_request_stack_anchor = selection.map(|(anchor, _)| anchor);
        self.pull_request_stack_cursor = selection.map(|(_, cursor)| cursor);
        self.pull_request_stack_error = None;
        if changed {
            self.invalidate_stack_inspector_content_rows();
        }
        if workspace_changed {
            self.reset_view_sidebar_scroll(View::PullRequests);
            self.request_pull_request_stack_prefetch(effects);
        }
        if self.pull_request_stack.is_some() {
            self.pull_request_section = PullRequestSection::Stack;
            self.sidebar_hidden = false;
            self.pull_request_lookup_active = false;
            self.pr_menu_open = false;
        }
        self.reconcile_stack_inspector();
        if changed
            && self.pull_request_stack.is_some()
            && self.pull_request_section == PullRequestSection::Stack
        {
            self.invalidate_preview();
            self.reset_pull_request_diff_runtime();
        }
        if self.pull_request_stack.is_none()
            && self.pull_request_section == PullRequestSection::Stack
        {
            self.pull_request_section = PullRequestSection::Overview;
            self.invalidate_preview();
            self.reset_pull_request_diff_runtime();
            self.pull_request_progress = None;
            self.request_pull_request_checks(true, effects);
            self.request_pull_request_conversation(true, effects);
            self.request_pull_request_review(true, effects);
            return;
        }
        self.request_stack_inspector(self.pull_request_lookup_refresh, effects);
    }

    fn request_pull_request_stack_prefetch(&self, effects: &mut Vec<AppEffect>) {
        let pull_requests = self
            .pull_request_stack
            .as_ref()
            .map_or_else(Vec::new, |stack| {
                let selected = self.pull_request_stack_cursor;
                stack
                    .members
                    .iter()
                    .filter(|member| Some(member.position) == selected)
                    .chain(
                        stack
                            .members
                            .iter()
                            .filter(|member| Some(member.position) != selected),
                    )
                    .filter_map(|member| stack.member_pull_request(member.position))
                    .collect()
            });
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::PrefetchPullRequestStackMembers {
                generation: 0,
                pull_requests,
            },
        )));
    }

    pub(crate) fn selected_pull_request_stack_member(&self) -> Option<&PullRequestStackMember> {
        let position = self.pull_request_stack_cursor?;
        self.pull_request_stack.as_ref()?.member(position)
    }

    pub(super) fn select_pull_request_stack_member(
        &mut self,
        position: usize,
        extend: bool,
        now: Instant,
    ) -> bool {
        if self
            .pull_request_stack
            .as_ref()
            .is_none_or(|stack| stack.member(position).is_none())
        {
            return false;
        }
        let previous = self.pull_request_stack_range();
        if !extend {
            self.pull_request_stack_anchor = Some(position);
        }
        self.pull_request_stack_cursor = Some(position);
        if self.pull_request_stack_range() == previous {
            return false;
        }
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.reset_pull_request_review();
        self.reconcile_stack_inspector();
        if self.stack_inspector.diff_open
            || self.stack_inspector.section == StackMemberSection::Files
        {
            self.schedule_preview(now);
        }
        true
    }

    pub(super) fn move_pull_request_stack_cursor(
        &mut self,
        amount: isize,
        extend: bool,
        now: Instant,
    ) -> bool {
        let Some(stack) = self.pull_request_stack.as_ref() else {
            return false;
        };
        let positions = stack
            .members
            .iter()
            .map(|member| member.position)
            .collect::<Vec<_>>();
        let current = self
            .pull_request_stack_cursor
            .and_then(|position| {
                positions
                    .iter()
                    .position(|candidate| *candidate == position)
            })
            .unwrap_or_default();
        let next = if amount < 0 {
            current.saturating_sub(amount.unsigned_abs())
        } else {
            (current + count(amount)).min(positions.len().saturating_sub(1))
        };
        positions
            .get(next)
            .copied()
            .is_some_and(|position| self.select_pull_request_stack_member(position, extend, now))
    }

    pub(super) fn move_pull_request_stack_to_edge(&mut self, end: bool, now: Instant) -> bool {
        let position = self.pull_request_stack.as_ref().and_then(|stack| {
            if end {
                stack.members.last()
            } else {
                stack.members.first()
            }
            .map(|member| member.position)
        });
        position.is_some_and(|position| self.select_pull_request_stack_member(position, false, now))
    }

    pub(super) fn pull_request_stack_hit_at(&self, column: u16, row: u16) -> Option<usize> {
        self.geometry
            .sidebar_hits
            .iter()
            .find(|hit| hit.area.contains((column, row).into()))
            .and_then(|hit| match hit.target {
                SidebarHit::PullRequestStackMember(position) => Some(position),
                _ => None,
            })
    }

    pub(super) fn open_pull_request_stack_diff(&mut self, effects: &mut Vec<AppEffect>) {
        if self.pull_request_stack.is_none() {
            return;
        }
        self.pull_request_section = PullRequestSection::Stack;
        self.stack_inspector.diff_open = true;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.request_preview(effects);
    }

    pub(super) fn close_pull_request_stack_diff(&mut self, effects: &mut Vec<AppEffect>) -> bool {
        if !self.stack_inspector.diff_open {
            return false;
        }
        self.stack_inspector.diff_open = false;
        self.pull_request_stack_anchor = self.pull_request_stack_cursor;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        if self.stack_inspector.section == StackMemberSection::Files {
            self.request_preview(effects);
        }
        true
    }

    pub(super) fn open_stack_member_review(&mut self, effects: &mut Vec<AppEffect>, now: Instant) {
        self.handle_pr_menu_item(PrMenuItem::Review, effects, now);
    }

    pub(super) fn inspect_pull_request_stack_tip(
        &mut self,
        now: Instant,
        effects: &mut Vec<AppEffect>,
    ) {
        let tip = self
            .pull_request_stack
            .as_ref()
            .and_then(PullRequestStack::tip)
            .map(|member| member.position);
        if let Some(position) = tip {
            let _ = self.select_pull_request_stack_member(position, false, now);
            self.select_stack_member_section(StackMemberSection::Checks, effects);
        }
    }
}
