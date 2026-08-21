#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn reset_local_diff_runtime(&mut self) {
        self.local_diff_request = None;
        self.local_diff_change_section = None;
        self.local_diff_preserved_paths.clear();
        self.local_diff_workspace_generation = None;
        self.local_diff_index = None;
        self.local_diff_documents.clear();
        self.local_diff_loading_path = None;
        self.local_diff_pending_paths.clear();
        self.local_diff_single_loaded = false;
    }

    pub(super) fn rebuild_indexed_preview_document(&mut self) {
        if self.view == View::PullRequests
            && self.pull_request_file_view == PullRequestFileView::AllFiles
        {
            self.rebuild_pull_request_all_files_document();
        } else if self.local_diff_index.is_some() {
            self.rebuild_local_diff_document();
        }
    }

    pub(super) fn rebuild_local_diff_document(&mut self) {
        let Some(index) = &self.local_diff_index else {
            return;
        };
        let document = if index.files.is_empty()
            && matches!(
                self.local_diff_request.as_ref(),
                Some(LocalDiffRequest::Changes { .. })
            ) {
            DiffDocument::empty(
                &index.title,
                if self.status.changes.is_empty() {
                    "Working tree clean — no changes"
                } else {
                    "No changes match the current filter"
                },
            )
        } else {
            let paths = index
                .files
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>();
            let visible = self.visible_preview_paths(&paths);
            index
                .document_with_visibility(&self.local_diff_documents, |path| visible.contains(path))
        };
        self.set_document(document);
    }

    pub(super) fn store_local_diff_document(&mut self, path: PathBuf, document: DiffDocument) {
        if self
            .local_diff_index
            .as_ref()
            .is_some_and(|index| index.files.len() == 1)
        {
            self.set_document(document);
            self.local_diff_single_loaded = true;
            self.document_loading = false;
        } else {
            drop(self.local_diff_documents.insert(path, document));
            if !self.document_loading {
                self.rebuild_local_diff_document();
            }
        }
    }

    pub(super) fn request_local_diff_file(&mut self, path: PathBuf, effects: &mut Vec<AppEffect>) {
        let Some(workspace_generation) = self.local_diff_workspace_generation else {
            return;
        };
        let indexed = self
            .local_diff_index
            .as_ref()
            .is_some_and(|index| index.files.iter().any(|file| file.path == path));
        if !indexed
            || self.local_diff_documents.contains_key(&path)
            || self.local_diff_loading_path.as_ref() == Some(&path)
            || self.local_diff_pending_paths.contains(&path)
        {
            return;
        }
        if self.local_diff_loading_path.is_some() {
            self.local_diff_pending_paths.push_back(path);
            return;
        }
        self.local_diff_loading_path = Some(path.clone());
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadLocalDiffFile {
            generation: self.diff_generation,
            workspace_generation,
            path,
        })));
    }

    pub(super) fn request_next_local_diff_file(&mut self, effects: &mut Vec<AppEffect>) {
        while let Some(path) = self.local_diff_pending_paths.pop_front() {
            if !self.preview_file_collapsed(&path.to_string_lossy())
                && !self.local_diff_documents.contains_key(&path)
            {
                self.request_local_diff_file(path, effects);
                return;
            }
        }
        if self.document_loading && self.local_diff_index.is_some() {
            self.document_loading = false;
            self.rebuild_local_diff_document();
        }
    }
}
