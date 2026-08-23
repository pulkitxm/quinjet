use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{MAX_RECENT_PROJECTS, state_root};
use crate::ssh::{MAX_SSH_MACHINES, SshMachine};

const RECENT_REMOTES_FILE: &str = "recent-remotes.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecentRemote {
    pub target: String,
    pub folder: String,
    #[serde(default = "initial_uses")]
    pub uses: u64,
}

const fn initial_uses() -> u64 {
    1
}

pub(crate) fn record_recent_remote(target: &str, folder: &Path) {
    let folder = folder.to_string_lossy().into_owned();
    let mut entries = load_recent_remotes();
    let uses = entries
        .iter()
        .find(|entry| entry.target == target && entry.folder == folder)
        .map_or(1, |entry| entry.uses.saturating_add(1));
    entries.retain(|entry| entry.target != target || entry.folder != folder);
    entries.insert(
        0,
        RecentRemote {
            target: target.to_owned(),
            folder,
            uses,
        },
    );
    entries.truncate(MAX_RECENT_PROJECTS);
    write_entries(&entries);
}

pub(crate) fn load_recent_ssh_machines() -> Vec<SshMachine> {
    grouped_ssh_machines()
}

pub(crate) fn load_recent_ssh_machines_with_current(
    current: &str,
    folder: &Path,
) -> Vec<SshMachine> {
    let mut machines = grouped_ssh_machines();
    if !machines.iter().any(|machine| machine.target == current) {
        machines.push(SshMachine {
            target: current.to_owned(),
            folder: folder.to_path_buf(),
            accessible: true,
            uses: 0,
            local: false,
        });
        machines.sort_by_key(|machine| std::cmp::Reverse(machine.uses));
    }
    if let Some(current_index) = machines
        .iter()
        .position(|machine| machine.target == current)
        .filter(|index| *index >= MAX_SSH_MACHINES)
    {
        let current_machine = machines.remove(current_index);
        machines.insert(MAX_SSH_MACHINES.saturating_sub(1), current_machine);
    }
    machines.truncate(MAX_SSH_MACHINES);
    machines
}

fn grouped_ssh_machines() -> Vec<SshMachine> {
    let mut machines = Vec::<SshMachine>::new();
    for entry in load_recent_remotes() {
        if let Some(machine) = machines
            .iter_mut()
            .find(|machine| machine.target == entry.target)
        {
            machine.uses = machine.uses.saturating_add(entry.uses);
            let folder = Path::new(&entry.folder);
            if !is_remote_absolute(&machine.folder) && is_remote_absolute(folder) {
                machine.folder = folder.to_path_buf();
            }
        } else {
            machines.push(SshMachine {
                target: entry.target,
                folder: Path::new(&entry.folder).to_path_buf(),
                accessible: false,
                uses: entry.uses,
                local: false,
            });
        }
    }
    machines.sort_by_key(|machine| std::cmp::Reverse(machine.uses));
    machines.truncate(MAX_SSH_MACHINES);
    machines
}

fn is_remote_absolute(path: &Path) -> bool {
    path.is_absolute() || path.as_os_str().to_string_lossy().starts_with('/')
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
                    uses: 2,
                },
                RecentRemote {
                    target: "second".to_owned(),
                    folder: "/two".to_owned(),
                    uses: 1,
                },
            ]
        );
        forget_recent_remote("first", Some("/other"));
        assert_eq!(load_recent_remotes().len(), 2);
        forget_recent_remote("first", Some("/one"));
        assert_eq!(load_recent_remotes().len(), 1);
        forget_recent_remote("second", None);
        assert_eq!(load_recent_remotes(), Vec::new());
    }

    #[test]
    fn machines_are_grouped_and_sorted_by_total_usage() {
        let state = tempfile::tempdir().unwrap();
        let _guard = StateRootGuard::new(state.path());
        record_recent_remote("occasional", Path::new("/one"));
        record_recent_remote("frequent", Path::new("/first"));
        record_recent_remote("frequent", Path::new("/second"));
        record_recent_remote("frequent", Path::new("/first"));
        let machines = load_recent_ssh_machines_with_current("new", Path::new("/repo"));
        assert_eq!(machines[0].target, "frequent");
        assert_eq!(machines[0].uses, 3);
        assert_eq!(machines[0].folder, Path::new("/first"));
        assert_eq!(machines[1].target, "occasional");
        assert_eq!(machines[1].uses, 1);
        assert_eq!(machines[2].target, "new");
        assert_eq!(machines[2].uses, 0);
    }

    #[test]
    fn machine_folder_prefers_an_absolute_recent_project() {
        let state = tempfile::tempdir().unwrap();
        let _guard = StateRootGuard::new(state.path());
        let project = state.path().join("work").join("project");
        record_recent_remote("remote", &project);
        record_recent_remote("remote", Path::new("."));

        let machines = load_recent_ssh_machines();

        assert_eq!(machines[0].folder, project);
        assert_eq!(machines[0].uses, 2);
    }

    #[test]
    fn current_machine_remains_visible_at_the_limit() {
        let state = tempfile::tempdir().unwrap();
        let _guard = StateRootGuard::new(state.path());
        for index in 0..MAX_SSH_MACHINES {
            record_recent_remote(&format!("host-{index}"), Path::new("/repo"));
        }
        let machines = load_recent_ssh_machines_with_current("new-current", Path::new("/current"));
        assert_eq!(machines.len(), MAX_SSH_MACHINES);
        assert!(
            machines
                .iter()
                .any(|machine| machine.target == "new-current")
        );
        assert_eq!(
            machines.last().map(|machine| machine.target.as_str()),
            Some("new-current")
        );
    }

    #[test]
    fn local_machine_list_contains_only_recorded_ssh_targets() {
        let state = tempfile::tempdir().unwrap();
        let _guard = StateRootGuard::new(state.path());
        record_recent_remote("remote-host", Path::new("/repo"));
        assert_eq!(
            load_recent_ssh_machines()
                .iter()
                .map(|machine| machine.target.as_str())
                .collect::<Vec<_>>(),
            vec!["remote-host"]
        );
    }
}
