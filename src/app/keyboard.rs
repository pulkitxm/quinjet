#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn handle_key(&mut self, key: KeyEvent, now: Instant) -> Vec<AppEffect> {
        self.link_hover = None;
        if self.repository_tab_menu.is_some() {
            return self.handle_repository_tab_key(key).unwrap_or_default();
        }
        if let Some(modal) = self.modal.take() {
            self.modal_free_scroll = false;
            return self.handle_modal_key(modal, key, now);
        }
        self.modal_scroll = 0;
        self.modal_free_scroll = false;
        if self.pull_request_lookup_active {
            return self.handle_pull_request_lookup_key(key, now);
        }
        if let Some(effects) = self.handle_repository_tab_key(key) {
            return effects;
        }

        let mut effects = Vec::new();
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('p') => {
                    self.modal = Some(Modal::CommandPalette {
                        query: TextBuffer::default(),
                        selected: 0,
                    });
                }
                KeyCode::Char('r') => self.request_active_refresh(&mut effects),
                KeyCode::Char('d') => self.scroll_content_half(true),
                KeyCode::Char('u') => self.scroll_content_half(false),
                _ => {}
            }
            return effects;
        }

        match key.code {
            KeyCode::Char('q') => effects.push(AppEffect::Quit),
            KeyCode::Char('?') => {
                self.modal = Some(Modal::Help {
                    selected: 0,
                    scroll: 0,
                    hover: None,
                });
            }
            KeyCode::Char(':') => {
                self.modal = Some(Modal::CommandPalette {
                    query: TextBuffer::default(),
                    selected: 0,
                });
            }
            KeyCode::Char('1') => self.switch_view(View::Changes, &mut effects),
            KeyCode::Char('2') => self.switch_view(View::History, &mut effects),
            KeyCode::Char('3') => self.switch_view(View::PullRequests, &mut effects),
            KeyCode::Tab | KeyCode::BackTab if !self.sidebar_hidden => {
                self.toggle_focus(&mut effects);
            }
            KeyCode::Char('r') => self.request_active_refresh(&mut effects),
            KeyCode::Char('/') if self.view == View::PullRequests => {
                self.pull_request_lookup_active = true;
                self.set_focus(Focus::Sidebar, &mut effects);
            }
            KeyCode::Char('/') => {
                self.modal = Some(Modal::Prompt {
                    title: "Filter".to_owned(),
                    input: TextBuffer::new(self.filter.clone()),
                    kind: PromptKind::Filter {
                        previous: self.filter.clone(),
                    },
                });
            }
            KeyCode::Char('v') => self.toggle_diff_layout(),
            KeyCode::Char('e' | 'E') if self.check_log_visible() => {
                self.toggle_all_check_steps();
            }
            KeyCode::Char('e' | 'E') => self.toggle_all_preview_files(&mut effects),
            KeyCode::Char('t' | 'T') => {
                self.expanded_diff = !self.expanded_diff;
                self.content_scroll = 0;
                self.request_preview(&mut effects);
            }
            KeyCode::Char('b') if self.view == View::History => {
                self.open_history_branches(&mut effects);
            }
            KeyCode::Char('b' | 'B') => self.open_branches(&mut effects),
            KeyCode::Char('d') if self.view == View::Changes => {
                self.open_compare_branches(&mut effects);
            }
            KeyCode::Up if self.scm_menu_open && self.view == View::Changes => {
                let items = self.scm_menu_items();
                if !items.is_empty() {
                    self.scm_menu_selected =
                        previous_list_index(self.scm_menu_selected, items.len());
                }
            }
            KeyCode::Down if self.scm_menu_open && self.view == View::Changes => {
                let items = self.scm_menu_items();
                if !items.is_empty() {
                    self.scm_menu_selected = next_list_index(self.scm_menu_selected, items.len());
                }
            }
            KeyCode::Enter if self.scm_menu_open && self.view == View::Changes => {
                let items = self.scm_menu_items();
                if let Some(item) = items.get(self.scm_menu_selected).copied() {
                    self.scm_menu_open = false;
                    self.handle_scm_menu_item(item, &mut effects);
                }
            }
            KeyCode::Up if self.pr_menu_open && self.view == View::PullRequests => {
                let items = self.pr_menu_items();
                if !items.is_empty() {
                    self.pr_menu_selected = previous_list_index(self.pr_menu_selected, items.len());
                }
            }
            KeyCode::Down if self.pr_menu_open && self.view == View::PullRequests => {
                let items = self.pr_menu_items();
                if !items.is_empty() {
                    self.pr_menu_selected = next_list_index(self.pr_menu_selected, items.len());
                }
            }
            KeyCode::Enter if self.pr_menu_open && self.view == View::PullRequests => {
                let items = self.pr_menu_items();
                if let Some(item) = items.get(self.pr_menu_selected).copied() {
                    self.pr_menu_open = false;
                    self.handle_pr_menu_item(item, &mut effects);
                }
            }
            KeyCode::Esc if self.scm_menu_open || self.pr_menu_open => {
                self.scm_menu_open = false;
                self.pr_menu_open = false;
            }
            KeyCode::Char('S') if self.view == View::Changes => self.open_stashes(&mut effects),
            KeyCode::Char('w') => self.open_projects(ProjectOpenMode::CurrentTab, &mut effects),
            KeyCode::Char('N') => self.open_projects(ProjectOpenMode::NewTab, &mut effects),
            KeyCode::Char('o') if self.view == View::PullRequests => {
                self.open_pull_request_repositories(&mut effects);
            }
            KeyCode::Char('c') if self.view == View::Changes => {
                if self.primary_is_stash() {
                    self.confirm_stash_selected();
                } else {
                    self.modal = Some(Modal::Commit {
                        input: TextBuffer::default(),
                        amend: false,
                    });
                }
            }
            KeyCode::Char('c') if self.review_surface_active() => {
                self.open_review_comment(false);
            }
            KeyCode::Char('C') if self.review_surface_active() => {
                self.open_review_comment(true);
            }
            KeyCode::Char('a') if self.review_surface_active() => self.open_review_reply(),
            KeyCode::Char('V') if self.review_surface_active() => {
                self.modal = Some(Modal::PullRequestReviewSubmit {
                    input: TextBuffer::default(),
                    decision: PullRequestReviewDecision::Comment,
                });
            }
            KeyCode::Char('x') if self.review_surface_active() => {
                if let Some(thread) = self.selected_review_thread() {
                    let operation = if thread.is_resolved && thread.viewer_can_unresolve {
                        PullRequestReviewOperation::Unresolve {
                            thread_id: thread.id.clone(),
                        }
                    } else if !thread.is_resolved && thread.viewer_can_resolve {
                        PullRequestReviewOperation::Resolve {
                            thread_id: thread.id.clone(),
                        }
                    } else {
                        return effects;
                    };
                    self.queue_pull_request_review_operation(operation, &mut effects);
                }
            }
            KeyCode::Char('a') if self.view == View::Changes => {
                self.handle_scm_menu_item(ScmMenuItem::StageAll, &mut effects);
            }
            KeyCode::Char('U') if self.view == View::Changes => {
                self.handle_scm_menu_item(ScmMenuItem::UnstageAll, &mut effects);
            }
            KeyCode::Char('*')
                if self.view == View::Changes
                    && self.focus == Focus::Sidebar
                    && self.selected_change_section.is_none() =>
            {
                self.toggle_checked_selected();
            }
            KeyCode::Char('s' | ' ')
                if self.view == View::Changes
                    && self.focus == Focus::Sidebar
                    && self.selected_change_section.is_none() =>
            {
                self.toggle_stage_selected(&mut effects);
            }
            KeyCode::Char(' ') if self.view == View::Changes && self.focus == Focus::Sidebar => {
                let _ = self.toggle_selected_change_section();
            }
            KeyCode::Char(' ')
                if self.view == View::PullRequests
                    && self.pull_request.is_none()
                    && !self.recent_pull_requests.is_empty()
                    && self.focus == Focus::Sidebar =>
            {
                let _ =
                    self.open_recent_pull_request(self.recent_pull_request_cursor, &mut effects);
            }
            KeyCode::Char(' ')
                if self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Files
                    && self.focus == Focus::Sidebar =>
            {
                let _ = self.toggle_selected_pull_request_directory();
            }
            KeyCode::Char(' ')
                if self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Overview
                    && self.focus == Focus::Sidebar
                    && self.selected_check_section.is_some() =>
            {
                let _ = self.toggle_selected_check_section();
            }
            KeyCode::Char(' ') if self.check_log_visible() => {
                self.toggle_check_step(self.pull_request_step_cursor);
            }
            KeyCode::Char(' ')
                if self.focus == Focus::Content && self.preview_files_collapsible() =>
            {
                if let Some(path) = self.selected_preview_file.clone() {
                    self.toggle_preview_file(path, &mut effects);
                }
            }
            KeyCode::Char('u')
                if self.view == View::Changes && self.selected_change_section.is_none() =>
            {
                self.unstage_selected(&mut effects);
            }
            KeyCode::Char('x') if self.view == View::Changes => {
                self.confirm_discard();
            }
            KeyCode::Char('X') if self.view == View::Changes => {
                self.confirm_remove();
            }
            KeyCode::Char('P') if self.view == View::PullRequests => {
                self.select_pull_request_section(PullRequestSection::Overview, &mut effects);
            }
            KeyCode::Char('F') if self.view == View::PullRequests => {
                self.select_pull_request_section(PullRequestSection::Files, &mut effects);
            }
            KeyCode::Char('C') if self.view == View::History => self.confirm_cherry_pick(),
            KeyCode::Char('R') if self.view == View::History => self.confirm_revert(),
            KeyCode::Char('n') if self.view == View::History => self.prompt_branch_at_commit(),
            KeyCode::Char('f' | 'p') if self.view == View::PullRequests => {
                self.show_toast(
                    "Fetch and push live in Changes · Shift+P and Shift+F switch section"
                        .to_owned(),
                    ToastLevel::Error,
                    now,
                );
            }
            KeyCode::Char('f') => self.queue_operation(GitOperation::Fetch, &mut effects),
            KeyCode::Char('p') => self.queue_operation(GitOperation::Push, &mut effects),
            KeyCode::Char('l')
                if self.focus == Focus::Sidebar
                    && !(self.view == View::PullRequests
                        && self.pull_request_section == PullRequestSection::Files) =>
            {
                self.queue_operation(GitOperation::Pull, &mut effects);
            }
            KeyCode::Char('y') => self.queue_operation(GitOperation::Sync, &mut effects),
            KeyCode::Enter if !self.sidebar_hidden => self.toggle_focus(&mut effects),
            KeyCode::Esc => {
                if self.auxiliary_preview.take().is_some() {
                    self.request_preview(&mut effects);
                } else if self.view == View::PullRequests
                    && (self.pull_request_exact_number.is_some() || self.pull_request.is_some())
                {
                    self.invalidate_preview();
                    self.pull_request_exact_number = None;
                    self.pull_request = None;
                    self.reset_pull_request_runtime();
                    self.pull_request_warnings.clear();
                    self.pull_request_error = None;
                    self.pull_request_progress = None;
                    self.pull_request_poll_due = None;
                    self.pull_request_lookup = TextBuffer::default();
                    self.pull_request_lookup_active = self.recent_pull_requests.is_empty();
                    self.set_document(DiffDocument::empty(
                        "Open Pull Request",
                        "Enter a pull-request number and press Enter",
                    ));
                } else if !self.filter.is_empty() {
                    self.filter.clear();
                    self.normalize_selection();
                    self.schedule_preview(now);
                } else {
                    self.set_focus(Focus::Sidebar, &mut effects);
                }
            }
            KeyCode::Up | KeyCode::Char('k')
                if self.view == View::PullRequests && self.pull_request.is_none() =>
            {
                self.move_recent_pull_request_cursor(-1);
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.view == View::PullRequests && self.pull_request.is_none() =>
            {
                self.move_recent_pull_request_cursor(1);
            }
            KeyCode::Up | KeyCode::Char('k') if self.review_surface_active() => {
                self.move_review_cursor(-1);
            }
            KeyCode::Down | KeyCode::Char('j') if self.review_surface_active() => {
                self.move_review_cursor(1);
            }
            KeyCode::Up | KeyCode::Char('k') => self.navigate(-1, now),
            KeyCode::Down | KeyCode::Char('j') => self.navigate(1, now),
            KeyCode::PageUp => self.page(-1, now),
            KeyCode::PageDown => self.page(1, now),
            KeyCode::Home => self.go_to_edge(false, now),
            KeyCode::End | KeyCode::Char('G') => self.go_to_edge(true, now),
            KeyCode::Char('g') => {
                if self
                    .pending_g
                    .is_some_and(|pressed| now.duration_since(pressed) < Duration::from_millis(500))
                {
                    self.go_to_edge(false, now);
                    self.pending_g = None;
                } else {
                    self.pending_g = Some(now);
                }
            }
            KeyCode::Char('z') => self.toggle_sidebar(&mut effects),
            KeyCode::Char('m') => effects.push(self.toggle_mouse_capture(now)),
            KeyCode::Char('O') => self.open_selection_on_github(&mut effects, now),
            KeyCode::Char('[') if self.check_log_visible() => {
                let _ = self.move_check_step_cursor(-1);
            }
            KeyCode::Char(']') if self.check_log_visible() => {
                let _ = self.move_check_step_cursor(1);
            }
            KeyCode::Char('[' | ']')
                if self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Overview => {}
            KeyCode::Char('[') => self.jump_hunk(false),
            KeyCode::Char(']') => self.jump_hunk(true),
            KeyCode::Left | KeyCode::Char('h')
                if self.focus == Focus::Sidebar
                    && self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Files =>
            {
                self.navigate_pull_request_tree_horizontal(false, now);
            }
            KeyCode::Right | KeyCode::Char('l')
                if self.focus == Focus::Sidebar
                    && self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Files =>
            {
                self.navigate_pull_request_tree_horizontal(true, now);
            }
            KeyCode::Left if self.focus == Focus::Sidebar && self.view == View::Changes => {
                self.navigate_change_section_horizontal(false, now);
            }
            KeyCode::Right if self.focus == Focus::Sidebar && self.view == View::Changes => {
                self.navigate_change_section_horizontal(true, now);
            }
            KeyCode::Left
                if self.focus == Focus::Sidebar
                    && self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Overview =>
            {
                self.navigate_check_section_horizontal(false, now);
            }
            KeyCode::Right
                if self.focus == Focus::Sidebar
                    && self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Overview =>
            {
                self.navigate_check_section_horizontal(true, now);
            }
            KeyCode::Left | KeyCode::Char('h') if self.focus == Focus::Content => {
                self.horizontal_scroll = self.horizontal_scroll.saturating_sub(4);
            }
            KeyCode::Right | KeyCode::Char('l') if self.focus == Focus::Content => {
                self.horizontal_scroll = self.horizontal_scroll.saturating_add(4);
            }
            _ => {}
        }
        effects
    }

    pub(crate) fn handle_paste(&mut self, text: &str) {
        if self.pull_request_lookup_active {
            let remaining =
                MAX_PULL_REQUEST_NUMBER_DIGITS.saturating_sub(self.pull_request_lookup.value.len());
            let digits = text
                .chars()
                .filter(char::is_ascii_digit)
                .take(remaining)
                .collect::<String>();
            self.pull_request_lookup.insert_str(&digits);
            return;
        }
        if let Some(
            Modal::Commit { input, .. }
            | Modal::PullRequestReviewComment { input, .. }
            | Modal::PullRequestReviewSubmit { input, .. }
            | Modal::Prompt { input, .. }
            | Modal::CommandPalette { query: input, .. }
            | Modal::Branches { query: input, .. }
            | Modal::HistoryBranches { query: input, .. }
            | Modal::CompareBranches { query: input, .. }
            | Modal::Stashes { query: input, .. }
            | Modal::Projects { query: input, .. }
            | Modal::PullRequestRepositories { query: input, .. },
        ) = self.modal.as_mut()
        {
            input.insert_str(text);
        }
        self.apply_live_modal_filter();
        self.modal_scroll = 0;
        self.modal_free_scroll = false;
    }
}
