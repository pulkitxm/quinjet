#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn handle_mouse(&mut self, event: MouseEvent, now: Instant) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        if self.modal.is_none() {
            self.modal_scroll = 0;
            self.modal_free_scroll = false;
        }
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
            toggle_membership(collapsed, common_dir.clone());
            let visible = Self::filtered_project_rows(groups, &query.value, collapsed);
            *selected = visible
                .iter()
                .position(|row| {
                    matches!(row, ProjectRow::Group(group_index) if groups.get(*group_index).is_some_and(|group| group.common_dir == common_dir))
                })
                .unwrap_or_else(|| (*selected).min(visible.len().saturating_sub(1)));
            let collapsed = collapsed.clone();
            self.remember_collapsed_project_groups(&collapsed);
            return effects;
        }
        if self.modal.is_some() && self.geometry.modal_list_len > 0 {
            let point = (event.column, event.row).into();
            match event.kind {
                MouseEventKind::Moved => {
                    if let Some(index) = self
                        .geometry
                        .modal_list_hits
                        .iter()
                        .find(|(area, _)| area.contains(point))
                        .map(|(_, index)| *index)
                    {
                        self.select_modal_row(index);
                    }
                    return effects;
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(index) = self
                        .geometry
                        .modal_list_hits
                        .iter()
                        .find(|(area, _)| area.contains(point))
                        .map(|(_, index)| *index)
                    {
                        self.select_modal_row(index);
                        self.modal_free_scroll = false;
                        return self
                            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
                    }
                }
                MouseEventKind::ScrollUp => {
                    self.modal_scroll = self.modal_scroll.saturating_sub(2);
                    self.modal_free_scroll = true;
                    return effects;
                }
                MouseEventKind::ScrollDown => {
                    self.modal_scroll = self
                        .modal_scroll
                        .saturating_add(2)
                        .min(self.geometry.modal_list_max_scroll);
                    self.modal_free_scroll = true;
                    return effects;
                }
                _ => {}
            }
        }
        if let Some(Modal::Help {
            selected,
            scroll,
            hover,
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
                        self.modal_free_scroll = false;
                    }
                }
                MouseEventKind::ScrollUp => {
                    *scroll = scroll.saturating_sub(2);
                    *hover = None;
                    self.modal_free_scroll = true;
                }
                MouseEventKind::ScrollDown => {
                    *scroll = scroll.saturating_add(2);
                    *hover = None;
                    self.modal_free_scroll = true;
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
                    | Modal::Conflict { .. }
                    | Modal::Projects { .. }
                    | Modal::PullRequestActions { .. }
                    | Modal::PullRequestReviewSubmit { .. }
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
                self.handle_modal_action(action, &mut effects, now);
            }
            return effects;
        }
        if self.modal.is_some() {
            return effects;
        }
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            if event.kind == MouseEventKind::Down(MouseButton::Left)
                && let Some(position) = self.pull_request_stack_hit_at(event.column, event.row)
            {
                self.set_focus(Focus::Sidebar, &mut effects);
                let _ = self.select_pull_request_stack_member(position, true, now);
                return effects;
            }
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
                } else if let Some(OpenTarget::Browser(url)) = self
                    .geometry
                    .link_hits
                    .iter()
                    .find(|hit| hit.area.contains(point))
                    .map(|hit| hit.target.clone())
                {
                    self.open_link(url, &mut effects, now);
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
                        self.handle_scm_action(action, &mut effects, now);
                    } else if let Some(hit) = self
                        .geometry
                        .stack_inspector_hits
                        .iter()
                        .find(|hit| hit.area.contains(point))
                        .map(|hit| hit.target)
                    {
                        self.handle_stack_inspector_hit(hit, now, &mut effects);
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
                            self.handle_sidebar_hit(hit, now, &mut effects);
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

    fn select_modal_row(&mut self, index: usize) {
        match self.modal.as_mut() {
            Some(
                Modal::PullRequestReviewThreadActions { selected, .. }
                | Modal::PullRequestActions { selected, .. }
                | Modal::Branches { selected, .. }
                | Modal::HistoryBranches { selected, .. }
                | Modal::CompareBranches { selected, .. }
                | Modal::Stashes { selected, .. }
                | Modal::Projects { selected, .. }
                | Modal::PullRequestRepositories { selected, .. }
                | Modal::CommandPalette { selected, .. }
                | Modal::Themes { selected, .. }
                | Modal::Appearances { selected, .. },
            ) => *selected = index,
            _ => return,
        }
        if let Some(name) = self.modal.as_ref().and_then(|modal| match modal {
            Modal::Themes { selected, .. } => ThemeName::ALL.get(*selected).copied(),
            _ => None,
        }) {
            self.apply_theme(name);
        }
        if let Some(choice) = self.modal.as_ref().and_then(|modal| match modal {
            Modal::Appearances { selected, .. } => AppearanceChoice::ALL.get(*selected).copied(),
            _ => None,
        }) {
            self.set_theme_selection(self.theme_selection, choice);
        }
    }
}
