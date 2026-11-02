use std::fs;

use serde::{Deserialize, Serialize};

pub(super) const PREFERENCES_FILE: &str = "preferences.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Preferences {
    wrap_code: bool,
}

pub(crate) fn load_code_wrap() -> bool {
    super::state_root()
        .and_then(|root| fs::read(root.join(PREFERENCES_FILE)).ok())
        .and_then(|data| serde_json::from_slice::<Preferences>(&data).ok())
        .is_some_and(|preferences| preferences.wrap_code)
}

pub(crate) fn record_code_wrap(wrap_code: bool) {
    let Some(path) = super::state_root().map(|root| root.join(PREFERENCES_FILE)) else {
        return;
    };
    if let Some(parent) = path.parent() {
        drop(fs::create_dir_all(parent));
    }
    let Ok(data) = serde_json::to_vec_pretty(&Preferences { wrap_code }) else {
        return;
    };
    let staging = path.with_extension("json.tmp");
    if fs::write(&staging, data).is_ok() {
        drop(fs::rename(staging, path));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
    fn code_wrap_round_trips_and_ignores_invalid_state() {
        let state = tempfile::tempdir().unwrap();
        let _guard = StateRootGuard::new(state.path());
        assert!(!load_code_wrap());

        record_code_wrap(true);
        assert!(load_code_wrap());
        record_code_wrap(false);
        assert!(!load_code_wrap());

        fs::write(state.path().join(PREFERENCES_FILE), b"{").unwrap();
        assert!(!load_code_wrap());
    }
}
