#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::search::{
    SearchFile, SearchMode, SearchRequest, SearchSource, SearchTarget, name_matches,
};

impl App {
    pub(crate) fn search_header_status(&self) -> Option<String> {
        if self.filter.is_empty() {
            return None;
        }
        let mut label = format!(
            "  Search: {}  [{}]  ",
            self.filter,
            self.search_mode.label()
        );
        if self.search_pending {
            label.push_str("searching...");
        } else {
            let count = match self.view {
                View::Changes => self.visible_change_indices().len(),
                View::History => self.visible_commit_indices().len(),
                View::PullRequests => self
                    .pull_request_tree
                    .iter()
                    .filter(|entry| matches!(entry, PullRequestTreeEntry::File { .. }))
                    .count(),
            };
            label.push_str(&count.to_string());
            label.push_str(if count == 1 { " result" } else { " results" });
        }
        Some(label)
    }

    pub(crate) fn search_title_suffix(&self) -> String {
        if self.filter.is_empty() {
            return String::new();
        }
        let mut suffix = format!("  /{}", self.filter);
        if self.search_mode != SearchMode::Name {
            suffix.push(' ');
            suffix.push_str(self.search_mode.label());
        }
        if self.search_pending {
            suffix.push_str(" …");
        }
        suffix
    }

    pub(super) fn list_item_visible(&self, name: &str, key: &str) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        if self.search_mode == SearchMode::Contents && self.search_pending {
            return true;
        }
        let name_hit = self.search_mode.includes_name() && name_matches(&self.filter, name);
        let content_hit = self.search_mode.includes_contents() && self.content_hits.contains(key);
        match self.search_mode {
            SearchMode::Name => name_hit,
            SearchMode::Contents => content_hit,
            SearchMode::Both => name_hit || content_hit,
        }
    }

    pub(super) fn open_list_search(&mut self) {
        self.modal = Some(Modal::Prompt {
            title: "Search".to_owned(),
            input: TextBuffer::new(self.filter.clone()),
            kind: PromptKind::Filter {
                previous: self.filter.clone(),
                previous_mode: self.search_mode,
                mode: self.search_mode,
            },
        });
    }

    pub(super) const fn cycle_search_mode(&mut self, reverse: bool) {
        if let Some(Modal::Prompt {
            kind: PromptKind::Filter { mode, .. },
            ..
        }) = &mut self.modal
        {
            *mode = if reverse {
                mode.previous()
            } else {
                mode.cycle()
            };
            self.search_mode = *mode;
        }
    }

    pub(super) fn update_live_search(&mut self, value: &str, mode: SearchMode, now: Instant) {
        if self.filter == value && self.search_mode == mode {
            return;
        }
        self.filter.clear();
        self.filter.push_str(value);
        self.search_mode = mode;
        self.schedule_search(now);
        self.normalize_selection();
        self.schedule_preview(now);
    }

    pub(super) fn restore_search(
        &mut self,
        previous: String,
        previous_mode: SearchMode,
        now: Instant,
    ) {
        self.filter = previous;
        self.search_mode = previous_mode;
        self.schedule_search(now);
    }

    pub(super) fn schedule_search(&mut self, now: Instant) {
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_pending = false;
        self.search_due = None;
        self.content_hits.clear();
        if self.filter.is_empty() || !self.search_mode.includes_contents() {
            if self.view == View::PullRequests {
                self.rebuild_pull_request_tree();
            }
            return;
        }
        self.search_pending = true;
        self.search_due = Some(now + PREVIEW_DEBOUNCE);
        if self.view == View::PullRequests {
            self.rebuild_pull_request_tree();
        }
    }

    pub(super) fn request_search(&mut self, effects: &mut Vec<AppEffect>) {
        if self.filter.is_empty() || !self.search_mode.includes_contents() {
            self.search_pending = false;
            self.content_hits.clear();
            return;
        }
        let Some(request) = self.search_request() else {
            self.search_pending = false;
            return;
        };
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_pending = true;
        effects.push(AppEffect::Git(Box::new(WorkerCommand::Search {
            generation: self.search_generation,
            request: Box::new(request),
        })));
    }

    fn search_request(&self) -> Option<SearchRequest> {
        let target = match self.view {
            View::Changes => SearchTarget::Changes {
                files: self
                    .status
                    .changes
                    .iter()
                    .map(|change| SearchFile {
                        path: change.path.clone(),
                        source: if change.area == ChangeArea::Staged {
                            SearchSource::Index
                        } else {
                            SearchSource::Worktree
                        },
                    })
                    .collect(),
            },
            View::History => SearchTarget::History {
                revision: self.history_revision(),
                skip: 0,
                limit: self.history.len().max(HISTORY_PAGE_SIZE),
                selected: self.selected_commit().map(|commit| commit.id.clone()),
            },
            View::PullRequests => {
                let workspace = self.pull_request_workspace_generation?;
                SearchTarget::PullRequest {
                    workspace,
                    paths: self
                        .pull_request_files
                        .iter()
                        .map(|file| file.path.clone())
                        .collect(),
                }
            }
        };
        Some(SearchRequest {
            query: self.filter.clone(),
            mode: self.search_mode,
            target,
        })
    }

    pub(super) fn apply_search_hits(
        &mut self,
        generation: u64,
        result: Result<crate::search::SearchHits, String>,
        now: Instant,
    ) {
        if generation != self.search_generation {
            return;
        }
        self.search_pending = false;
        match result {
            Ok(hits) => {
                self.content_hits.clear();
                self.content_hits.extend(hits.paths);
                self.content_hits.extend(hits.commits);
                if self.view == View::PullRequests {
                    self.rebuild_pull_request_tree();
                }
                self.normalize_selection();
                self.schedule_preview(now);
            }
            Err(error) => self.show_toast(error, ToastLevel::Error, now),
        }
    }
}
