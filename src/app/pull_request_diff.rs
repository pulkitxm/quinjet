use super::*;

impl App {
    /// Drop only the prepared diff. The section, cursors, checks and
    /// conversation stay exactly where the reader left them.
    pub(super) fn reset_pull_request_diff_runtime(&mut self) {
        self.pull_request_workspace_generation = None;
        self.pull_request_documents.clear();
        self.pull_request_document_order.clear();
        self.pull_request_document_bytes = 0;
        self.pull_request_prefetched_paths.clear();
        self.pull_request_loading_path = None;
        self.pull_request_single_file = None;
        self.pull_request_prefetching = false;
        self.pull_request_prefetch_retrying = false;
    }

    pub(super) fn reset_pull_request_runtime(&mut self) {
        self.reset_pull_request_diff_runtime();
        self.pull_request_section = PullRequestSection::Overview;
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.pull_request_files.clear();
        self.pull_request_tree.clear();
        self.pull_request_total_files = 0;
        self.pull_request_files_truncated = false;
        self.pull_request_file_cursor = 0;
        self.pull_request_tree_cursor = 0;
        self.collapsed_pull_request_directories.clear();
        self.pull_request_checks.clear();
        self.pull_request_check_cursor = None;
        self.selected_check_section = None;
        self.collapsed_check_sections.clear();
        self.pull_request_checks_loading = false;
        self.pull_request_checks_error = None;
        self.pull_request_checks_generation = self.pull_request_checks_generation.wrapping_add(1);
        self.pull_request_prefetched_logs.clear();
        self.pull_request_conversation = PullRequestConversation::default();
        self.pull_request_conversation_loading = false;
        self.pull_request_conversation_refresh_again = false;
        self.pull_request_conversation_error = None;
        self.pull_request_conversation_generation =
            self.pull_request_conversation_generation.wrapping_add(1);
        self.pull_request_check_log = None;
        self.pull_request_check_log_loading = false;
        self.pull_request_check_log_error = None;
        self.pull_request_check_log_target = None;
        self.pull_request_check_log_generation =
            self.pull_request_check_log_generation.wrapping_add(1);
        self.expanded_check_steps.clear();
        self.pull_request_step_cursor = 0;
        self.pull_request_content_rows.clear();
        self.pull_request_content_rows_key = None;
        self.pull_request_content_width = 0;
        self.pull_request_content_links.clear();
        self.pull_request_content_generation = self.pull_request_content_generation.wrapping_add(1);
        self.pull_request_checks_read_at = None;
        self.pull_request_detail_read_at = None;
        self.pull_request_log_read_at = None;
        self.reset_sidebar_scroll();
    }

    pub(super) fn apply_pull_request_index(&mut self, index: PullRequestDiffIndex) {
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.pull_request_documents.clear();
        self.pull_request_loading_path = None;
        self.pull_request_single_file = None;
        self.pull_request_prefetching = false;
        self.pull_request_prefetch_retrying = false;
        self.pull_request_files = index.files;
        self.pull_request_total_files = index.total_files;
        self.pull_request_files_truncated = index.truncated;
        self.pull_request_file_cursor = self
            .pull_request_file_cursor
            .min(self.pull_request_files.len().saturating_sub(1));
        self.sync_pull_request_tree_cursor_to_file();
        let paths = self
            .pull_request_files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        self.reset_preview_file_folds(&paths);
        self.selected_preview_file = self
            .selected_pull_request_file()
            .map(|file| file.path.clone());
        self.preview_file_cursor = self.pull_request_file_cursor;
        self.rebuild_pull_request_all_files_document();
    }

    pub(super) fn rebuild_pull_request_all_files_document(&mut self) {
        let Some(pull_request) = self.pull_request.as_ref() else {
            return;
        };
        let title = format!(
            "PR #{} — All Files · {} changed",
            pull_request.number, self.pull_request_total_files
        );
        if self.pull_request_files.is_empty() {
            let mut document = DiffDocument::empty(
                title,
                if self.pull_request_files_truncated {
                    "The changed-file index was truncated before any paths were read"
                } else {
                    "This pull request has no changed files"
                },
            );
            document.pull_request_details = Some(pull_request_details(pull_request));
            self.set_document(document);
            return;
        }
        let index = DiffIndex {
            title,
            files: self
                .pull_request_files
                .iter()
                .map(|file| crate::git::diff::DiffFileIndexEntry {
                    path: file.path.clone(),
                    old_path: file.old_path.clone(),
                    status: pull_request_file_status_label(file.status).to_owned(),
                    counts: file.counts,
                })
                .collect(),
            truncated: self.pull_request_files_truncated,
            commit_details: None,
        };
        let paths = index
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let visible = self.visible_preview_paths(&paths);
        let mut document = index
            .document_with_visibility(&self.pull_request_documents, |path| visible.contains(path));
        document.pull_request_details = Some(pull_request_details(pull_request));
        self.set_document(document);
    }

    pub(super) fn cache_current_pull_request_single_document(&mut self) {
        let Some(path) = self.pull_request_single_file.take() else {
            return;
        };
        if self.pull_request_documents.contains_key(&path) {
            return;
        }
        let document = std::mem::take(&mut self.document);
        self.invalidate_diff_rows();
        self.cache_pull_request_document(path, document);
    }

    pub(super) fn cache_pull_request_document(&mut self, path: PathBuf, document: DiffDocument) {
        if let Some(previous) = self.pull_request_documents.remove(&path) {
            self.pull_request_document_bytes = self
                .pull_request_document_bytes
                .saturating_sub(diff_document_size(&previous));
            self.pull_request_document_order
                .retain(|candidate| candidate != &path);
        }
        self.pull_request_document_bytes = self
            .pull_request_document_bytes
            .saturating_add(diff_document_size(&document));
        let _ = self.pull_request_prefetched_paths.insert(path.clone());
        self.pull_request_document_order.push_back(path.clone());
        drop(self.pull_request_documents.insert(path, document));
        self.prune_pull_request_documents(MAX_PULL_REQUEST_DOCUMENT_BYTES);
    }

    pub(super) fn prune_pull_request_documents(&mut self, maximum_bytes: usize) {
        while self.pull_request_document_bytes > maximum_bytes
            && self.pull_request_documents.len() > 1
        {
            let Some(expired) = self.pull_request_document_order.pop_front() else {
                break;
            };
            if let Some(document) = self.pull_request_documents.remove(&expired) {
                self.pull_request_document_bytes = self
                    .pull_request_document_bytes
                    .saturating_sub(diff_document_size(&document));
            }
        }
    }

    pub(super) fn take_pull_request_document(&mut self, path: &Path) -> Option<DiffDocument> {
        let document = self.pull_request_documents.remove(path)?;
        self.pull_request_document_bytes = self
            .pull_request_document_bytes
            .saturating_sub(diff_document_size(&document));
        self.pull_request_document_order
            .retain(|candidate| candidate != path);
        Some(document)
    }

    pub(super) fn show_pull_request_all_files(&mut self) {
        self.cache_current_pull_request_single_document();
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.pull_request_loading_path = None;
        self.document_loading = false;
        self.selected_preview_file = self
            .selected_pull_request_file()
            .map(|file| file.path.clone());
        self.preview_file_cursor = self.pull_request_file_cursor;
        self.rebuild_pull_request_all_files_document();
    }

    pub(super) fn select_pull_request_section(
        &mut self,
        section: PullRequestSection,
        effects: &mut Vec<AppEffect>,
    ) {
        if section == PullRequestSection::Files {
            if self.pull_request_section == PullRequestSection::Files
                && self.pull_request_file_view == PullRequestFileView::AllFiles
            {
                return;
            }
            self.invalidate_preview();
            self.pull_request_section = PullRequestSection::Files;
            self.reset_sidebar_scroll();
            self.content_scroll = 0;
            self.horizontal_scroll = 0;
            self.show_pull_request_all_files();
            self.request_preview(effects);
            return;
        }
        if self.pull_request_section == section {
            return;
        }
        self.invalidate_preview();
        self.pull_request_section = section;
        self.reset_sidebar_scroll();
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.request_pull_request_checks(false, effects);
        self.request_pull_request_conversation(false, effects);
        self.request_check_run_log(false, effects);
    }

    pub(super) fn request_pull_request_diff_file(
        &mut self,
        path: PathBuf,
        show_loading: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        if !self.pull_request_files.iter().any(|file| file.path == path) {
            return;
        }
        if self.pull_request_file_view == PullRequestFileView::SingleFile {
            if self.pull_request_documents.contains_key(&path) {
                self.cache_current_pull_request_single_document();
            }
            if let Some(document) = self.take_pull_request_document(&path) {
                self.document_loading = false;
                self.set_document(document);
                self.pull_request_single_file = Some(path);
                self.selected_preview_file = None;
                self.preview_file_cursor = 0;
                return;
            }
        } else if self.pull_request_documents.contains_key(&path) {
            self.document_loading = false;
            self.rebuild_pull_request_all_files_document();
            return;
        }
        let Some(workspace_generation) = self.pull_request_workspace_generation else {
            return;
        };
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.pull_request_loading_path = Some(path.clone());
        self.document_loading = show_loading;
        self.pull_request_progress = None;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestFile {
                generation: self.diff_generation,
                workspace_generation,
                path,
            },
        )));
    }

    /// A path still needs its patch unless it is already cached, already in
    /// flight, or currently occupying the single-file document.
    pub(super) fn pull_request_file_needs_patch(&self, path: &Path) -> bool {
        !self.pull_request_documents.contains_key(path)
            && self.pull_request_loading_path.as_deref() != Some(path)
            && self.pull_request_single_file.as_deref() != Some(path)
    }

    /// A finished patch knows its real totals, so a file whose counts GitHub
    /// could not report fills its header in as soon as its document arrives.
    pub(super) fn backfill_pull_request_counts(
        &mut self,
        path: &Path,
        document: &DiffDocument,
    ) -> bool {
        if document.truncated {
            return false;
        }
        let Some(file) = self
            .pull_request_files
            .iter_mut()
            .find(|file| file.path == path && file.counts.is_none())
        else {
            return false;
        };
        let mut additions = 0_usize;
        let mut deletions = 0_usize;
        for line in &document.lines {
            match line.kind {
                DiffLineKind::Added => additions = additions.saturating_add(1),
                DiffLineKind::Removed => deletions = deletions.saturating_add(1),
                _ => {}
            }
        }
        file.counts = Some(DiffLineCounts {
            additions,
            deletions,
            binary: false,
        });
        true
    }

    /// Where background fill should start: the first file visible in the
    /// Files tree, so patches land where the reader is looking and then wrap
    /// around the rest of the index in order.
    pub(super) fn prefetch_anchor_index(&self) -> usize {
        if self.view != View::PullRequests || self.pull_request_section != PullRequestSection::Files
        {
            return 0;
        }
        self.pull_request_tree
            .iter()
            .skip(self.sidebar_offset)
            .find_map(|entry| match entry {
                PullRequestTreeEntry::File { index, .. } => Some(*index),
                PullRequestTreeEntry::Directory { .. } => None,
            })
            .unwrap_or(0)
    }

    /// Walk the index in batches until every file has a patch. Each batch is one
    /// Git invocation and lands as soon as it is parsed, so the diff fills in
    /// progressively instead of a file at a time on demand.
    pub(super) fn request_pull_request_prefetch(&mut self, effects: &mut Vec<AppEffect>) {
        if self.pull_request_prefetching {
            return;
        }
        let Some(workspace_generation) = self.pull_request_workspace_generation else {
            return;
        };
        if self.pull_request_prefetched_paths.len() >= MAX_PREFETCHED_PULL_REQUEST_FILES {
            return;
        }
        let remaining = MAX_PREFETCHED_PULL_REQUEST_FILES
            .saturating_sub(self.pull_request_prefetched_paths.len());
        let limit = PULL_REQUEST_PREFETCH_BATCH.min(remaining);
        let anchor = self
            .prefetch_anchor_index()
            .min(self.pull_request_files.len());
        let (before, from_anchor) = self.pull_request_files.split_at(anchor);
        let mut batch_bytes = 0_usize;
        let mut paths: Vec<PathBuf> = Vec::new();
        for file in from_anchor.iter().chain(before.iter()) {
            if paths.len() >= limit {
                break;
            }
            if !self.pull_request_file_needs_patch(&file.path)
                || self.pull_request_prefetched_paths.contains(&file.path)
            {
                continue;
            }
            let estimate = estimated_patch_bytes(file.counts);
            if !paths.is_empty()
                && batch_bytes.saturating_add(estimate) > PULL_REQUEST_PREFETCH_BYTE_BUDGET
            {
                break;
            }
            batch_bytes = batch_bytes.saturating_add(estimate);
            paths.push(file.path.clone());
        }
        if paths.is_empty() {
            return;
        }
        self.pull_request_prefetching = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation,
                paths,
            },
        )));
    }
}
