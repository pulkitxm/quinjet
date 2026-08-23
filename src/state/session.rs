use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const PROJECT_SESSION_FILE: &str = "project-session.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSession {
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) active: Option<PathBuf>,
}

pub(crate) fn load_project_session() -> Option<ProjectSession> {
    let path = super::state_root()?.join(PROJECT_SESSION_FILE);
    let data = fs::read(path).ok()?;
    let session = serde_json::from_slice::<ProjectSession>(&data).ok()?;
    normalize(session)
}

pub(crate) fn record_project_session(session: ProjectSession) {
    let Some(session) = normalize(session) else {
        return;
    };
    let Some(path) = super::state_root().map(|root| root.join(PROJECT_SESSION_FILE)) else {
        return;
    };
    if let Some(parent) = path.parent() {
        drop(fs::create_dir_all(parent));
    }
    let Ok(data) = serde_json::to_vec_pretty(&session) else {
        return;
    };
    let staging = path.with_extension("json.tmp");
    if fs::write(&staging, data).is_ok() {
        drop(fs::rename(staging, path));
    }
}

fn normalize(mut session: ProjectSession) -> Option<ProjectSession> {
    let mut seen = HashSet::new();
    session.roots.retain(|root| seen.insert(root.clone()));
    session.roots.truncate(super::MAX_RECENT_PROJECTS);
    if session
        .active
        .as_ref()
        .is_some_and(|active| !session.roots.iter().any(|root| root == active))
    {
        session.active = None;
    }
    (!session.roots.is_empty()).then_some(session)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

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
    fn project_session_round_trips() {
        let state = TempDir::new().expect("state directory");
        let _guard = StateRootGuard::new(state.path());
        let session = ProjectSession {
            roots: vec![PathBuf::from("/one"), PathBuf::from("/two")],
            active: Some(PathBuf::from("/two")),
        };

        record_project_session(session.clone());

        assert_eq!(load_project_session(), Some(session));
    }

    #[test]
    fn project_session_deduplicates_roots_and_rejects_unknown_active_root() {
        let state = TempDir::new().expect("state directory");
        let _guard = StateRootGuard::new(state.path());
        record_project_session(ProjectSession {
            roots: vec![PathBuf::from("/one"), PathBuf::from("/one")],
            active: Some(PathBuf::from("/missing")),
        });

        assert_eq!(
            load_project_session(),
            Some(ProjectSession {
                roots: vec![PathBuf::from("/one")],
                active: None,
            })
        );
    }

    #[test]
    fn empty_and_invalid_sessions_are_ignored() {
        let state = TempDir::new().expect("state directory");
        let _guard = StateRootGuard::new(state.path());
        record_project_session(ProjectSession {
            roots: Vec::new(),
            active: None,
        });
        assert_eq!(load_project_session(), None);

        fs::write(state.path().join(PROJECT_SESSION_FILE), b"{").expect("invalid state");
        assert_eq!(load_project_session(), None);
    }
}
