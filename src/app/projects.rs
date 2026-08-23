#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
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
    ) -> Vec<(usize, usize)> {
        let mut rows = Self::matching_project_rows(groups, query);
        if query.is_empty() {
            rows.retain(|(group_index, _)| {
                groups
                    .get(*group_index)
                    .is_some_and(|group| !collapsed.contains(&group.common_dir))
            });
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
