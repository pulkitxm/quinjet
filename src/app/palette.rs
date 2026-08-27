#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn execute_palette(
        &mut self,
        command: PaletteCommand,
        effects: &mut Vec<AppEffect>,
        now: Instant,
    ) {
        match command {
            PaletteCommand::Refresh => self.request_active_refresh(effects),
            PaletteCommand::StageAll => self.queue_operation(GitOperation::StageAll, effects),
            PaletteCommand::UnstageAll => {
                self.queue_operation(GitOperation::UnstageAll, effects);
            }
            PaletteCommand::Commit | PaletteCommand::Amend => {
                self.modal = Some(Modal::Commit {
                    input: TextBuffer::default(),
                    amend: command == PaletteCommand::Amend,
                });
            }
            PaletteCommand::Fetch => self.queue_operation(GitOperation::Fetch, effects),
            PaletteCommand::Pull => self.queue_operation(GitOperation::Pull, effects),
            PaletteCommand::Push => self.queue_operation(GitOperation::Push, effects),
            PaletteCommand::Sync => self.queue_operation(GitOperation::Sync, effects),
            PaletteCommand::Stash => self.prompt_stash(false, false),
            PaletteCommand::StashStaged => self.prompt_stash(false, true),
            PaletteCommand::StashIncludeUntracked => self.prompt_stash(true, false),
            PaletteCommand::StashPop => self.queue_operation(GitOperation::StashPop(None), effects),
            PaletteCommand::ManageStashes => self.open_stashes(effects),
            PaletteCommand::OpenProject => {
                self.open_projects(ProjectOpenMode::CurrentTab, effects);
            }
            PaletteCommand::OpenProjectNewTab => {
                self.open_projects(ProjectOpenMode::NewTab, effects);
            }
            PaletteCommand::Branches => self.open_branches(effects),
            PaletteCommand::CompareBranch => self.open_compare_branches(effects),
            PaletteCommand::RenameCurrentBranch => {
                if self.status.branch.detached || self.status.branch.head.is_empty() {
                    self.show_toast(
                        "Cannot rename a detached or unnamed branch".to_owned(),
                        ToastLevel::Error,
                        now,
                    );
                } else {
                    let old = self.status.branch.head.clone();
                    self.modal = Some(Modal::Prompt {
                        title: "Rename Current Local Branch".to_owned(),
                        input: TextBuffer::new(old.clone()),
                        kind: PromptKind::RenameBranch { old },
                    });
                }
            }
            PaletteCommand::ToggleDiffLayout => self.toggle_diff_layout(),
            PaletteCommand::ToggleAllFiles => self.toggle_all_preview_files(effects),
            PaletteCommand::ShowChanges => self.switch_view(View::Changes, effects),
            PaletteCommand::ShowHistory => self.switch_view(View::History, effects),
            PaletteCommand::ShowPullRequests => self.switch_view(View::PullRequests, effects),
            PaletteCommand::ChangeTheme => {
                self.modal = Some(Modal::Themes {
                    selected: ThemeName::ALL
                        .iter()
                        .position(|name| *name == self.theme_name)
                        .unwrap_or_default(),
                    original: self.theme_name,
                });
            }
            PaletteCommand::ChangeAppearance => {
                self.modal = Some(Modal::Appearances {
                    selected: AppearanceChoice::ALL
                        .iter()
                        .position(|choice| *choice == self.appearance_choice)
                        .unwrap_or_default(),
                    original_choice: self.appearance_choice,
                    original_appearance: self.appearance,
                });
            }
            PaletteCommand::Help => {
                self.modal = Some(Modal::Help {
                    selected: 0,
                    scroll: 0,
                    hover: None,
                });
            }
            PaletteCommand::Quit if !self.exit_locked() => effects.push(AppEffect::Quit),
            PaletteCommand::Quit => {}
        }
    }

    pub(super) fn apply_theme(&mut self, name: ThemeName) {
        self.theme_name = name;
        self.theme = Theme::new(name, self.appearance);
        self.invalidate_pull_request_content_rows();
    }

    pub(super) fn apply_live_modal_filter(&mut self) {
        if let Some(Modal::Prompt {
            input,
            kind: PromptKind::Filter { .. },
            ..
        }) = self.modal.as_ref()
        {
            self.filter.clone_from(&input.value);
            self.normalize_selection();
            self.preview_due = Some(Instant::now() + PREVIEW_DEBOUNCE);
        }
    }

    pub(super) fn begin_resize(&mut self, target: ResizeTarget, column: u16, now: Instant) {
        let double_tap = self.last_resize_tap.is_some_and(|(previous, tapped)| {
            previous == target
                && now.saturating_duration_since(tapped) <= RESIZE_DOUBLE_TAP_INTERVAL
        });
        if double_tap {
            match target {
                ResizeTarget::Sidebar => {
                    let maximum = self
                        .geometry
                        .main
                        .width
                        .saturating_sub(MIN_CONTENT_WIDTH)
                        .max(MIN_SIDEBAR_WIDTH);
                    self.sidebar_width = DEFAULT_SIDEBAR_WIDTH.clamp(MIN_SIDEBAR_WIDTH, maximum);
                }
                ResizeTarget::Diff => self.diff_split_percent = DEFAULT_DIFF_SPLIT_PERCENT,
            }
            self.resize_target = None;
            self.last_resize_tap = None;
            return;
        }

        self.last_resize_tap = Some((target, now));
        self.resize_target = Some(target);
        match target {
            ResizeTarget::Sidebar => self.resize_sidebar(column),
            ResizeTarget::Diff => self.resize_diff(column),
        }
    }

    pub(super) fn resize_sidebar(&mut self, column: u16) {
        let main = self.geometry.main;
        let maximum = main
            .width
            .saturating_sub(MIN_CONTENT_WIDTH)
            .max(MIN_SIDEBAR_WIDTH);
        self.sidebar_width = column
            .saturating_sub(main.x)
            .clamp(MIN_SIDEBAR_WIDTH, maximum);
    }

    #[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
    pub(super) fn resize_diff(&mut self, column: u16) {
        let content = self.geometry.content;
        if content.width == 0 {
            return;
        }
        let relative = column.saturating_sub(content.x).min(content.width);
        self.diff_split_percent = (relative.saturating_mul(100) / content.width)
            .clamp(MIN_DIFF_SPLIT_PERCENT, MAX_DIFF_SPLIT_PERCENT);
    }

    pub(super) fn switch_view(&mut self, view: View, effects: &mut Vec<AppEffect>) {
        if self.view == view {
            if view == View::PullRequests && self.pull_request.is_none() {
                self.pull_request_lookup_active = self.recent_pull_requests.is_empty();
            }
            return;
        }
        let leaving = self.view;
        if leaving == View::PullRequests && self.document_loading {
            self.pull_request_loading_path = None;
        }
        self.store_active_view();
        self.invalidate_preview();
        self.view = view;
        self.scm_menu_open = false;
        self.pr_menu_open = false;
        self.text_selection = None;
        self.resize_target = None;
        let resume_preview = self.restore_view(view);
        if view == View::PullRequests {
            self.decorate_pull_request_review();
        }
        self.set_focus(self.focus, effects);
        self.schedule_pull_request_poll(Instant::now());
        if view == View::PullRequests && self.pull_request.is_none() {
            self.pull_request_lookup_active = self.recent_pull_requests.is_empty();
        } else {
            if resume_preview && self.local_diff_index.is_some() {
                self.request_next_local_diff_file(effects);
            }
            self.preview_due = Some(Instant::now() + PREVIEW_DEBOUNCE);
        }
    }

    pub(super) fn loading_document_for_view(&self, view: View) -> DiffDocument {
        match view {
            View::Changes => DiffDocument::empty("Working Tree", "Loading selected changes…"),
            View::History => {
                let title = self.selected_commit().map_or_else(
                    || "Commit History".to_owned(),
                    |commit| format!("{} — {}", commit.short_id, commit.subject),
                );
                let message = if self.history_loading && self.history.is_empty() {
                    "Loading commit history…"
                } else if self.history.is_empty() {
                    "No commits in this repository"
                } else {
                    "Loading commit preview…"
                };
                DiffDocument::empty(title, message)
            }
            View::PullRequests => self.selected_pull_request().map_or_else(
                || {
                    DiffDocument::empty(
                        "Open Pull Request",
                        "Enter a pull-request number and press Enter",
                    )
                },
                |pull_request| {
                    pull_request_loading_document(
                        pull_request,
                        self.pull_request_progress
                            .map_or("Calculating pull-request diff…", PullRequestProgress::label),
                    )
                },
            ),
        }
    }

    pub(super) fn toggle_focus(&mut self, effects: &mut Vec<AppEffect>) {
        let focus = match self.focus {
            Focus::Sidebar => Focus::Content,
            Focus::Content => Focus::Sidebar,
        };
        self.set_focus(focus, effects);
    }

    pub(super) fn toggle_sidebar(&mut self, effects: &mut Vec<AppEffect>) {
        self.sidebar_hidden = !self.sidebar_hidden;
        let focus = if self.sidebar_hidden {
            Focus::Content
        } else {
            Focus::Sidebar
        };
        self.set_focus(focus, effects);
        self.resize_target = None;
        if self.sidebar_hidden {
            self.pull_request_lookup_active = false;
        }
    }

    pub(super) const fn toggle_diff_layout(&mut self) {
        self.diff_layout = match self.diff_layout {
            DiffLayout::Unified => DiffLayout::SideBySide,
            DiffLayout::SideBySide => DiffLayout::Unified,
        };
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
    }

    pub(super) fn navigate(&mut self, amount: isize, now: Instant) {
        if self.focus == Focus::Content
            && self.check_log_visible()
            && self.move_check_step_cursor(amount)
        {
            return;
        }
        if self.focus == Focus::Content {
            if self.preview_files_collapsible() {
                self.navigate_preview_file(amount);
            } else if amount < 0 {
                self.content_scroll = self.content_scroll.saturating_sub(amount.unsigned_abs());
            } else {
                self.content_scroll = self.content_scroll.saturating_add(count(amount));
            }
            return;
        }

        let preserve_auxiliary_preview = self.auxiliary_preview.is_some();
        match self.view {
            View::Changes => {
                let targets = self.change_targets();
                if targets.is_empty() {
                    self.selected_change_section = Some(ChangeSection::Unstaged);
                    self.change_cursor = 0;
                    return;
                }
                let current = self
                    .selected_change_target()
                    .and_then(|target| targets.iter().position(|candidate| *candidate == target))
                    .unwrap_or_default();
                let next = if amount < 0 {
                    current.saturating_sub(amount.unsigned_abs())
                } else {
                    (current + count(amount)).min(targets.len() - 1)
                };
                if let Some(target) = targets.get(next).copied() {
                    self.select_change_target(target);
                }
            }
            View::History => {
                let length = self.visible_commit_indices().len();
                if length == 0 {
                    self.history_cursor = 0;
                    return;
                }
                self.history_cursor = if amount < 0 {
                    self.history_cursor.saturating_sub(amount.unsigned_abs())
                } else {
                    (self.history_cursor + count(amount)).min(length - 1)
                };
            }
            View::PullRequests => {
                match self.pull_request_section {
                    PullRequestSection::Files => {
                        let length = self.pull_request_tree_entries().len();
                        if length == 0 {
                            self.pull_request_file_cursor = 0;
                            self.pull_request_tree_cursor = 0;
                            return;
                        }
                        let cursor = if amount < 0 {
                            self.pull_request_tree_cursor
                                .saturating_sub(amount.unsigned_abs())
                        } else {
                            (self.pull_request_tree_cursor + count(amount)).min(length - 1)
                        };
                        self.select_pull_request_tree_entry(cursor, now);
                    }
                    PullRequestSection::Overview => {
                        self.move_check_cursor(amount);
                        self.schedule_preview(now);
                    }
                    PullRequestSection::Stack => {
                        let _ = self.move_pull_request_stack_cursor(amount, false, now);
                    }
                }
                return;
            }
        }
        if !preserve_auxiliary_preview {
            self.schedule_preview(now);
        }
    }

    pub(super) fn page(&mut self, direction: isize, now: Instant) {
        if self.focus == Focus::Content {
            let amount = self.geometry.content.height.saturating_sub(4).max(1) as usize;
            if direction < 0 {
                self.content_scroll = self.content_scroll.saturating_sub(amount);
            } else {
                self.content_scroll = self.content_scroll.saturating_add(amount);
            }
        } else {
            let amount = offset(self.geometry.sidebar.height.saturating_sub(4).max(1));
            self.navigate(direction * amount, now);
        }
    }

    pub(super) fn go_to_edge(&mut self, end: bool, now: Instant) {
        if self.focus == Focus::Content && self.check_log_visible() {
            let steps = self.check_step_numbers();
            if let Some(step) = if end { steps.last() } else { steps.first() } {
                self.reveal_check_step(*step);
                return;
            }
        }
        if self.focus == Focus::Content {
            self.content_scroll = if end { usize::MAX } else { 0 };
            return;
        }
        let preserve_auxiliary_preview = self.auxiliary_preview.is_some();
        match self.view {
            View::Changes => {
                let targets = self.change_targets();
                if let Some(target) = if end { targets.last() } else { targets.first() } {
                    self.select_change_target(*target);
                }
            }
            View::History => {
                let length = self.visible_commit_indices().len();
                self.history_cursor = if end { length.saturating_sub(1) } else { 0 };
            }
            View::PullRequests => {
                match self.pull_request_section {
                    PullRequestSection::Files => {
                        let entries = self.pull_request_tree_entries();
                        let cursor = if end {
                            entries.len().saturating_sub(1)
                        } else {
                            0
                        };
                        self.select_pull_request_tree_entry(cursor, now);
                    }
                    PullRequestSection::Overview => {
                        let targets = self.check_list_targets();
                        let Some(target) = (if end {
                            targets.last().copied()
                        } else {
                            targets.first().copied()
                        }) else {
                            return;
                        };
                        self.select_check_list_target(target);
                        self.schedule_preview(now);
                    }
                    PullRequestSection::Stack => {
                        let _ = self.move_pull_request_stack_to_edge(end, now);
                    }
                }
                return;
            }
        }
        if !preserve_auxiliary_preview {
            self.schedule_preview(now);
        }
    }

    #[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
    pub(super) fn scroll_content_half(&mut self, down: bool) {
        let amount = (self.geometry.content.height / 2).max(1) as usize;
        if down {
            self.content_scroll = self.content_scroll.saturating_add(amount);
        } else {
            self.content_scroll = self.content_scroll.saturating_sub(amount);
        }
    }

    pub(super) fn jump_hunk(&mut self, forward: bool) {
        if forward {
            if let Some((index, _)) = self
                .document
                .lines
                .iter()
                .enumerate()
                .skip(self.content_scroll.saturating_add(1))
                .find(|(_, line)| line.kind == DiffLineKind::HunkHeader)
            {
                self.content_scroll = index;
            }
        } else if let Some((index, _)) = self
            .document
            .lines
            .iter()
            .enumerate()
            .take(self.content_scroll)
            .rev()
            .find(|(_, line)| line.kind == DiffLineKind::HunkHeader)
        {
            self.content_scroll = index;
        }
    }
}
