#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn request_pull_request_prefetch(&mut self, effects: &mut Vec<AppEffect>) {
        if self.pull_request_prefetching
            || !self.tab_active
            || self.view != View::PullRequests
            || self.pull_request_section != PullRequestSection::Files
            || self.pull_request_file_view != PullRequestFileView::AllFiles
        {
            return;
        }
        let Some(workspace_generation) = self.pull_request_workspace_generation else {
            return;
        };
        let mut batch_bytes = 0_usize;
        let mut paths = Vec::new();
        for path in &self.pull_request_viewport_paths {
            if paths.len() >= PULL_REQUEST_PREFETCH_BATCH {
                break;
            }
            if !self.pull_request_file_needs_patch(path)
                || self.preview_file_collapsed(&path.to_string_lossy())
            {
                continue;
            }
            let Some(file) = self
                .pull_request_files
                .iter()
                .find(|file| &file.path == path)
            else {
                continue;
            };
            let estimate = estimated_patch_bytes(file.counts);
            if !paths.is_empty()
                && batch_bytes.saturating_add(estimate) > PULL_REQUEST_PREFETCH_BYTE_BUDGET
            {
                break;
            }
            batch_bytes = batch_bytes.saturating_add(estimate);
            paths.push(path.clone());
        }
        if paths.is_empty() {
            return;
        }
        self.pull_request_prefetching = true;
        self.pull_request_prefetch_paths.clone_from(&paths);
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation,
                paths,
            },
        )));
    }
}
