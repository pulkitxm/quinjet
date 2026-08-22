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
