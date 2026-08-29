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

const REPOSITORY: &str = "https://github.com/acme/project";

#[test]
fn an_untracked_pull_request_reads_as_an_empty_record() {
    let _root = StateRoot::new();

    let record = load_review_progress(REPOSITORY, 42);

    assert_eq!(record.number, 42);
    assert_eq!(record.repository, REPOSITORY);
    assert!(record.viewed.is_empty());
    assert!(record.visited_oid.is_empty());
}

#[test]
fn viewed_files_round_trip_with_the_head_they_were_read_at() {
    let _root = StateRoot::new();
    let mut record = ReviewProgressRecord::new(REPOSITORY, 42);
    record.mark_viewed(Path::new("src/lib.rs"), "aaaa");
    record.mark_viewed(Path::new("src/main.rs"), "aaaa");
    record.record_visit("aaaa", "2026-08-21T02:00:00Z".to_owned());
    record_review_progress(record);

    let stored = load_review_progress(REPOSITORY, 42);

    assert_eq!(stored.viewed.len(), 2);
    assert_eq!(stored.viewed_at(Path::new("src/lib.rs")), Some("aaaa"));
    assert_eq!(stored.viewed_at(Path::new("src/other.rs")), None);
    assert_eq!(stored.visited_oid, "aaaa");
    assert_eq!(stored.visited_at, "2026-08-21T02:00:00Z");
}

#[test]
fn marking_a_file_again_replaces_its_head_rather_than_duplicating_it() {
    let mut record = ReviewProgressRecord::new(REPOSITORY, 42);
    record.mark_viewed(Path::new("src/lib.rs"), "aaaa");
    record.mark_viewed(Path::new("src/lib.rs"), "bbbb");

    assert_eq!(record.viewed.len(), 1);
    assert_eq!(record.viewed_at(Path::new("src/lib.rs")), Some("bbbb"));
}

#[test]
fn unmarking_reports_whether_it_changed_anything() {
    let mut record = ReviewProgressRecord::new(REPOSITORY, 42);
    record.mark_viewed(Path::new("src/lib.rs"), "aaaa");

    assert!(record.mark_unviewed(Path::new("src/lib.rs")));
    assert!(!record.mark_unviewed(Path::new("src/lib.rs")));
    assert!(record.viewed.is_empty());
}

#[test]
fn a_repository_url_matches_regardless_of_a_trailing_slash_or_case() {
    let _root = StateRoot::new();
    let mut record = ReviewProgressRecord::new("https://github.com/acme/project/", 42);
    record.mark_viewed(Path::new("src/lib.rs"), "aaaa");
    record_review_progress(record);

    assert_eq!(
        load_review_progress("https://GitHub.com/acme/project", 42)
            .viewed_at(Path::new("src/lib.rs")),
        Some("aaaa")
    );
}

#[test]
fn a_different_pull_request_in_the_same_repository_keeps_its_own_record() {
    let _root = StateRoot::new();
    let mut first = ReviewProgressRecord::new(REPOSITORY, 42);
    first.mark_viewed(Path::new("src/lib.rs"), "aaaa");
    record_review_progress(first);
    let mut second = ReviewProgressRecord::new(REPOSITORY, 43);
    second.mark_viewed(Path::new("src/other.rs"), "bbbb");
    record_review_progress(second);

    assert_eq!(load_review_progress(REPOSITORY, 42).viewed.len(), 1);
    assert_eq!(
        load_review_progress(REPOSITORY, 43).viewed_at(Path::new("src/other.rs")),
        Some("bbbb")
    );
}

#[test]
fn forgetting_removes_only_the_named_pull_request() {
    let _root = StateRoot::new();
    for number in [42, 43] {
        let mut record = ReviewProgressRecord::new(REPOSITORY, number);
        record.mark_viewed(Path::new("src/lib.rs"), "aaaa");
        record_review_progress(record);
    }

    forget_review_progress(REPOSITORY, 42);

    assert!(load_review_progress(REPOSITORY, 42).viewed.is_empty());
    assert_eq!(load_review_progress(REPOSITORY, 43).viewed.len(), 1);
}

#[test]
fn the_record_list_is_capped_and_drops_the_least_recently_touched() {
    let _root = StateRoot::new();
    for number in 1..=u64::try_from(MAX_TRACKED_PULL_REQUESTS + 4).unwrap_or_default() {
        let mut record = ReviewProgressRecord::new(REPOSITORY, number);
        record.mark_viewed(Path::new("src/lib.rs"), "aaaa");
        record_review_progress(record);
    }

    assert_eq!(read_records().len(), MAX_TRACKED_PULL_REQUESTS);
    assert!(load_review_progress(REPOSITORY, 1).viewed.is_empty());
    assert_eq!(
        load_review_progress(
            REPOSITORY,
            u64::try_from(MAX_TRACKED_PULL_REQUESTS + 4).unwrap_or_default()
        )
        .viewed
        .len(),
        1
    );
}

#[test]
fn an_unreadable_document_is_treated_as_empty_rather_than_failing() {
    let root = StateRoot::new();
    let path = review_progress_path().expect("a state root");
    fs::create_dir_all(path.parent().expect("a parent")).expect("the state directory");
    fs::write(&path, b"{not json").expect("a corrupt document");

    assert!(load_review_progress(REPOSITORY, 42).viewed.is_empty());

    let mut record = ReviewProgressRecord::new(REPOSITORY, 42);
    record.mark_viewed(Path::new("src/lib.rs"), "aaaa");
    record_review_progress(record);
    assert_eq!(load_review_progress(REPOSITORY, 42).viewed.len(), 1);
    drop(root);
}

#[test]
fn viewed_files_are_capped_so_one_record_cannot_grow_without_bound() {
    let mut record = ReviewProgressRecord::new(REPOSITORY, 42);
    for index in 0..MAX_VIEWED_FILES + 3 {
        record.mark_viewed(&PathBuf::from(format!("src/file-{index}.rs")), "aaaa");
    }

    assert_eq!(record.viewed.len(), MAX_VIEWED_FILES);
    assert_eq!(record.viewed_at(Path::new("src/file-0.rs")), None);
    assert_eq!(
        record.viewed_at(&PathBuf::from(format!(
            "src/file-{}.rs",
            MAX_VIEWED_FILES + 2
        ))),
        Some("aaaa")
    );
}
