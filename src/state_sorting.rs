use crate::git::{ProjectGroup, Worktree};

pub(crate) fn sort_project_groups(groups: &mut [ProjectGroup]) {
    groups.sort_by(|left, right| {
        right
            .updated_unix()
            .cmp(&left.updated_unix())
            .then_with(|| left.name.cmp(&right.name))
    });
}

pub(crate) fn sort_worktrees(worktrees: &mut [Worktree]) {
    worktrees.sort_by(|left, right| {
        right
            .updated_unix
            .cmp(&left.updated_unix)
            .then_with(|| left.path.cmp(&right.path))
    });
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn projects_and_worktrees_sort_by_their_latest_commit() {
        let mut older = worktree("/repos/older", 10);
        let newer = worktree("/repos/newer", 30);
        let middle = worktree("/repos/middle", 20);
        older.current = true;
        let mut worktrees = vec![older.clone(), newer.clone(), middle.clone()];
        sort_worktrees(&mut worktrees);
        assert_eq!(
            worktrees
                .iter()
                .map(|worktree| worktree.path.as_path())
                .collect::<Vec<_>>(),
            [
                Path::new("/repos/newer"),
                Path::new("/repos/middle"),
                Path::new("/repos/older"),
            ]
        );

        let mut groups = vec![
            ProjectGroup {
                name: "older".to_owned(),
                common_dir: PathBuf::from("/repos/older/.git"),
                worktrees: vec![older],
            },
            ProjectGroup {
                name: "newer".to_owned(),
                common_dir: PathBuf::from("/repos/newer/.git"),
                worktrees: vec![newer, middle],
            },
        ];
        sort_project_groups(&mut groups);
        assert_eq!(
            groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            ["newer", "older"]
        );
    }

    fn worktree(path: &str, updated_unix: i64) -> Worktree {
        Worktree {
            path: PathBuf::from(path),
            head: format!("{updated_unix:040x}"),
            updated_at: Some(format!("2026-08-22T18:00:{updated_unix:02}Z")),
            updated_unix: Some(updated_unix),
            branch: Some("main".to_owned()),
            current: false,
            bare: false,
            detached: false,
            locked: None,
            prunable: None,
        }
    }
}
