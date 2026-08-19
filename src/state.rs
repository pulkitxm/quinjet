use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git::{ProjectGroup, Repository, Worktree};

const MAX_RECENT_PROJECTS: usize = 20;
const RECENT_PROJECTS_FILE: &str = "recent-projects.json";

#[cfg(test)]
thread_local! {
    static STATE_ROOT_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecentEntry {
    path: PathBuf,
    common_dir: PathBuf,
}

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
    let mut groups = Vec::new();
    let mut seen = Vec::new();
    for entry in recent_entries_with_current(session_root) {
        if seen.iter().any(|common| common == &entry.common_dir) {
            continue;
        }
        let Some(repository) = open_repository(&entry) else {
            continue;
        };
        let Ok(worktrees) = repository.worktrees_relative_to(session_root) else {
            continue;
        };
        seen.push(entry.common_dir.clone());
        groups.push(ProjectGroup {
            name: project_name(&worktrees, &repository),
            common_dir: entry.common_dir,
            worktrees,
        });
    }
    groups
}

fn recent_entries_with_current(session_root: &Path) -> Vec<RecentEntry> {
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

fn read_entries() -> Vec<RecentEntry> {
    let Some(path) = recent_projects_path() else {
        return Vec::new();
    };
    let Ok(data) = fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

fn write_entries(entries: &[RecentEntry]) {
    let Some(path) = recent_projects_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        drop(fs::create_dir_all(parent));
    }
    let Ok(data) = serde_json::to_vec_pretty(entries) else {
        return;
    };
    let staging = path.with_extension("json.tmp");
    if fs::write(&staging, data).is_ok() {
        drop(fs::rename(staging, path));
    }
}

fn recent_projects_path() -> Option<PathBuf> {
    Some(state_root()?.join(RECENT_PROJECTS_FILE))
}

fn state_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        let override_root = STATE_ROOT_OVERRIDE.with(|cell| cell.borrow().clone());
        if override_root.is_some() {
            return override_root;
        }
    }
    if let Some(path) = env::var_os("QUINJET_STATE_DIR").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        if let Some(path) = env::var_os("LOCALAPPDATA").filter(|path| !path.is_empty()) {
            return Some(PathBuf::from(path).join("Quinjet").join("state"));
        }
    }
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|path| !path.is_empty())?;
    let home = PathBuf::from(home);
    Some(
        env::var_os("XDG_STATE_HOME")
            .filter(|path| !path.is_empty())
            .map_or_else(|| home.join(".local/state"), PathBuf::from)
            .join("quinjet"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn records_one_project_per_common_directory() {
        let state = unique_dir("state");
        drop(STATE_ROOT_OVERRIDE.with(|cell| cell.replace(Some(state.clone()))));
        let repo = git_repo("recent-a");
        let linked = unique_dir("recent-linked");
        drop(fs::remove_dir_all(&linked));
        let linked_display = linked.display().to_string();
        run_git(&repo, &["worktree", "add", "-b", "topic", &linked_display]);
        let linked = fs::canonicalize(&linked).unwrap();
        record_recent_project(&repo);
        record_recent_project(&linked);
        let entries = read_entries();
        assert_eq!(entries.len(), 1);
        let groups = load_recent_projects(&linked);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].worktrees.len(), 2);
        assert!(
            groups[0]
                .worktrees
                .iter()
                .any(|tree| tree.current && tree.path == linked)
        );
        forget_recent_project(&entries[0].common_dir);
        assert!(read_entries().is_empty());
        drop(STATE_ROOT_OVERRIDE.with(|cell| cell.replace(None)));
        drop(fs::remove_dir_all(&state));
        drop(fs::remove_dir_all(&repo));
        drop(fs::remove_dir_all(&linked));
    }

    fn unique_dir(label: &str) -> PathBuf {
        let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            env::temp_dir().join(format!("quinjet-state-{label}-{}-{id}", std::process::id()));
        drop(fs::remove_dir_all(&path));
        drop(fs::create_dir_all(&path));
        path
    }

    fn git_repo(label: &str) -> PathBuf {
        let path = unique_dir(label);
        run_git(&path, &["init", "--initial-branch=main"]);
        fs::write(path.join("README.md"), "test\n").unwrap();
        run_git(&path, &["add", "README.md"]);
        run_git(
            &path,
            &[
                "-c",
                "user.name=Quinjet Test",
                "-c",
                "user.email=quinjet@example.com",
                "commit",
                "--message=initial",
            ],
        );
        path
    }

    fn run_git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
