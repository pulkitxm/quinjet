use std::fs;
use std::path::PathBuf;

use crate::git::work::WorkSession;

const WORK_SESSIONS_FILE: &str = "work-sessions.json";
#[doc = " Sessions are working notes, not an archive. Sixteen is more than"]
#[doc = " anybody has in flight, and the oldest is dropped first."]
const MAX_WORK_SESSIONS: usize = 16;

fn matches(session: &WorkSession, id: &str) -> bool {
    session.id.eq_ignore_ascii_case(id)
}

pub(crate) fn load_work_sessions() -> Vec<WorkSession> {
    read_records()
}

pub(crate) fn load_work_session(id: &str) -> Option<WorkSession> {
    read_records()
        .into_iter()
        .find(|session| matches(session, id))
}

#[doc = " Store one session at the front, so the cap drops the one nobody has"]
#[doc = " touched in longest rather than the one being worked on now."]
pub(crate) fn record_work_session(session: WorkSession) {
    let mut sessions = read_records();
    sessions.retain(|stored| !matches(stored, &session.id));
    sessions.insert(0, session);
    sessions.truncate(MAX_WORK_SESSIONS);
    write_records(&sessions);
}

pub(crate) fn forget_work_session(id: &str) {
    let mut sessions = read_records();
    let before = sessions.len();
    sessions.retain(|stored| !matches(stored, id));
    if sessions.len() != before {
        write_records(&sessions);
    }
}

#[doc = " The next free identifier for a pull request, which is the number and"]
#[doc = " the lowest suffix nothing already recorded is using. Reusing an"]
#[doc = " abandoned session's name would make two records collide in the branch"]
#[doc = " namespace, so the search skips whatever is stored under any state."]
pub(crate) fn next_work_session_id(number: u64) -> String {
    let sessions = read_records();
    for suffix in 1..=u32::try_from(MAX_WORK_SESSIONS).unwrap_or(u32::MAX) + 1 {
        let candidate = format!("w{number}-{suffix}");
        if !sessions.iter().any(|session| matches(session, &candidate)) {
            return candidate;
        }
    }
    format!("w{number}-{}", MAX_WORK_SESSIONS + 1)
}

fn work_sessions_path() -> Option<PathBuf> {
    Some(super::state_root()?.join(WORK_SESSIONS_FILE))
}

fn read_records() -> Vec<WorkSession> {
    let Some(path) = work_sessions_path() else {
        return Vec::new();
    };
    let Ok(data) = fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

fn write_records(sessions: &[WorkSession]) {
    let Some(path) = work_sessions_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        drop(fs::create_dir_all(parent));
    }
    let Ok(data) = serde_json::to_vec_pretty(sessions) else {
        return;
    };
    let staging = path.with_extension("json.tmp");
    if fs::write(&staging, data).is_ok() {
        drop(fs::rename(staging, path));
    }
}

#[cfg(test)]
mod tests;
