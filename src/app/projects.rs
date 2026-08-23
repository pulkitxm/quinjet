#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectOpenMode {
    Initial,
    CurrentTab,
    NewTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectRow {
    Group(usize),
    Worktree {
        group_index: usize,
        tree_index: usize,
    },
}

impl App {
    pub(crate) fn first_project_worktree_index(
        groups: &[ProjectGroup],
        query: &str,
        collapsed: &HashSet<PathBuf>,
    ) -> usize {
        Self::filtered_project_rows(groups, query, collapsed)
            .iter()
            .position(|row| matches!(row, ProjectRow::Worktree { .. }))
            .unwrap_or_default()
    }

    pub(super) fn remember_collapsed_project_groups(&mut self, collapsed: &HashSet<PathBuf>) {
        self.collapsed_project_groups.clone_from(collapsed);
        #[cfg(not(test))]
        crate::state::record_collapsed_project_groups(collapsed);
    }

    pub(crate) fn all_project_groups_expanded(
        groups: &[ProjectGroup],
        collapsed: &HashSet<PathBuf>,
    ) -> bool {
        !groups.is_empty()
            && groups
                .iter()
                .all(|group| !collapsed.contains(&group.common_dir))
    }

    pub(crate) fn toggle_all_project_groups(
        groups: &[ProjectGroup],
        collapsed: &mut HashSet<PathBuf>,
    ) {
        if Self::all_project_groups_expanded(groups, collapsed) {
            collapsed.extend(groups.iter().map(|group| group.common_dir.clone()));
        } else {
            collapsed.clear();
        }
    }

    pub(crate) fn filtered_project_rows(
        groups: &[ProjectGroup],
        query: &str,
        collapsed: &HashSet<PathBuf>,
    ) -> Vec<ProjectRow> {
        let matching = Self::matching_project_rows(groups, query);
        let mut rows = Vec::new();
        for (group_index, group) in groups.iter().enumerate() {
            let trees = matching
                .iter()
                .filter_map(|(matching_group, tree_index)| {
                    (*matching_group == group_index).then_some(*tree_index)
                })
                .collect::<Vec<_>>();
            if trees.is_empty() {
                continue;
            }
            rows.push(ProjectRow::Group(group_index));
            if query.is_empty() && collapsed.contains(&group.common_dir) {
                continue;
            }
            rows.extend(trees.into_iter().map(|tree_index| ProjectRow::Worktree {
                group_index,
                tree_index,
            }));
        }
        rows
    }

    pub(crate) fn matching_project_rows(
        groups: &[ProjectGroup],
        query: &str,
    ) -> Vec<(usize, usize)> {
        let query = query.to_lowercase();
        let mut rows = Vec::new();
        for (group_index, group) in groups.iter().enumerate() {
            let group_matches = query.is_empty() || group.name.to_lowercase().contains(&query);
            for (tree_index, tree) in group.worktrees.iter().enumerate() {
                if tree.prunable.is_some() {
                    continue;
                }
                let tree_matches = tree.path.to_string_lossy().to_lowercase().contains(&query)
                    || tree.branch_label().to_lowercase().contains(&query);
                if group_matches || tree_matches {
                    rows.push((group_index, tree_index));
                }
            }
        }
        rows
    }
}
