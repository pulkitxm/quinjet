use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(super) const PROJECT_PICKER_FILE: &str = "project-picker.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPickerState {
    collapsed: Vec<PathBuf>,
}

pub(crate) fn load_collapsed_project_groups() -> HashSet<PathBuf> {
    let Some(path) = super::state_root().map(|root| root.join(PROJECT_PICKER_FILE)) else {
        return HashSet::new();
    };
    let Ok(data) = fs::read(path) else {
        return HashSet::new();
    };
    serde_json::from_slice::<ProjectPickerState>(&data)
        .map(|state| state.collapsed.into_iter().collect())
        .unwrap_or_default()
}

pub(crate) fn record_collapsed_project_groups(collapsed: &HashSet<PathBuf>) {
    let Some(path) = super::state_root().map(|root| root.join(PROJECT_PICKER_FILE)) else {
        return;
    };
    if let Some(parent) = path.parent() {
        drop(fs::create_dir_all(parent));
    }
    let mut collapsed = collapsed.iter().cloned().collect::<Vec<_>>();
    collapsed.sort();
    let Ok(data) = serde_json::to_vec_pretty(&ProjectPickerState { collapsed }) else {
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
        previous: Option<PathBuf>,
    }

    impl StateRootGuard {
        fn new(root: &std::path::Path) -> Self {
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
    fn folds_round_trip_and_ignore_invalid_state() {
        let state = tempfile::tempdir().unwrap();
        let _guard = StateRootGuard::new(state.path());
        let collapsed = HashSet::from([
            PathBuf::from("/work/one/.git"),
            PathBuf::from("/work/two/.git"),
        ]);

        record_collapsed_project_groups(&collapsed);

        assert_eq!(load_collapsed_project_groups(), collapsed);
        fs::write(state.path().join(PROJECT_PICKER_FILE), b"{").unwrap();
        assert!(load_collapsed_project_groups().is_empty());
    }
}
