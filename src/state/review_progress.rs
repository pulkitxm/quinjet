use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const REVIEW_PROGRESS_FILE: &str = "review-progress.json";
#[doc = " Progress is a working note, not an archive. Sixty-four pull requests is"]
#[doc = " more than anyone has open at once, and the oldest is dropped first."]
const MAX_TRACKED_PULL_REQUESTS: usize = 64;
#[doc = " A pull request with more viewed files than this is past the point where"]
#[doc = " per-file tracking helps, and the cap keeps one record small."]
const MAX_VIEWED_FILES: usize = 4_096;

#[doc = " One file the reviewer marked as read, and the head commit it was read"]
#[doc = " at. Keeping the commit is what lets a later read say `changed` instead of"]
#[doc = " quietly keeping the mark or quietly dropping it."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewedFile {
    pub path: PathBuf,
    pub head_oid: String,
}

#[doc = " What Quinjet remembers locally about reviewing one pull request. It is"]
#[doc = " deliberately not shared with GitHub: file-viewed state there belongs to"]
#[doc = " the web session, and mirroring it would need write access this never"]
#[doc = " asks for."]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewProgressRecord {
    pub repository: String,
    pub number: u64,
    #[doc = " The head commit the reviewer last looked at this pull request under."]
    pub visited_oid: String,
    pub visited_at: String,
    pub viewed: Vec<ViewedFile>,
}

impl ReviewProgressRecord {
    pub(crate) fn new(repository: &str, number: u64) -> Self {
        Self {
            repository: repository.to_owned(),
            number,
            ..Self::default()
        }
    }

    pub(crate) fn viewed_at(&self, path: &Path) -> Option<&str> {
        self.viewed
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.head_oid.as_str())
    }

    pub(crate) fn mark_viewed(&mut self, path: &Path, head_oid: &str) {
        self.viewed.retain(|file| file.path != path);
        self.viewed.push(ViewedFile {
            path: path.to_path_buf(),
            head_oid: head_oid.to_owned(),
        });
        if self.viewed.len() > MAX_VIEWED_FILES {
            drop(self.viewed.remove(0));
        }
    }

    pub(crate) fn mark_unviewed(&mut self, path: &Path) -> bool {
        let before = self.viewed.len();
        self.viewed.retain(|file| file.path != path);
        self.viewed.len() != before
    }

    pub(crate) fn record_visit(&mut self, head_oid: &str, at: String) {
        head_oid.clone_into(&mut self.visited_oid);
        self.visited_at = at;
    }
}

fn matches(record: &ReviewProgressRecord, repository: &str, number: u64) -> bool {
    record.number == number
        && record
            .repository
            .trim_end_matches('/')
            .eq_ignore_ascii_case(repository.trim_end_matches('/'))
}

pub(crate) fn load_review_progress(repository: &str, number: u64) -> ReviewProgressRecord {
    read_records()
        .into_iter()
        .find(|record| matches(record, repository, number))
        .unwrap_or_else(|| ReviewProgressRecord::new(repository, number))
}

#[doc = " Store one record at the front, so the cap drops the pull request nobody"]
#[doc = " has touched in longest rather than the one being reviewed now."]
pub(crate) fn record_review_progress(record: ReviewProgressRecord) {
    let mut records = read_records();
    records.retain(|stored| !matches(stored, &record.repository, record.number));
    records.insert(0, record);
    records.truncate(MAX_TRACKED_PULL_REQUESTS);
    write_records(&records);
}

pub(crate) fn forget_review_progress(repository: &str, number: u64) {
    let mut records = read_records();
    let before = records.len();
    records.retain(|stored| !matches(stored, repository, number));
    if records.len() != before {
        write_records(&records);
    }
}

fn review_progress_path() -> Option<PathBuf> {
    Some(super::state_root()?.join(REVIEW_PROGRESS_FILE))
}

fn read_records() -> Vec<ReviewProgressRecord> {
    let Some(path) = review_progress_path() else {
        return Vec::new();
    };
    let Ok(data) = fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

fn write_records(records: &[ReviewProgressRecord]) {
    let Some(path) = review_progress_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        drop(fs::create_dir_all(parent));
    }
    let Ok(data) = serde_json::to_vec_pretty(records) else {
        return;
    };
    let staging = path.with_extension("json.tmp");
    if fs::write(&staging, data).is_ok() {
        drop(fs::rename(staging, path));
    }
}

#[cfg(test)]
mod tests;
