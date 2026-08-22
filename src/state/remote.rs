use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{MAX_RECENT_PROJECTS, state_root};

const RECENT_REMOTES_FILE: &str = "recent-remotes.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecentRemote {
    pub target: String,
    pub folder: String,
}

pub(crate) fn record_recent_remote(target: &str, folder: &Path) {
    let folder = folder.to_string_lossy().into_owned();
    let mut entries = load_recent_remotes();
    entries.retain(|entry| entry.target != target || entry.folder != folder);
    entries.insert(
        0,
        RecentRemote {
            target: target.to_owned(),
            folder,
        },
    );
    entries.truncate(MAX_RECENT_PROJECTS);
    write_entries(&entries);
}

pub(crate) fn forget_recent_remote(target: &str, folder: Option<&str>) {
    let mut entries = load_recent_remotes();
    entries.retain(|entry| {
        entry.target != target || folder.is_some_and(|folder| entry.folder != folder)
    });
    write_entries(&entries);
}

pub(crate) fn load_recent_remotes() -> Vec<RecentRemote> {
    let Some(path) = state_root().map(|root| root.join(RECENT_REMOTES_FILE)) else {
        return Vec::new();
    };
    let Ok(data) = fs::read(path) else {
        return Vec::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

fn write_entries(entries: &[RecentRemote]) {
    let Some(path) = state_root().map(|root| root.join(RECENT_REMOTES_FILE)) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    struct StateRootGuard {
        previous: Option<std::path::PathBuf>,
    }

    impl StateRootGuard {
        fn new(root: &Path) -> Self {
            let previous = super::super::STATE_ROOT_OVERRIDE
                .with(|cell| cell.replace(Some(root.to_path_buf())));
            Self { previous }
        }
    }

    impl Drop for StateRootGuard {
        fn drop(&mut self) {
            let previous = self.previous.take();
            drop(super::super::STATE_ROOT_OVERRIDE.with(|cell| cell.replace(previous)));
        }
    }

    #[test]
    fn remote_recents_deduplicate_reorder_and_forget() {
        let state = tempfile::tempdir().unwrap();
        let _guard = StateRootGuard::new(state.path());
        record_recent_remote("first", Path::new("/one"));
        record_recent_remote("second", Path::new("/two"));
        record_recent_remote("first", Path::new("/one"));
        assert_eq!(
            load_recent_remotes(),
            vec![
                RecentRemote {
                    target: "first".to_owned(),
                    folder: "/one".to_owned(),
                },
                RecentRemote {
                    target: "second".to_owned(),
                    folder: "/two".to_owned(),
                },
            ]
        );
        forget_recent_remote("first", Some("/other"));
        assert_eq!(load_recent_remotes().len(), 2);
        forget_recent_remote("first", Some("/one"));
        assert_eq!(load_recent_remotes().len(), 1);
        forget_recent_remote("second", None);
        assert!(load_recent_remotes().is_empty());
    }
}
