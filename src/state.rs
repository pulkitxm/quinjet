#[cfg(test)]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{env, fs};

use serde::{Deserialize, Serialize};

use crate::git::{ProjectGroup, Repository, Worktree};
use crate::state_sorting::{sort_project_groups, sort_worktrees};

mod project_picker;
mod remote;
pub(crate) mod session;

#[cfg(test)]
use project_picker::PROJECT_PICKER_FILE;
pub(crate) use project_picker::{load_collapsed_project_groups, record_collapsed_project_groups};
pub(crate) use remote::{
    forget_recent_remote, load_recent_remotes, load_recent_ssh_machines,
    load_recent_ssh_machines_with_current, record_recent_remote,
};

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
        let Ok(mut worktrees) = repository.worktrees_relative_to(session_root) else {
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
    use std::process::Command;

    use super::*;
    use crate::git::support::same_path;

    struct StateRootGuard {
        previous: Option<PathBuf>,
    }

    impl StateRootGuard {
        fn new(root: &Path) -> Self {
            let previous = STATE_ROOT_OVERRIDE.with(|cell| cell.replace(Some(root.to_path_buf())));
            Self { previous }
        }
    }

    impl Drop for StateRootGuard {
        fn drop(&mut self) {
            let previous = self.previous.take();
            drop(STATE_ROOT_OVERRIDE.with(|cell| cell.replace(previous)));
        }
    }

    #[test]
    fn invalid_state_documents_are_treated_as_empty() {
        let (state, _guard) = isolated_state();
        let documents: [&[u8]; 4] = [b"", b"{", br#"{"path":"repo","commonDir":"git"}"#, b"null"];
        for document in documents {
            write_state_bytes(state.path(), document);
            assert_eq!(read_entries(), Vec::<RecentEntry>::new());
        }
    }

    #[test]
    fn partially_valid_state_is_treated_as_empty() {
        let (state, _guard) = isolated_state();
        write_state_bytes(
            state.path(),
            br#"[{"path":"valid","commonDir":"common"},{"path":3,"commonDir":"broken"}]"#,
        );
        assert_eq!(read_entries(), Vec::<RecentEntry>::new());
    }

    #[test]
    fn project_picker_folds_round_trip_and_ignore_invalid_state() {
        let (state, _guard) = isolated_state();
        let collapsed = HashSet::from([
            PathBuf::from("/work/one/.git"),
            PathBuf::from("/work/two/.git"),
        ]);

        record_collapsed_project_groups(&collapsed);

        assert_eq!(load_collapsed_project_groups(), collapsed);
        fs::write(state.path().join(PROJECT_PICKER_FILE), b"{").unwrap();
        assert!(load_collapsed_project_groups().is_empty());
    }

    #[test]
    fn current_repository_is_inserted_first_and_deduplicated() {
        let (_state, _guard) = isolated_state();
        let repo = git_repo();
        let common_dir = repository_common_dir(repo.path());
        let first = fake_entry(1);
        let last = fake_entry(2);
        let stored = vec![
            first.clone(),
            RecentEntry {
                path: PathBuf::from("old-checkout"),
                common_dir: common_dir.clone(),
            },
            last.clone(),
            RecentEntry {
                path: PathBuf::from("older-checkout"),
                common_dir: common_dir.clone(),
            },
        ];
        write_entries(&stored);

        let entries = recent_entries_with_current(repo.path());

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].common_dir, common_dir);
        assert!(same_path(&entries[0].path, repo.path()));
        assert_eq!(entries[1], first);
        assert_eq!(entries[2], last);
        assert_eq!(read_entries(), stored);
    }

    #[test]
    fn recording_preserves_order_and_caps_entries() {
        let (_state, _guard) = isolated_state();
        let repo = git_repo();
        let older = (0..MAX_RECENT_PROJECTS + 5)
            .map(fake_entry)
            .collect::<Vec<_>>();
        write_entries(&older);

        record_recent_project(repo.path());

        let entries = read_entries();
        assert_eq!(entries.len(), MAX_RECENT_PROJECTS);
        assert!(same_path(&entries[0].path, repo.path()));
        assert_eq!(
            entries.get(1..),
            older.get(..MAX_RECENT_PROJECTS.saturating_sub(1))
        );
    }

    #[test]
    fn recording_non_repository_paths_preserves_state() {
        let (_state, _guard) = isolated_state();
        let entries = vec![fake_entry(1), fake_entry(2)];
        write_entries(&entries);
        let plain = tempfile::tempdir().unwrap();

        record_recent_project(plain.path());
        record_recent_project(&plain.path().join("deleted"));

        assert_eq!(read_entries(), entries);
    }

    #[test]
    fn loading_skips_deleted_and_non_repository_entries() {
        let (_state, _guard) = isolated_state();
        let active = git_repo();
        let active_common = repository_common_dir(active.path());
        let deleted = git_repo();
        let deleted_entry = RecentEntry {
            path: deleted.path().to_path_buf(),
            common_dir: repository_common_dir(deleted.path()),
        };
        drop(deleted);
        let plain = tempfile::tempdir().unwrap();
        let session = tempfile::tempdir().unwrap();
        write_entries(&[
            RecentEntry {
                path: session.path().join("missing-checkout"),
                common_dir: active_common.clone(),
            },
            RecentEntry {
                path: active.path().to_path_buf(),
                common_dir: active_common.clone(),
            },
            deleted_entry,
            RecentEntry {
                path: plain.path().to_path_buf(),
                common_dir: plain.path().to_path_buf(),
            },
        ]);

        let groups = load_recent_projects(session.path());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].common_dir, active_common);
        assert_eq!(groups[0].worktrees.len(), 1);
        assert!(same_path(&groups[0].worktrees[0].path, active.path()));
    }

    #[test]
    fn forgetting_existing_missing_and_only_entries_is_stable() {
        let (_state, _guard) = isolated_state();
        let first = fake_entry(1);
        let only = fake_entry(2);
        write_entries(&[first.clone(), only.clone()]);

        forget_recent_project(&first.common_dir);
        assert_eq!(read_entries(), vec![only.clone()]);
        forget_recent_project(Path::new("missing-common-dir"));
        assert_eq!(read_entries(), vec![only.clone()]);
        forget_recent_project(&only.common_dir);
        assert_eq!(read_entries(), Vec::<RecentEntry>::new());
    }

    #[test]
    fn writing_creates_the_state_directory() {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("nested").join("state");
        let _guard = StateRootGuard::new(&root);
        let entries = vec![fake_entry(1)];

        write_entries(&entries);

        assert!(root.is_dir());
        assert_eq!(read_entries(), entries);
        assert!(!root.join("recent-projects.json.tmp").exists());
    }

    #[test]
    fn regular_file_state_root_rejects_writes() {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("state");
        fs::write(&root, "sentinel").unwrap();
        let _guard = StateRootGuard::new(&root);

        write_entries(&[fake_entry(1)]);

        assert_eq!(fs::read_to_string(&root).unwrap(), "sentinel");
        assert_eq!(read_entries(), Vec::<RecentEntry>::new());
    }

    #[test]
    fn unwritable_staging_path_preserves_existing_state() {
        let (state, _guard) = isolated_state();
        let original = vec![fake_entry(1)];
        write_entries(&original);
        fs::create_dir_all(state.path().join("recent-projects.json.tmp")).unwrap();

        write_entries(&[fake_entry(2)]);

        assert_eq!(read_entries(), original);
    }

    #[test]
    fn records_one_project_per_common_directory() {
        let (_state, _guard) = isolated_state();
        let repo = git_repo();
        let linked_root = tempfile::tempdir().unwrap();
        let linked = linked_root.path().join("topic");
        let linked_display = linked.display().to_string();
        run_git(
            repo.path(),
            &["worktree", "add", "-b", "topic", &linked_display],
        );
        record_recent_project(repo.path());
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
                .any(|tree| tree.current && same_path(&tree.path, &linked))
        );
        forget_recent_project(&entries[0].common_dir);
        assert_eq!(read_entries(), Vec::<RecentEntry>::new());
    }

    fn isolated_state() -> (tempfile::TempDir, StateRootGuard) {
        let state = tempfile::tempdir().unwrap();
        let guard = StateRootGuard::new(state.path());
        (state, guard)
    }

    fn fake_entry(index: usize) -> RecentEntry {
        RecentEntry {
            path: PathBuf::from(format!("project-{index}")),
            common_dir: PathBuf::from(format!("common-{index}")),
        }
    }

    fn repository_common_dir(path: &Path) -> PathBuf {
        Repository::discover(path)
            .unwrap()
            .git_common_dir()
            .unwrap()
    }

    fn write_state_bytes(state: &Path, data: &[u8]) {
        fs::write(state.join(RECENT_PROJECTS_FILE), data).unwrap();
    }

    fn git_repo() -> tempfile::TempDir {
        let path = tempfile::tempdir().unwrap();
        run_git(path.path(), &["init", "--initial-branch=main"]);
        fs::write(path.path().join("README.md"), "test\n").unwrap();
        run_git(path.path(), &["add", "README.md"]);
        run_git(
            path.path(),
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
