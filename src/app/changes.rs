#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(crate) fn local_diff_load_progress(&self) -> Option<(usize, usize)> {
        self.local_diff_index.as_ref().map(|index| {
            let loaded = if index.files.len() == 1 && self.local_diff_single_loaded {
                1
            } else {
                index
                    .files
                    .iter()
                    .filter(|file| self.local_diff_documents.contains_key(&file.path))
                    .count()
            };
            (loaded, index.files.len())
        })
    }

    pub(crate) fn local_diff_line_counts(&self) -> Option<(usize, usize)> {
        self.local_diff_index.as_ref().map(|index| {
            let counts = index.line_counts();
            (counts.additions, counts.deletions)
        })
    }

    pub(crate) fn selected_change(&self) -> Option<&Change> {
        let visible = self.visible_change_indices();
        visible
            .get(self.change_cursor)
            .and_then(|index| self.status.changes.get(*index))
    }

    pub(crate) fn selected_section_changes(&self) -> Vec<Change> {
        let Some(section) = self.selected_change_section else {
            return Vec::new();
        };
        self.visible_change_indices()
            .into_iter()
            .filter_map(|index| self.status.changes.get(index))
            .filter(|change| section.matches(change))
            .cloned()
            .collect()
    }

    pub(crate) fn change_rows(&self) -> Vec<ChangeRow> {
        let visible = self.visible_change_indices();
        let mut rows = Vec::new();
        for section in ChangeSection::ALL {
            let members = visible
                .iter()
                .enumerate()
                .filter_map(|(cursor, index)| {
                    self.status
                        .changes
                        .get(*index)
                        .filter(|change| section.matches(change))
                        .map(|_| (*index, cursor))
                })
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            if !rows.is_empty() {
                rows.push(ChangeRow::Spacer);
            }
            let collapsed = self.collapsed_change_sections.contains(&section);
            rows.push(ChangeRow::Section {
                section,
                count: members.len(),
                collapsed,
            });
            if !collapsed {
                rows.extend(
                    members
                        .into_iter()
                        .map(|(index, cursor)| ChangeRow::Change {
                            section,
                            index,
                            cursor,
                        }),
                );
            }
        }
        rows
    }

    pub(super) fn change_targets(&self) -> Vec<ChangeTarget> {
        self.change_rows()
            .into_iter()
            .filter_map(|row| match row {
                ChangeRow::Section { section, .. } => Some(ChangeTarget::Section(section)),
                ChangeRow::Change { cursor, .. } => Some(ChangeTarget::Change(cursor)),
                ChangeRow::Spacer => None,
            })
            .collect()
    }

    pub(super) fn selected_change_target(&self) -> Option<ChangeTarget> {
        self.selected_change_section
            .map(ChangeTarget::Section)
            .or(Some(ChangeTarget::Change(self.change_cursor)))
            .filter(|target| self.change_targets().contains(target))
    }

    pub(crate) fn preview_file_selected(&self, path: &str) -> bool {
        self.selected_preview_file
            .as_deref()
            .is_some_and(|selected| selected.to_string_lossy() == path)
    }

    pub(crate) fn preview_files_collapsible(&self) -> bool {
        if self.view == View::PullRequests {
            return self.pull_request_file_view == PullRequestFileView::AllFiles
                && self.pull_request_files.len() > 1;
        }
        let rendered_files = self.document.file_count();
        if self.document_loading && rendered_files > 0 {
            return rendered_files > 1;
        }
        if let Some(index) = self.local_diff_index.as_ref() {
            return index.files.len() > 1;
        }
        self.document
            .lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .take(2)
            .count()
            > 1
    }

    pub(crate) fn preview_file_collapsed(&self, path: &str) -> bool {
        if !self.preview_files_collapsible() {
            return false;
        }
        if self.files_collapsed {
            !self.expanded_preview_files.contains(Path::new(path))
        } else {
            self.collapsed_preview_files.contains(Path::new(path))
        }
    }

    pub(super) fn reset_preview_file_folds(&mut self, paths: &[PathBuf]) {
        self.invalidate_diff_rows();
        self.collapsed_preview_files.clear();
        self.expanded_preview_files.clear();
        if paths.len() > 1 && !self.files_collapsed && !self.collapse_preference_set {
            self.collapsed_preview_files.extend(paths.iter().cloned());
        }
    }

    pub(super) fn refreshed_preview_file_collapsed(&self, path: &Path, total: usize) -> bool {
        if total <= 1 {
            return false;
        }
        if self.files_collapsed {
            return !self.expanded_preview_files.contains(path);
        }
        self.collapsed_preview_files.contains(path)
            || (!self.collapse_preference_set
                && (self.local_diff_preserved_paths.len() <= 1
                    || !self.local_diff_preserved_paths.contains(path)))
    }

    pub(super) fn finalize_refreshed_preview_state(&mut self) {
        let paths = self
            .local_diff_index
            .as_ref()
            .map(|index| {
                index
                    .files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let indexed = paths.iter().cloned().collect::<HashSet<_>>();
        self.collapsed_preview_files
            .retain(|path| indexed.contains(path));
        self.expanded_preview_files
            .retain(|path| indexed.contains(path));
        if !self.files_collapsed && !self.collapse_preference_set && paths.len() > 1 {
            if self.local_diff_preserved_paths.len() <= 1 {
                self.collapsed_preview_files.extend(paths.iter().cloned());
            } else {
                self.collapsed_preview_files.extend(
                    paths
                        .iter()
                        .filter(|path| !self.local_diff_preserved_paths.contains(*path))
                        .cloned(),
                );
            }
        }
        self.selected_preview_file = self
            .selected_preview_file
            .clone()
            .filter(|selected| indexed.contains(selected))
            .or_else(|| paths.first().cloned());
        self.preview_file_cursor = self
            .selected_preview_file
            .as_ref()
            .and_then(|selected| paths.iter().position(|path| path == selected))
            .unwrap_or_default();
        self.local_diff_preserved_paths.clear();
    }

    pub(super) fn visible_preview_paths(&self, paths: &[PathBuf]) -> HashSet<PathBuf> {
        if paths.len() <= 1 {
            return paths.iter().cloned().collect();
        }
        if self.files_collapsed {
            return paths
                .iter()
                .filter(|path| self.expanded_preview_files.contains(*path))
                .cloned()
                .collect();
        }
        paths
            .iter()
            .filter(|path| !self.collapsed_preview_files.contains(*path))
            .cloned()
            .collect()
    }

    pub(super) fn toggle_all_preview_files(&mut self, effects: &mut Vec<AppEffect>) {
        if !self.preview_files_collapsible() {
            return;
        }
        self.files_collapsed = !self.preview_files_all_collapsed();
        self.collapse_preference_set = true;
        self.invalidate_diff_rows();
        self.collapsed_preview_files.clear();
        self.expanded_preview_files.clear();
        self.content_scroll = 0;
        self.rebuild_indexed_preview_document();
        if self.files_collapsed {
            self.local_diff_pending_paths.clear();
        } else {
            for path in self.preview_file_paths() {
                self.request_local_diff_file(path, effects);
            }
        }
    }

    pub(super) fn toggle_preview_file(&mut self, path: PathBuf, effects: &mut Vec<AppEffect>) {
        if !self.preview_files_collapsible() {
            return;
        }
        let was_collapsed = self.preview_file_collapsed(&path.to_string_lossy());
        let overrides = if self.files_collapsed {
            &mut self.expanded_preview_files
        } else {
            &mut self.collapsed_preview_files
        };
        toggle_membership(overrides, path.clone());
        self.invalidate_diff_rows();
        self.selected_preview_file = Some(path.clone());
        self.preview_file_cursor = self
            .preview_file_paths()
            .iter()
            .position(|candidate| candidate == &path)
            .unwrap_or_default();
        let is_collapsed = self.preview_file_collapsed(&path.to_string_lossy());
        self.rebuild_indexed_preview_document();
        if was_collapsed && !is_collapsed {
            if self.view == View::PullRequests
                && self.pull_request_file_view == PullRequestFileView::AllFiles
            {
                self.request_pull_request_diff_file(path, false, effects);
            } else {
                self.request_local_diff_file(path, effects);
            }
        }
    }

    pub(crate) fn preview_files_all_collapsed(&self) -> bool {
        let paths = self.preview_file_paths();
        paths.len() > 1
            && paths
                .iter()
                .all(|path| self.preview_file_collapsed(&path.to_string_lossy()))
    }

    pub(super) fn preview_file_paths(&self) -> Vec<PathBuf> {
        self.document
            .lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .filter_map(|line| line.spans.first())
            .map(|span| PathBuf::from(span.text.split("  · ").next().unwrap_or(span.text.as_str())))
            .collect()
    }

    pub(super) fn navigate_preview_file(&mut self, amount: isize) {
        let paths = self.preview_file_paths();
        if paths.is_empty() {
            return;
        }
        let current = self
            .selected_preview_file
            .as_ref()
            .and_then(|selected| paths.iter().position(|path| path == selected))
            .unwrap_or_else(|| self.preview_file_cursor.min(paths.len() - 1));
        self.preview_file_cursor = if amount < 0 {
            current.saturating_sub(amount.unsigned_abs())
        } else {
            (current + count(amount)).min(paths.len() - 1)
        };
        self.selected_preview_file = paths.get(self.preview_file_cursor).cloned();
        if let Some(line_index) = self.document.lines.iter().position(|line| {
            line.kind == DiffLineKind::FileHeader
                && line.spans.first().is_some_and(|span| {
                    span.text.split("  · ").next().is_some_and(|path| {
                        paths
                            .get(self.preview_file_cursor)
                            .is_some_and(|selected| Path::new(path) == selected)
                    })
                })
        }) {
            self.content_scroll = line_index;
        }
    }

    pub(super) const fn select_change_target(&mut self, target: ChangeTarget) {
        match target {
            ChangeTarget::Section(section) => self.selected_change_section = Some(section),
            ChangeTarget::Change(cursor) => {
                self.selected_change_section = None;
                self.change_cursor = cursor;
            }
        }
    }

    pub(super) fn toggle_change_section(&mut self, section: ChangeSection) {
        toggle_membership(&mut self.collapsed_change_sections, section);
        self.selected_change_section = Some(section);
    }

    pub(super) fn toggle_selected_change_section(&mut self) -> bool {
        let Some(section) = self.selected_change_section else {
            return false;
        };
        self.toggle_change_section(section);
        true
    }

    pub(super) fn navigate_change_section_horizontal(&mut self, expand: bool, now: Instant) {
        let Some(target) = self.selected_change_target() else {
            return;
        };
        match target {
            ChangeTarget::Section(section) => {
                let collapsed = self.collapsed_change_sections.contains(&section);
                if expand && !collapsed {
                    let targets = self.change_targets();
                    if let Some(index) = targets
                        .iter()
                        .position(|candidate| *candidate == target)
                        .and_then(|index| targets.get(index.saturating_add(1)))
                        .copied()
                        .filter(|candidate| matches!(candidate, ChangeTarget::Change(_)))
                    {
                        self.select_change_target(index);
                        self.schedule_preview(now);
                    }
                }
            }
            ChangeTarget::Change(_) if !expand => {
                let Some(change) = self.selected_change() else {
                    return;
                };
                if let Some(section) = ChangeSection::ALL
                    .into_iter()
                    .find(|section| section.matches(change))
                {
                    self.selected_change_section = Some(section);
                    self.schedule_preview(now);
                }
            }
            ChangeTarget::Change(_) => {}
        }
    }

    pub(crate) fn selected_commit(&self) -> Option<&Commit> {
        let visible = self.visible_commit_indices();
        visible
            .get(self.history_cursor)
            .and_then(|index| self.history.get(*index))
    }

    #[expect(
        clippy::unused_self,
        reason = "the method belongs to the app surface even without state"
    )]
    pub(crate) fn palette_commands(&self, query: &str) -> Vec<PaletteCommand> {
        let words: Vec<_> = query
            .split_ascii_whitespace()
            .map(str::to_lowercase)
            .collect();
        PaletteCommand::ALL
            .into_iter()
            .filter(|command| {
                let label = command.label().to_lowercase();
                words.iter().all(|word| label.contains(word))
            })
            .collect()
    }

    pub(crate) fn filtered_branches(items: &[Branch], query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, branch)| query.is_empty() || branch.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn filtered_history_branches(items: &[HistoryBranch], query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, branch)| query.is_empty() || branch.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn filtered_stashes(items: &[Stash], query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, stash)| {
                query.is_empty()
                    || stash.reference.to_lowercase().contains(&query)
                    || stash.message.to_lowercase().contains(&query)
                    || stash.branch.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn filtered_project_rows(
        groups: &[ProjectGroup],
        query: &str,
    ) -> Vec<(usize, usize)> {
        let query = query.to_lowercase();
        let mut rows = Vec::new();
        for (group_index, group) in groups.iter().enumerate() {
            let group_matches = query.is_empty() || group.name.to_lowercase().contains(&query);
            for (tree_index, tree) in group.worktrees.iter().enumerate() {
                let tree_matches = tree.path.to_string_lossy().to_lowercase().contains(&query)
                    || tree.branch_label().to_lowercase().contains(&query);
                if group_matches || tree_matches {
                    rows.push((group_index, tree_index));
                }
            }
        }
        rows
    }

    pub(crate) fn worktree_path_for_branch(&self, name: &str) -> Option<&Path> {
        self.worktrees.iter().find_map(|tree| {
            (tree.branch.as_deref() == Some(name) && !tree.current).then_some(tree.path.as_path())
        })
    }

    pub(crate) fn filtered_github_repositories(
        items: &[GitHubRepository],
        query: &str,
    ) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, repository)| {
                query.is_empty()
                    || repository.display_name().to_lowercase().contains(&query)
                    || repository
                        .remotes
                        .iter()
                        .any(|remote| remote.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect()
    }
}
