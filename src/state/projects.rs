use std::path::Path;

use crate::git::{ProjectGroup, Repository, Worktree};
use crate::state_sorting::{sort_project_groups, sort_worktrees};

use super::{MAX_RECENT_PROJECTS, RecentEntry, read_entries, write_entries};

pub(crate) fn record_recent_project(root: &Path) {
    let Ok(repository) = Repository::discover(root) else {
        return;
    };
    let Ok(common_dir) = repository.git_common_dir() else {
        return;
    };
    let mut entries = read_entries();
    entries.retain(|entry| entry.common_dir != common_dir);
    entries.insert(
        0,
        RecentEntry {
            path: repository.root().to_path_buf(),
            common_dir,
        },
    );
    entries.truncate(MAX_RECENT_PROJECTS);
    write_entries(&entries);
}

pub(crate) fn forget_recent_project(common_dir: &Path) {
    let mut entries = read_entries();
    entries.retain(|entry| entry.common_dir != common_dir);
    write_entries(&entries);
}

pub(crate) fn load_recent_projects(session_root: &Path) -> Vec<ProjectGroup> {
    load_project_groups(
        recent_entries_with_current(session_root),
        Some(session_root),
    )
}

pub(crate) fn load_stored_projects() -> Vec<ProjectGroup> {
    load_project_groups(read_entries(), None)
}

fn load_project_groups(
    entries: Vec<RecentEntry>,
    session_root: Option<&Path>,
) -> Vec<ProjectGroup> {
    let mut groups = Vec::new();
    let mut seen = Vec::new();
    for entry in entries {
        if seen.iter().any(|common| common == &entry.common_dir) {
            continue;
        }
        let Some(repository) = open_repository(&entry) else {
            continue;
        };
        let Ok(mut worktrees) =
            repository.worktrees_relative_to(session_root.unwrap_or(&entry.path))
        else {
            continue;
        };
        sort_worktrees(&mut worktrees);
        seen.push(entry.common_dir.clone());
        groups.push(ProjectGroup {
            name: project_name(&worktrees, &repository),
            common_dir: entry.common_dir,
            worktrees,
        });
    }
    sort_project_groups(&mut groups);
    groups
}

pub(super) fn recent_entries_with_current(session_root: &Path) -> Vec<RecentEntry> {
    let mut entries = read_entries();
    if let Ok(repository) = Repository::discover(session_root)
        && let Ok(common_dir) = repository.git_common_dir()
    {
        entries.retain(|entry| entry.common_dir != common_dir);
        entries.insert(
            0,
            RecentEntry {
                path: repository.root().to_path_buf(),
                common_dir,
            },
        );
    }
    entries
}

fn open_repository(entry: &RecentEntry) -> Option<Repository> {
    Repository::discover(&entry.path)
        .ok()
        .or_else(|| Repository::discover(&entry.common_dir).ok())
}

fn project_name(worktrees: &[Worktree], repository: &Repository) -> String {
    worktrees
        .first()
        .and_then(|tree| tree.path.file_name())
        .map_or_else(
            || repository.name(),
            |name| name.to_string_lossy().into_owned(),
        )
}
