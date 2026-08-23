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
