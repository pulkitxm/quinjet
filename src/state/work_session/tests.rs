use tempfile::TempDir;

use super::*;
use crate::state::STATE_ROOT_OVERRIDE;

struct StateRoot {
    _directory: TempDir,
    previous: Option<PathBuf>,
}

impl StateRoot {
    fn new() -> Self {
        let directory = TempDir::new().expect("a scratch state root");
        let previous =
            STATE_ROOT_OVERRIDE.with(|cell| cell.replace(Some(directory.path().to_path_buf())));
        Self {
            _directory: directory,
            previous,
        }
    }
}

impl Drop for StateRoot {
    fn drop(&mut self) {
        let previous = self.previous.take();
        drop(STATE_ROOT_OVERRIDE.with(|cell| cell.replace(previous)));
    }
}

fn session(id: &str, number: u64) -> WorkSession {
    WorkSession {
        schema_version: WorkSession::SCHEMA_VERSION,
        id: id.to_owned(),
        repository: "acme/project".to_owned(),
        number,
        branch: format!("quinjet/work/{id}"),
        start_oid: "a".repeat(40),
        ..WorkSession::default()
    }
}

#[test]
fn an_unknown_identifier_reads_as_nothing_rather_than_an_empty_session() {
    let _root = StateRoot::new();

    assert_eq!(load_work_session("w42-1"), None);
    assert_eq!(load_work_sessions(), Vec::new());
}

#[test]
fn a_session_round_trips_through_the_state_file() {
    let _root = StateRoot::new();

    record_work_session(session("w42-1", 42));

    let stored = load_work_session("w42-1").expect("the session was stored");
    assert_eq!(stored.number, 42);
    assert_eq!(stored.branch, "quinjet/work/w42-1");
    assert_eq!(stored.start_oid, "a".repeat(40));
}

#[test]
fn recording_a_session_again_replaces_it_and_moves_it_to_the_front() {
    let _root = StateRoot::new();
    record_work_session(session("w42-1", 42));
    record_work_session(session("w43-1", 43));

    let mut updated = session("w42-1", 42);
    updated.state = Some(crate::git::work::WorkSessionState::Published);
    record_work_session(updated);

    let stored = load_work_sessions();
    assert_eq!(stored.len(), 2);
    assert_eq!(
        stored.first().map(|session| session.id.clone()),
        Some("w42-1".to_owned())
    );
    assert_eq!(
        load_work_session("w42-1").map(|session| session.state()),
        Some(crate::git::work::WorkSessionState::Published)
    );
}

#[test]
fn the_next_identifier_skips_every_name_already_stored() {
    let _root = StateRoot::new();

    assert_eq!(next_work_session_id(42), "w42-1");
    record_work_session(session("w42-1", 42));
    assert_eq!(next_work_session_id(42), "w42-2");
    record_work_session(session("w42-2", 42));
    assert_eq!(next_work_session_id(42), "w42-3");
    assert_eq!(next_work_session_id(43), "w43-1");
}

#[test]
fn forgetting_a_session_leaves_its_neighbors_alone() {
    let _root = StateRoot::new();
    record_work_session(session("w42-1", 42));
    record_work_session(session("w43-1", 43));

    forget_work_session("w42-1");

    assert_eq!(load_work_session("w42-1"), None);
    assert!(load_work_session("w43-1").is_some());
    forget_work_session("nothing");
    assert_eq!(load_work_sessions().len(), 1);
}

#[test]
fn the_cap_drops_the_session_nobody_has_touched_in_longest() {
    let _root = StateRoot::new();

    for index in 0..MAX_WORK_SESSIONS + 4 {
        record_work_session(session(
            &format!("w{index}-1"),
            u64::try_from(index).unwrap_or_default(),
        ));
    }

    let stored = load_work_sessions();
    assert_eq!(stored.len(), MAX_WORK_SESSIONS);
    assert_eq!(load_work_session("w0-1"), None);
    assert!(load_work_session(&format!("w{}-1", MAX_WORK_SESSIONS + 3)).is_some());
}

#[test]
fn an_unreadable_state_document_is_treated_as_no_sessions() {
    let root = StateRoot::new();
    let path = super::super::state_root()
        .expect("the override supplies a root")
        .join(WORK_SESSIONS_FILE);
    fs::create_dir_all(path.parent().expect("the file has a parent"))
        .expect("the state directory is writable");
    fs::write(&path, b"{ not json").expect("the fixture is written");

    assert_eq!(load_work_sessions(), Vec::new());
    assert_eq!(next_work_session_id(42), "w42-1");
    drop(root);
}
