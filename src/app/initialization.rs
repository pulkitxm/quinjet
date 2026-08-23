#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the constructor explicitly initializes every independent state field"
    )]
    pub(crate) fn new(root: impl AsRef<Path>, name: impl Into<String>) -> Self {
        Self {
            repository_root: root.as_ref().to_path_buf(),
            repository_name: name.into(),
            view: View::Changes,
            view_states: ViewStates::default(),
            focus: Focus::Sidebar,
            diff_layout: DiffLayout::SideBySide,
            theme: Theme::default(),
            theme_name: ThemeName::default(),
            appearance_choice: AppearanceChoice::default(),
            appearance: Appearance::Dark,
            status: RepoStatus::default(),
            history: Vec::new(),
            worktrees: Vec::new(),
            project_groups: Vec::new(),
            collapsed_project_groups: {
                #[cfg(test)]
                {
                    HashSet::new()
                }
                #[cfg(not(test))]
                {
                    crate::state::load_collapsed_project_groups()
                }
            },
            ssh_context: SshContext::from_environment(),
            history_branch: None,
            pull_request: None,
            github_repositories: Vec::new(),
            local_github_repository: None,
            pull_request_repository: None,
            pull_request_warnings: Vec::new(),
            pull_request_error: None,
            pull_request_exact_number: None,
            pull_request_from_cache: false,
            history_branches: Vec::new(),
            history_branches_loading: false,
            history_branches_loaded: false,
            pull_request_lookup: TextBuffer::default(),
            pull_request_lookup_active: false,
            recent_pull_requests: initial_recent_pull_requests(),
            recent_pull_request_cursor: 0,
            pull_request_section: PullRequestSection::Overview,
            pull_request_file_view: PullRequestFileView::AllFiles,
            pull_request_files: Vec::new(),
            pull_request_tree: Vec::new(),
            pull_request_total_files: 0,
            pull_request_files_truncated: false,
            pull_request_file_cursor: 0,
            pull_request_tree_cursor: 0,
            collapsed_pull_request_directories: HashSet::new(),
            pull_request_checks: Vec::new(),
            pull_request_check_cursor: None,
            selected_check_section: None,
            collapsed_check_sections: HashSet::new(),
            pull_request_checks_loading: false,
            pull_request_checks_error: None,
            pull_request_checks_from_cache: false,
            pull_request_prefetched_logs: HashSet::new(),
            pull_request_conversation: PullRequestConversation::default(),
            pull_request_conversation_loading: false,
            pull_request_conversation_refresh_again: false,
            pull_request_conversation_error: None,
            pull_request_review: PullRequestReviewSnapshot::default(),
            pull_request_review_loading: false,
            pull_request_review_mutating: false,
            pull_request_review_error: None,
            pull_request_review_cursor: None,
            pull_request_review_line_threads: HashMap::new(),
            pull_request_check_log: None,
            pull_request_check_log_loading: false,
            pull_request_check_log_error: None,
            expanded_check_steps: HashSet::new(),
            pull_request_step_cursor: 0,
            pull_request_step_reveal: false,
            pull_request_content_rows: Vec::new(),
            pull_request_content_rows_key: None,
            pull_request_content_width: 0,
            pull_request_content_links: Vec::new(),
            pull_request_content_generation: 0,
            relative_time_generation: crate::date_time::relative_time_generation(),
            content_at_bottom: true,
            pull_request_progress: None,
            auxiliary_preview: None,
            document: DiffDocument::empty("Working Tree", "Loading changes…"),
            document_layout_generation: 0,
            unified_diff_rows: Vec::new(),
            side_by_side_diff_rows: Vec::new(),
            diff_rows_key: None,
            selected_change_section: Some(ChangeSection::Unstaged),
            collapsed_change_sections: HashSet::new(),
            checked_change_paths: HashSet::new(),
            scm_menu_open: false,
            scm_menu_selected: 0,
            pr_menu_open: false,
            pr_menu_selected: 0,
            preferred_merge_method: PullRequestMergeMethod::default(),
            selected_preview_file: None,
            preview_file_cursor: 0,
            collapsed_preview_files: HashSet::new(),
            expanded_preview_files: HashSet::new(),
            content_file_anchor: None,
            change_cursor: 0,
            history_cursor: 0,
            sidebar_offset: 0,
            sidebar_free_scroll: false,
            sidebar_last_cursor: None,
            content_scroll: 0,
            horizontal_scroll: 0,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_hidden: false,
            diff_split_percent: DEFAULT_DIFF_SPLIT_PERCENT,
            expanded_diff: false,
            files_collapsed: false,
            collapse_preference_set: false,
            resize_target: None,
            filter: String::new(),
            modal: None,
            modal_scroll: 0,
            modal_free_scroll: false,
            toast: None,
            mouse_capture: true,
            mouse_capture_preference: true,
            link_hover: None,
            text_selection: None,
            rendered_cells: Vec::new(),
            webhooks_listening: false,
            host_client: None,
            busy: None,
            operation_frame: 0,
            refreshing: false,
            document_loading: false,
            history_loading: false,
            history_complete: false,
            pull_request_loading: false,
            last_refresh: None,
            geometry: UiGeometry::default(),
            repository_tabs: Vec::new(),
            repository_tab_drag: None,
            repository_tab_menu: None,
            tab_active: true,
            status_generation: 0,
            changes_diff_version: 0,
            diff_generation: 0,
            history_generation: 0,
            pull_request_generation: 0,
            repository_generation: 0,
            pull_request_workspace_generation: None,
            pull_request_documents: HashMap::new(),
            pull_request_document_order: VecDeque::new(),
            pull_request_document_bytes: 0,
            pull_request_prefetched_paths: HashSet::new(),
            pull_request_loading_path: None,
            pull_request_single_file: None,
            pull_request_prefetching: false,
            pull_request_prefetch_retrying: false,
            pull_request_checks_generation: 0,
            pull_request_conversation_generation: 0,
            pull_request_review_generation: 0,
            pull_request_check_log_generation: 0,
            pull_request_check_log_target: None,
            local_diff_request: None,
            local_diff_change_section: None,
            local_diff_preserving_document: false,
            local_diff_preserved_paths: HashSet::new(),
            local_diff_workspace_generation: None,
            local_diff_index: None,
            local_diff_documents: HashMap::new(),
            local_diff_loading_path: None,
            local_diff_pending_paths: VecDeque::new(),
            local_diff_single_loaded: false,
            branch_generation: 0,
            history_branch_generation: 0,
            stash_generation: 0,
            worktree_generation: 0,
            project_generation: 0,
            operation_id: 0,
            refresh_again: false,
            history_refresh_again: false,
            preview_due: None,
            pull_request_poll_due: None,
            pull_request_checks_read_at: None,
            pull_request_detail_read_at: None,
            pull_request_log_read_at: None,
            pending_g: None,
            last_resize_tap: None,
        }
    }

    pub(crate) const fn set_host_client(&mut self, client: Option<Client>) {
        self.host_client = client;
    }

    pub(crate) const fn exit_locked(&self) -> bool {
        matches!(self.host_client, Some(Client::Edith))
    }

    pub(crate) fn set_theme_selection(&mut self, name: ThemeName, choice: AppearanceChoice) {
        let appearance = choice.resolve();
        self.theme = Theme::new(name, appearance);
        self.theme_name = name;
        self.appearance_choice = choice;
        self.appearance = appearance;
        self.invalidate_pull_request_content_rows();
    }

    #[doc = " Launch straight into one pull request: the `--pr` flag arrives here"]
    #[doc = " before the first frame, so the lookup races the initial status reads"]
    #[doc = " instead of waiting for them."]
    pub(crate) fn open_pull_request_on_launch(&mut self, number: u64) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        self.switch_view(View::PullRequests, &mut effects);
        self.request_pull_request_lookup(number, false, false, &mut effects);
        effects
    }

    pub(crate) fn initial_effects(&mut self) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        self.request_refresh(&mut effects);
        self.request_history(true, &mut effects);
        self.request_history_branches(&mut effects);
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadLocalGitHubRepository,
        )));
        effects
    }
}
