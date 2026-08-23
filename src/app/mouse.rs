#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::excessive_nesting,
        reason = "the key handler mirrors the shape of the input it decodes"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn handle_mouse(&mut self, event: MouseEvent, now: Instant) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        self.link_hover = (event.kind == MouseEventKind::Moved
            && event.modifiers.intersects(
                KeyModifiers::CONTROL
                    | KeyModifiers::SUPER
                    | KeyModifiers::META
                    | KeyModifiers::HYPER,
            )
            && self
                .geometry
                .link_hits
                .iter()
                .any(|hit| hit.area.contains((event.column, event.row).into())))
        .then_some((event.column, event.row));
        if let Some(effects) = self.handle_repository_tab_mouse(event) {
            return effects;
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left)
            && let Some(common_dir) = self
                .geometry
                .project_collapse_hits
                .iter()
                .find(|(area, _)| area.contains((event.column, event.row).into()))
                .map(|(_, common_dir)| common_dir.clone())
            && let Some(Modal::Projects {
                groups,
                selected,
                query,
                collapsed,
                ..
            }) = self.modal.as_mut()
        {
            toggle_membership(collapsed, common_dir);
            let visible = Self::filtered_project_rows(groups, &query.value, collapsed);
            *selected = (*selected).min(visible.len().saturating_sub(1));
            return effects;
        }
        if let Some(Modal::Help {
            selected, hover, ..
        }) = &mut self.modal
        {
            let point = (event.column, event.row).into();
            match event.kind {
                MouseEventKind::Moved => {
                    *hover = self
                        .geometry
                        .help_hits
                        .iter()
                        .find(|hit| hit.area.contains(point))
                        .map(|hit| hit.index);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(index) = self
                        .geometry
                        .help_hits
                        .iter()
                        .find(|hit| hit.area.contains(point))
                        .map(|hit| hit.index)
                    {
                        *selected = index;
                        *hover = Some(index);
                    }
                }
                MouseEventKind::ScrollUp => {
                    *selected = previous_list_index(*selected, crate::ui::help_shortcut_count());
                    *hover = None;
                }
                MouseEventKind::ScrollDown => {
                    *selected = next_list_index(*selected, crate::ui::help_shortcut_count());
                    *hover = None;
                }
                _ => {}
            }
            return effects;
        }
        if matches!(
            self.modal,
            Some(
                Modal::Commit { .. }
                    | Modal::Confirm { .. }
                    | Modal::Projects { .. }
                    | Modal::PullRequestActions { .. }
                    | Modal::PullRequestReviewThreadActions { .. }
            )
        ) && event.kind == MouseEventKind::Down(MouseButton::Left)
        {
            if let Some(action) = self
                .geometry
                .modal_action_hits
                .iter()
                .find(|(area, _)| area.contains((event.column, event.row).into()))
                .map(|(_, action)| *action)
            {
                self.handle_modal_action(action, &mut effects);
            }
            return effects;
        }
        if self.modal.is_some() {
            return effects;
        }
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            match event.kind {
                MouseEventKind::ScrollUp | MouseEventKind::ScrollLeft => {
                    self.scroll_horizontal(false);
                }
                MouseEventKind::ScrollDown | MouseEventKind::ScrollRight => {
                    self.scroll_horizontal(true);
                }
                _ => {}
            }
            return effects;
        }

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.text_selection = None;
                let point = (event.column, event.row).into();
                if self
                    .geometry
                    .project_hits
                    .iter()
                    .any(|area| area.contains(point))
                {
                    self.open_projects(ProjectOpenMode::CurrentTab, &mut effects);
                } else if let Some(target) = self
                    .geometry
                    .link_hits
                    .iter()
                    .find(|hit| hit.area.contains(point))
                    .map(|hit| hit.target.clone())
                {
                    effects.push(AppEffect::Open(target));
                } else if self
                    .geometry
                    .sidebar_divider
                    .contains((event.column, event.row).into())
                {
                    self.begin_resize(ResizeTarget::Sidebar, event.column, now);
                } else if self
                    .geometry
                    .diff_divider
                    .is_some_and(|divider| divider.contains((event.column, event.row).into()))
                {
                    self.begin_resize(ResizeTarget::Diff, event.column, now);
                } else {
                    self.last_resize_tap = None;
                    if self
                        .geometry
                        .changes_tab
                        .contains((event.column, event.row).into())
                    {
                        self.switch_view(View::Changes, &mut effects);
                    } else if self
                        .geometry
                        .history_tab
                        .contains((event.column, event.row).into())
                    {
                        self.switch_view(View::History, &mut effects);
                    } else if self
                        .geometry
                        .pull_requests_tab
                        .contains((event.column, event.row).into())
                    {
                        self.switch_view(View::PullRequests, &mut effects);
                    } else if let Some(action) = self
                        .geometry
                        .scm_action_hits
                        .iter()
                        .find(|hit| hit.area.contains((event.column, event.row).into()))
                        .map(|hit| hit.action.clone())
                    {
                        self.handle_scm_action(action, &mut effects);
                    } else if self.scm_menu_open || self.pr_menu_open {
                        self.scm_menu_open = false;
                        self.pr_menu_open = false;
                    } else if self
                        .geometry
                        .sidebar
                        .contains((event.column, event.row).into())
                    {
                        self.set_focus(Focus::Sidebar, &mut effects);
                        if let Some(hit) = self
                            .geometry
                            .sidebar_hits
                            .iter()
                            .find(|hit| hit.area.contains((event.column, event.row).into()))
                            .map(|hit| hit.target.clone())
                        {
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
                                SidebarHit::PullRequestFiles => self.select_pull_request_section(
                                    PullRequestSection::Files,
                                    &mut effects,
                                ),
                                SidebarHit::PullRequestOverview => self
                                    .select_pull_request_section(
                                        PullRequestSection::Overview,
                                        &mut effects,
                                    ),
                                SidebarHit::PullRequestConversation => {
                                    self.selected_check_section = None;
                                    self.select_pull_request_check(None, &mut effects);
                                }
                                SidebarHit::PullRequestCheckSection(section) => {
                                    self.toggle_check_section(section);
                                    self.schedule_preview(now);
                                }
                                SidebarHit::PullRequestChooseRepository => {
                                    self.open_pull_request_repositories(&mut effects);
                                }
                                SidebarHit::PullRequestLookup => {
                                    self.pull_request_lookup_active = true;
                                }
                                SidebarHit::RecentPullRequest(index) => {
                                    let _ = self.open_recent_pull_request(index, &mut effects);
                                }
                                SidebarHit::PullRequestDirectory(path) => {
                                    self.toggle_pull_request_directory(path);
                                }
                                SidebarHit::PullRequestFile(index) => {
                                    if let Some(cursor) =
                                        self.pull_request_tree_entries().iter().position(|entry| {
                                            matches!(
                                                entry,
                                                PullRequestTreeEntry::File {
                                                    index: entry_index,
                                                    ..
                                                } if *entry_index == index
                                            )
                                        })
                                    {
                                        self.select_pull_request_tree_entry(cursor, now);
                                    }
                                }
                                SidebarHit::PullRequestCheck(index) => {
                                    if index < self.pull_request_checks.len() {
                                        self.select_pull_request_check(Some(index), &mut effects);
                                    }
                                }
                            }
                        }
                    } else if self
                        .geometry
                        .content
                        .contains((event.column, event.row).into())
                    {
                        if let Some(step) = self
                            .geometry
                            .content_step_hits
                            .iter()
                            .find(|hit| hit.area.contains((event.column, event.row).into()))
                            .map(|hit| hit.step)
                        {
                            self.focus = Focus::Content;
                            self.toggle_check_step(step);
                        } else if let Some(thread_id) = self
                            .geometry
                            .content_review_hits
                            .iter()
                            .find(|hit| hit.area.contains((event.column, event.row).into()))
                            .map(|hit| hit.thread_id.clone())
                        {
                            self.focus = Focus::Content;
                            self.open_review_thread_actions(&thread_id);
                        } else if let Some(path) = self
                            .geometry
                            .content_file_hits
                            .iter()
                            .find(|hit| hit.area.contains((event.column, event.row).into()))
                            .map(|hit| hit.path.clone())
                        {
                            self.focus = Focus::Content;
                            self.toggle_preview_file(path, &mut effects);
                        } else {
                            self.focus = Focus::Content;
                            self.start_text_selection(event.column, event.row);
                        }
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.last_resize_tap = None;
                match self.resize_target {
                    Some(ResizeTarget::Sidebar) => self.resize_sidebar(event.column),
                    Some(ResizeTarget::Diff) => self.resize_diff(event.column),
                    None => self.update_text_selection(event.column, event.row),
                }
            }
            MouseEventKind::Up(MouseButton::Left) if self.resize_target.take().is_none() => {
                self.update_text_selection(event.column, event.row);
                let dragged = self
                    .text_selection
                    .is_some_and(|selection| selection.anchor != selection.head);
                if dragged {
                    let text = self.selected_text();
                    if !text.is_empty() {
                        effects.push(AppEffect::Copy(text));
                        self.show_toast(
                            "Selection copied from this pane".to_owned(),
                            ToastLevel::Success,
                            now,
                        );
                    }
                } else {
                    self.text_selection = None;
                }
            }
            MouseEventKind::ScrollDown => {
                if self
                    .geometry
                    .sidebar
                    .contains((event.column, event.row).into())
                {
                    self.focus = Focus::Sidebar;
                    self.sidebar_free_scroll = true;
                    self.sidebar_offset = self.sidebar_offset.saturating_add(2);
                    let visible_height =
                        usize::from(self.geometry.sidebar.height.saturating_sub(2));
                    let visible_end = self.sidebar_offset.saturating_add(visible_height);
                    self.request_history_near_end(visible_end, &mut effects);
                } else {
                    self.focus = Focus::Content;
                    self.content_scroll = self.content_scroll.saturating_add(2);
                }
            }
            MouseEventKind::ScrollUp => {
                if self
                    .geometry
                    .sidebar
                    .contains((event.column, event.row).into())
                {
                    self.focus = Focus::Sidebar;
                    self.sidebar_free_scroll = true;
                    self.sidebar_offset = self.sidebar_offset.saturating_sub(2);
                } else {
                    self.focus = Focus::Content;
                    self.content_scroll = self.content_scroll.saturating_sub(2);
                }
            }
            MouseEventKind::ScrollLeft => {
                self.scroll_horizontal(false);
            }
            MouseEventKind::ScrollRight => {
                self.scroll_horizontal(true);
            }
            _ => {}
        }
        effects
    }
}
