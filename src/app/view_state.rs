#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag restores an independent part of a view"
)]
pub(crate) struct ViewState {
    focus: Focus,
    filter: String,
    auxiliary_preview: Option<AuxiliaryPreview>,
    document: DiffDocument,
    selected_preview_file: Option<PathBuf>,
    preview_file_cursor: usize,
    collapsed_preview_files: HashSet<PathBuf>,
    expanded_preview_files: HashSet<PathBuf>,
    sidebar_offset: usize,
    sidebar_free_scroll: bool,
    sidebar_last_cursor: Option<usize>,
    content_scroll: usize,
    horizontal_scroll: usize,
    content_at_bottom: bool,
    pull_request_lookup_active: bool,
    local_diff_request: Option<LocalDiffRequest>,
    local_diff_change_section: Option<ChangeSection>,
    local_diff_preserving_document: bool,
    local_diff_preserved_paths: HashSet<PathBuf>,
    local_diff_workspace_generation: Option<u64>,
    local_diff_index: Option<DiffIndex>,
    local_diff_documents: HashMap<PathBuf, DiffDocument>,
    local_diff_loading_path: Option<PathBuf>,
    local_diff_pending_paths: VecDeque<PathBuf>,
    local_diff_single_loaded: bool,
    resume_preview: bool,
}

impl ViewState {
    pub(super) fn take(app: &mut App) -> Self {
        let resume_preview = app.document_loading;
        let mut local_diff_pending_paths = std::mem::take(&mut app.local_diff_pending_paths);
        if let Some(path) = app.local_diff_loading_path.take() {
            local_diff_pending_paths.push_front(path);
        }
        Self {
            focus: app.focus,
            filter: std::mem::take(&mut app.filter),
            auxiliary_preview: app.auxiliary_preview.take(),
            document: std::mem::take(&mut app.document),
            selected_preview_file: app.selected_preview_file.take(),
            preview_file_cursor: app.preview_file_cursor,
            collapsed_preview_files: std::mem::take(&mut app.collapsed_preview_files),
            expanded_preview_files: std::mem::take(&mut app.expanded_preview_files),
            sidebar_offset: app.sidebar_offset,
            sidebar_free_scroll: app.sidebar_free_scroll,
            sidebar_last_cursor: app.sidebar_last_cursor,
            content_scroll: app.content_scroll,
            horizontal_scroll: app.horizontal_scroll,
            content_at_bottom: app.content_at_bottom,
            pull_request_lookup_active: app.pull_request_lookup_active,
            local_diff_request: app.local_diff_request.take(),
            local_diff_change_section: app.local_diff_change_section,
            local_diff_preserving_document: app.local_diff_preserving_document,
            local_diff_preserved_paths: std::mem::take(&mut app.local_diff_preserved_paths),
            local_diff_workspace_generation: app.local_diff_workspace_generation,
            local_diff_index: app.local_diff_index.take(),
            local_diff_documents: std::mem::take(&mut app.local_diff_documents),
            local_diff_loading_path: None,
            local_diff_pending_paths,
            local_diff_single_loaded: app.local_diff_single_loaded,
            resume_preview,
        }
    }

    pub(super) fn restore(self, app: &mut App) -> bool {
        app.focus = self.focus;
        app.filter = self.filter;
        app.auxiliary_preview = self.auxiliary_preview;
        app.set_document(self.document);
        app.selected_preview_file = self.selected_preview_file;
        app.preview_file_cursor = self.preview_file_cursor;
        app.collapsed_preview_files = self.collapsed_preview_files;
        app.expanded_preview_files = self.expanded_preview_files;
        app.sidebar_offset = self.sidebar_offset;
        app.sidebar_free_scroll = self.sidebar_free_scroll;
        app.sidebar_last_cursor = self.sidebar_last_cursor;
        app.content_scroll = self.content_scroll;
        app.horizontal_scroll = self.horizontal_scroll;
        app.content_at_bottom = self.content_at_bottom;
        app.pull_request_lookup_active = self.pull_request_lookup_active;
        app.local_diff_request = self.local_diff_request;
        app.local_diff_change_section = self.local_diff_change_section;
        app.local_diff_preserving_document = self.local_diff_preserving_document;
        app.local_diff_preserved_paths = self.local_diff_preserved_paths;
        app.local_diff_workspace_generation = self.local_diff_workspace_generation;
        app.local_diff_index = self.local_diff_index;
        app.local_diff_documents = self.local_diff_documents;
        app.local_diff_loading_path = self.local_diff_loading_path;
        app.local_diff_pending_paths = self.local_diff_pending_paths;
        app.local_diff_single_loaded = self.local_diff_single_loaded;
        self.resume_preview
    }

    pub(super) const fn reset_sidebar_scroll(&mut self) {
        self.sidebar_offset = 0;
        self.sidebar_free_scroll = false;
        self.sidebar_last_cursor = None;
    }

    pub(super) const fn reset_content_position(&mut self) {
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
    }

    pub(super) fn set_document(&mut self, document: DiffDocument) {
        self.document = document;
    }

    pub(super) const fn mark_preview_for_resume(&mut self) {
        self.resume_preview = true;
    }
}

#[derive(Default)]
pub(crate) struct ViewStates {
    changes: Option<ViewState>,
    history: Option<ViewState>,
    pull_requests: Option<ViewState>,
}

impl ViewStates {
    pub(super) fn put(&mut self, view: View, state: ViewState) {
        match view {
            View::Changes => self.changes = Some(state),
            View::History => self.history = Some(state),
            View::PullRequests => self.pull_requests = Some(state),
        }
    }

    pub(super) const fn take(&mut self, view: View) -> Option<ViewState> {
        match view {
            View::Changes => self.changes.take(),
            View::History => self.history.take(),
            View::PullRequests => self.pull_requests.take(),
        }
    }

    pub(super) const fn state_mut(&mut self, view: View) -> Option<&mut ViewState> {
        match view {
            View::Changes => self.changes.as_mut(),
            View::History => self.history.as_mut(),
            View::PullRequests => self.pull_requests.as_mut(),
        }
    }
}

impl App {
    pub(super) fn store_active_view(&mut self) {
        let view = self.view;
        let state = ViewState::take(self);
        self.view_states.put(view, state);
    }

    pub(super) fn restore_view(&mut self, view: View) -> bool {
        let Some(state) = self.view_states.take(view) else {
            self.reset_view_presentation(view);
            return false;
        };
        state.restore(self)
    }

    pub(super) fn reset_view_presentation(&mut self, view: View) {
        self.focus = Focus::Sidebar;
        self.filter.clear();
        self.auxiliary_preview = None;
        self.selected_preview_file = None;
        self.preview_file_cursor = 0;
        self.collapsed_preview_files.clear();
        self.expanded_preview_files.clear();
        self.reset_sidebar_scroll();
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.content_at_bottom = true;
        self.pull_request_lookup_active = view == View::PullRequests
            && self.pull_request.is_none()
            && self.recent_pull_requests.is_empty();
        self.reset_local_diff_runtime();
        self.set_document(self.loading_document_for_view(view));
    }

    pub(super) fn reset_view_sidebar_scroll(&mut self, view: View) {
        if self.view == view {
            self.reset_sidebar_scroll();
        } else if let Some(state) = self.view_states.state_mut(view) {
            state.reset_sidebar_scroll();
        }
    }

    pub(super) fn reset_view_content_position(&mut self, view: View) {
        if self.view == view {
            self.content_scroll = 0;
            self.horizontal_scroll = 0;
        } else if let Some(state) = self.view_states.state_mut(view) {
            state.reset_content_position();
        }
    }

    pub(super) fn set_view_document(&mut self, view: View, document: DiffDocument) {
        if self.view == view {
            self.set_document(document);
        } else if let Some(state) = self.view_states.state_mut(view) {
            state.set_document(document);
        }
    }

    pub(super) const fn mark_view_preview_for_resume(&mut self, view: View) {
        if let Some(state) = self.view_states.state_mut(view) {
            state.mark_preview_for_resume();
        }
    }
}
