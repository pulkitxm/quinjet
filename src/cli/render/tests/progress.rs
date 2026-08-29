use std::path::PathBuf;

use super::*;
use crate::git::github::{
    PullRequestCommit, PullRequestFileStatus, ReviewFileProgress, ReviewFileState, ReviewSince,
    ReviewSinceSource, ReviewThreadProgress,
};

fn file(path: &str, state: ReviewFileState, changed_since: bool) -> ReviewFileProgress {
    ReviewFileProgress {
        path: PathBuf::from(path),
        status: PullRequestFileStatus::Modified,
        state,
        viewed_at_oid: String::new(),
        changed_since,
    }
}

fn sample() -> ReviewProgress {
    ReviewProgress {
        schema_version: ReviewProgress::SCHEMA_VERSION,
        repository: "acme/project".to_owned(),
        number: 42,
        head_oid: "a".repeat(40),
        since: ReviewSince {
            oid: "b".repeat(40),
            source: ReviewSinceSource::Review,
            detail: "COMMENTED".to_owned(),
        },
        visited_at: String::new(),
        files: vec![
            file("src/lib.rs", ReviewFileState::ChangedSinceViewed, true),
            file("src/main.rs", ReviewFileState::Unviewed, false),
            file("README.md", ReviewFileState::Viewed, false),
        ],
        viewed: 1,
        remaining: 2,
        changed_since_viewed: 1,
        changed_since: 1,
        new_commits: vec![PullRequestCommit {
            oid: "a".repeat(40),
            subject: "revise".to_owned(),
            ..PullRequestCommit::default()
        }],
        threads: ReviewThreadProgress {
            total: 3,
            unresolved: 2,
            outdated_unresolved: 1,
            awaiting_your_reply: 1,
            awaiting_others: 1,
        },
        next: Some(ReviewNextStep::File {
            path: PathBuf::from("src/lib.rs"),
            state: ReviewFileState::ChangedSinceViewed,
        }),
        thread_step: None,
        truncated: false,
        warnings: Vec::new(),
    }
}

#[test]
fn a_progress_reading_leads_with_the_counts_and_what_it_measured_from() {
    let text = review_progress(&sample(), false);
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(lines[0], "#42  1 of 3 files read  ·  2 unresolved");
    assert_eq!(lines[1], "since    your last review bbbbbbbbbbbb");
    assert_eq!(lines[2], "commits  1 new since then");
    assert_eq!(lines[3], "changed  1 file(s) you had read have moved since");
    assert_eq!(
        lines[4],
        "threads  1 awaiting your reply, 1 awaiting others, 1 outdated"
    );
    assert!(text.contains("next     changed src/lib.rs"), "{text}");
}

#[test]
fn only_what_is_left_is_listed_unless_everything_is_asked_for() {
    let text = review_progress(&sample(), false);
    assert!(text.contains("changed   src/lib.rs *"), "{text}");
    assert!(text.contains("unviewed  src/main.rs"), "{text}");
    assert!(!text.contains("README.md"), "{text}");

    let all = review_progress(&sample(), true);
    assert!(all.contains("viewed    README.md"), "{all}");
}

#[test]
fn a_finished_review_says_so_rather_than_printing_an_empty_list() {
    let mut progress = sample();
    progress.files = vec![file("README.md", ReviewFileState::Viewed, false)];
    progress.viewed = 1;
    progress.remaining = 0;
    progress.changed_since_viewed = 0;
    progress.threads = ReviewThreadProgress::default();
    progress.next = None;

    let text = review_progress(&progress, false);

    assert!(text.contains("Nothing left to review"), "{text}");
    assert!(!text.contains("next  "), "{text}");
}

#[test]
fn an_unknown_since_commit_is_named_rather_than_printed_blank() {
    let mut progress = sample();
    progress.since.oid = String::new();
    progress.since.source = ReviewSinceSource::MergeBase;

    assert!(
        review_progress(&progress, false).contains("since    the merge base an unknown commit"),
        "{}",
        review_progress(&progress, false)
    );
}

#[test]
fn truncation_and_warnings_are_reported_under_the_listing() {
    let mut progress = sample();
    progress.truncated = true;
    progress.warnings = vec!["unable to compare bbbbbbbbbbbb with the head commit".to_owned()];

    let text = review_progress(&progress, false);

    assert!(text.contains("reached Quinjet's size cap"), "{text}");
    assert!(
        text.contains("note     unable to compare bbbbbbbbbbbb"),
        "{text}"
    );
}

#[test]
fn a_next_file_and_a_next_thread_each_read_as_one_instruction() {
    let file_step = review_next(&ReviewNextStep::File {
        path: PathBuf::from("src/lib.rs"),
        state: ReviewFileState::Unviewed,
    });
    assert_eq!(file_step, "file    src/lib.rs\nstate   unviewed\n");

    let thread = review_next(&ReviewNextStep::Thread {
        id: "THREAD_1".to_owned(),
        path: PathBuf::from("src/lib.rs"),
        line: Some(12),
        outdated: true,
        author: "hubot".to_owned(),
        excerpt: "Please rename this".to_owned(),
    });
    assert!(thread.contains("thread  src/lib.rs:12"), "{thread}");
    assert!(thread.contains("id      THREAD_1"), "{thread}");
    assert!(thread.contains("from    @hubot"), "{thread}");
    assert!(
        thread.contains("state   outdated by a later commit"),
        "{thread}"
    );
    assert!(thread.contains("says    Please rename this"), "{thread}");
}

#[test]
fn a_file_level_thread_prints_without_a_line_number() {
    let thread = review_next(&ReviewNextStep::Thread {
        id: "THREAD_2".to_owned(),
        path: PathBuf::from("src/lib.rs"),
        line: None,
        outdated: false,
        author: "hubot".to_owned(),
        excerpt: String::new(),
    });

    assert!(thread.contains("thread  src/lib.rs\n"), "{thread}");
    assert!(!thread.contains("says"), "{thread}");
    assert!(!thread.contains("state"), "{thread}");
}
