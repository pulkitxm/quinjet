#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn handle_stack_inspector_hit(
        &mut self,
        hit: StackInspectorHit,
        now: Instant,
        effects: &mut Vec<AppEffect>,
    ) {
        self.set_focus(Focus::Content, effects);
        match hit {
            StackInspectorHit::Section(section) => {
                self.select_stack_member_section(section, effects);
            }
            StackInspectorHit::TipChecks => self.inspect_pull_request_stack_tip(now, effects),
            StackInspectorHit::Diff => self.open_pull_request_stack_diff(effects),
        }
    }

    pub(super) fn handle_sidebar_hit(
        &mut self,
        hit: SidebarHit,
        now: Instant,
        effects: &mut Vec<AppEffect>,
    ) {
        match hit {
            SidebarHit::ChangeSection(section) => {
                self.auxiliary_preview = None;
                self.toggle_change_section(section);
                self.schedule_preview(now);
            }
            SidebarHit::Change(index) => {
                if let Some(cursor) = self
                    .visible_change_indices()
                    .iter()
                    .position(|visible| *visible == index)
                {
                    self.auxiliary_preview = None;
                    self.selected_change_section = None;
                    self.change_cursor = cursor;
                    self.schedule_preview(now);
                }
            }
            SidebarHit::Commit(index) => {
                if let Some(cursor) = self
                    .visible_commit_indices()
                    .iter()
                    .position(|visible| *visible == index)
                {
                    self.history_cursor = cursor;
                    self.schedule_preview(now);
                }
            }
            SidebarHit::PullRequestFiles => {
                self.select_pull_request_section(PullRequestSection::Files, effects);
            }
            SidebarHit::PullRequestOverview => {
                self.select_pull_request_section(PullRequestSection::Overview, effects);
            }
            SidebarHit::PullRequestStack => {
                self.select_pull_request_section(PullRequestSection::Stack, effects);
            }
            SidebarHit::PullRequestStackMember(position) => {
                let _ = self.select_pull_request_stack_member(position, false, now);
            }
            SidebarHit::PullRequestConversation => {
                self.selected_check_section = None;
                self.select_pull_request_check(None, effects);
            }
            SidebarHit::PullRequestCheckSection(section) => {
                self.toggle_check_section(section);
                self.schedule_preview(now);
            }
            SidebarHit::PullRequestChooseRepository => self.open_pull_request_repositories(effects),
            SidebarHit::PullRequestLookup => self.pull_request_lookup_active = true,
            SidebarHit::RecentPullRequest(index) => {
                let _ = self.open_recent_pull_request(index, effects);
            }
            SidebarHit::PullRequestDirectory(path) => {
                self.toggle_pull_request_directory(path);
            }
            SidebarHit::PullRequestFile(index) => {
                if let Some(cursor) = self.pull_request_tree_entries().iter().position(|entry| {
                    matches!(
                        entry,
                        PullRequestTreeEntry::File {
                            index: entry_index,
                            ..
                        } if *entry_index == index
                    )
                }) {
                    self.select_pull_request_tree_entry(cursor, now);
                }
            }
            SidebarHit::PullRequestCheck(index) => {
                if index < self.pull_request_checks.len() {
                    self.select_pull_request_check(Some(index), effects);
                }
            }
        }
    }
}
