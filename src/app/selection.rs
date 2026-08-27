#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(crate) fn visible_change_indices(&self) -> Vec<usize> {
        let query = self.filter.to_lowercase();
        self.status
            .changes
            .iter()
            .enumerate()
            .filter(|(_, change)| {
                query.is_empty() || change.display_path().to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn visible_commit_indices(&self) -> Vec<usize> {
        let query = self.filter.to_lowercase();
        self.history
            .iter()
            .enumerate()
            .filter(|(_, commit)| {
                query.is_empty()
                    || commit.subject.to_lowercase().contains(&query)
                    || commit.author.to_lowercase().contains(&query)
                    || commit.id.starts_with(&query)
                    || commit
                        .decorations
                        .iter()
                        .any(|decoration| decoration.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn history_branch_label(&self) -> String {
        self.history_branch.as_ref().map_or_else(
            || {
                if self.status.branch.head.is_empty() {
                    "HEAD".to_owned()
                } else {
                    self.status.branch.head.clone()
                }
            },
            |branch| branch.name.clone(),
        )
    }

    pub(super) fn history_revision(&self) -> String {
        self.history_branch
            .as_ref()
            .map_or_else(|| "HEAD".to_owned(), |branch| branch.reference.clone())
    }

    pub(crate) fn selected_pull_request(&self) -> Option<&PullRequest> {
        if self.pull_request_section == PullRequestSection::Stack {
            return self
                .stack_inspector
                .selected_pull_request
                .as_ref()
                .or(self.stack_inspector.selected_locator.as_ref())
                .or(self.pull_request.as_ref());
        }
        self.pull_request.as_ref()
    }

    pub(super) fn move_recent_pull_request_cursor(&mut self, delta: isize) {
        if self.recent_pull_requests.is_empty() {
            self.recent_pull_request_cursor = 0;
            return;
        }
        self.recent_pull_request_cursor = if delta < 0 {
            self.recent_pull_request_cursor
                .saturating_sub(delta.unsigned_abs())
        } else {
            self.recent_pull_request_cursor
                .saturating_add(count(delta))
                .min(self.recent_pull_requests.len() - 1)
        };
    }

    pub(super) fn open_recent_pull_request(
        &mut self,
        index: usize,
        effects: &mut Vec<AppEffect>,
    ) -> bool {
        let Some(recent) = self.recent_pull_requests.get(index).cloned() else {
            return false;
        };
        self.recent_pull_request_cursor = index;
        if !self
            .github_repositories
            .iter()
            .any(|repository| repository.url.eq_ignore_ascii_case(&recent.repository.url))
        {
            self.github_repositories.push(recent.repository.clone());
        }
        self.pull_request_repository = Some(recent.repository);
        self.pull_request_lookup = TextBuffer::new(recent.number.to_string());
        self.pull_request_lookup_active = false;
        self.request_pull_request_lookup(recent.number, false, false, effects);
        true
    }

    pub(crate) fn selected_pull_request_file(&self) -> Option<&PullRequestFile> {
        self.pull_request_files.get(self.pull_request_file_cursor)
    }

    pub(crate) fn selected_pull_request_check(&self) -> Option<&PullRequestCheck> {
        self.pull_request_check_cursor
            .and_then(|cursor| self.pull_request_checks.get(cursor))
    }

    pub(crate) fn check_list_rows(&self) -> Vec<CheckListRow> {
        let mut rows = vec![CheckListRow::Conversation];
        if self.pull_request_checks.is_empty() {
            return rows;
        }
        rows.push(CheckListRow::Spacer);
        rows.push(CheckListRow::Heading);
        for section in CheckStatusSection::ALL {
            let members = self
                .pull_request_checks
                .iter()
                .enumerate()
                .filter(|(_, check)| section.matches(check.status))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            let collapsed = self.collapsed_check_sections.contains(&section);
            rows.push(CheckListRow::Section {
                section,
                count: members.len(),
                collapsed,
            });
            if !collapsed {
                rows.extend(
                    members
                        .into_iter()
                        .map(|index| CheckListRow::Check { index }),
                );
            }
        }
        rows
    }

    pub(super) fn check_list_targets(&self) -> Vec<CheckListTarget> {
        self.check_list_rows()
            .into_iter()
            .filter_map(|row| match row {
                CheckListRow::Conversation => Some(CheckListTarget::Conversation),
                CheckListRow::Section { section, .. } => Some(CheckListTarget::Section(section)),
                CheckListRow::Check { index } => Some(CheckListTarget::Check(index)),
                CheckListRow::Heading | CheckListRow::Spacer => None,
            })
            .collect()
    }

    pub(super) fn selected_check_list_target(&self) -> CheckListTarget {
        self.selected_check_section.map_or_else(
            || {
                self.pull_request_check_cursor
                    .map_or(CheckListTarget::Conversation, CheckListTarget::Check)
            },
            CheckListTarget::Section,
        )
    }

    pub(super) fn select_check_list_target(&mut self, target: CheckListTarget) {
        match target {
            CheckListTarget::Conversation => {
                self.selected_check_section = None;
                let _ = self.set_check_cursor(None);
            }
            CheckListTarget::Section(section) => {
                self.selected_check_section = Some(section);
                let _ = self.set_check_cursor(None);
            }
            CheckListTarget::Check(index) => {
                let _ = self.set_check_cursor(Some(index));
            }
        }
    }

    pub(super) fn toggle_check_section(&mut self, section: CheckStatusSection) {
        toggle_membership(&mut self.collapsed_check_sections, section);
        self.selected_check_section = Some(section);
        if self.collapsed_check_sections.contains(&section)
            && self
                .selected_pull_request_check()
                .is_some_and(|check| section.matches(check.status))
        {
            let _ = self.set_check_cursor(None);
        }
    }

    pub(super) fn toggle_selected_check_section(&mut self) -> bool {
        let Some(section) = self.selected_check_section else {
            return false;
        };
        self.toggle_check_section(section);
        true
    }

    pub(super) fn navigate_check_section_horizontal(&mut self, expand: bool, now: Instant) {
        match self.selected_check_list_target() {
            CheckListTarget::Section(section) => {
                let collapsed = self.collapsed_check_sections.contains(&section);
                if expand && !collapsed {
                    let targets = self.check_list_targets();
                    let current = CheckListTarget::Section(section);
                    if let Some(target) = targets
                        .iter()
                        .position(|candidate| *candidate == current)
                        .and_then(|index| targets.get(index.saturating_add(1)))
                        .copied()
                        .filter(|candidate| matches!(candidate, CheckListTarget::Check(_)))
                    {
                        self.select_check_list_target(target);
                        self.schedule_preview(now);
                    }
                }
            }
            CheckListTarget::Check(_) if !expand => {
                let Some(check) = self.selected_pull_request_check() else {
                    return;
                };
                if let Some(section) = CheckStatusSection::ALL
                    .into_iter()
                    .find(|section| section.matches(check.status))
                {
                    self.selected_check_section = Some(section);
                    let _ = self.set_check_cursor(None);
                    self.schedule_preview(now);
                }
            }
            CheckListTarget::Conversation | CheckListTarget::Check(_) => {}
        }
    }

    pub(super) fn rebuild_pull_request_tree(&mut self) {
        let mut entries = Vec::with_capacity(self.pull_request_files.len().saturating_mul(2));
        let mut root = PullRequestTreeNode::default();
        for (index, file) in self.pull_request_files.iter().enumerate() {
            root.insert(&file.path, index);
        }
        root.append_entries(0, &self.collapsed_pull_request_directories, &mut entries);
        self.pull_request_tree = entries;
    }

    pub(crate) fn pull_request_tree_entries(&mut self) -> &[PullRequestTreeEntry] {
        if self.pull_request_tree.is_empty() && !self.pull_request_files.is_empty() {
            self.rebuild_pull_request_tree();
        }
        &self.pull_request_tree
    }

    pub(crate) fn pull_request_directory_collapsed(&self, path: &Path) -> bool {
        self.collapsed_pull_request_directories.contains(path)
    }

    pub(super) fn sync_pull_request_tree_cursor_to_file(&mut self) {
        self.rebuild_pull_request_tree();
        self.pull_request_tree_cursor = self
            .pull_request_tree
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    PullRequestTreeEntry::File { index, .. }
                        if *index == self.pull_request_file_cursor
                )
            })
            .unwrap_or_default();
    }

    pub(super) fn select_pull_request_tree_entry(&mut self, cursor: usize, now: Instant) {
        let entries = self.pull_request_tree_entries();
        let length = entries.len();
        let Some(entry) = entries.get(cursor.min(length.saturating_sub(1))).cloned() else {
            self.pull_request_tree_cursor = 0;
            return;
        };
        self.pull_request_tree_cursor = cursor.min(length - 1);
        if let PullRequestTreeEntry::File { index, .. } = entry {
            let changed_file = index != self.pull_request_file_cursor;
            let entering_single_file =
                self.pull_request_file_view != PullRequestFileView::SingleFile;
            if changed_file || entering_single_file {
                self.pull_request_file_cursor = index;
                self.pull_request_file_view = PullRequestFileView::SingleFile;
                self.content_scroll = 0;
                self.horizontal_scroll = 0;
                self.schedule_preview(now);
            }
        }
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the caller has no use for the value afterwards"
    )]
    pub(super) fn toggle_pull_request_directory(&mut self, path: PathBuf) {
        toggle_membership(&mut self.collapsed_pull_request_directories, path.clone());
        self.rebuild_pull_request_tree();
        self.pull_request_tree_cursor = self
            .pull_request_tree
            .iter()
            .position(|entry| {
                matches!(entry, PullRequestTreeEntry::Directory { path: entry_path, .. } if entry_path == &path)
            })
            .unwrap_or_default();
    }

    pub(super) fn toggle_selected_pull_request_directory(&mut self) -> bool {
        if self.view != View::PullRequests
            || self.pull_request_section != PullRequestSection::Files
            || self.focus != Focus::Sidebar
        {
            return false;
        }
        let cursor = self.pull_request_tree_cursor;
        let Some(PullRequestTreeEntry::Directory { path, .. }) =
            self.pull_request_tree_entries().get(cursor).cloned()
        else {
            return false;
        };
        self.toggle_pull_request_directory(path);
        true
    }

    pub(super) fn navigate_pull_request_tree_horizontal(&mut self, expand: bool, now: Instant) {
        if self.pull_request_tree_entries().is_empty() {
            return;
        }
        let entries = &self.pull_request_tree;
        let Some(entry) = entries.get(self.pull_request_tree_cursor).cloned() else {
            return;
        };
        match entry {
            PullRequestTreeEntry::Directory { path, depth, .. } => {
                let collapsed = self.pull_request_directory_collapsed(&path);
                if expand && !collapsed {
                    let child = self.pull_request_tree_cursor.saturating_add(1);
                    if entries
                        .get(child)
                        .is_some_and(|entry| entry.depth() > depth)
                    {
                        self.select_pull_request_tree_entry(child, now);
                    }
                } else if !expand {
                    let parent_cursor =
                        entries
                            .get(..self.pull_request_tree_cursor)
                            .and_then(|parents| {
                                parents.iter().rposition(|entry| {
                                    entry
                                        .directory_depth()
                                        .is_some_and(|parent_depth| parent_depth < depth)
                                })
                            });
                    if let Some(cursor) = parent_cursor {
                        self.pull_request_tree_cursor = cursor;
                    }
                }
            }
            PullRequestTreeEntry::File { depth, .. } if !expand => {
                let parent_cursor =
                    entries
                        .get(..self.pull_request_tree_cursor)
                        .and_then(|parents| {
                            parents.iter().rposition(|entry| {
                                entry
                                    .directory_depth()
                                    .is_some_and(|parent_depth| parent_depth < depth)
                            })
                        });
                if let Some(cursor) = parent_cursor {
                    self.pull_request_tree_cursor = cursor;
                }
            }
            PullRequestTreeEntry::File { .. } => {}
        }
    }
}
