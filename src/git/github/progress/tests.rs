use super::delta::{ReviewProgressInputs, build_progress};
use super::*;
use crate::git::github::{
    PullRequestFile, PullRequestReviewComment, PullRequestReviewSide,
    PullRequestReviewThreadSubject,
};
use crate::state::ViewedFile;

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OLDER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const BASE: &str = "cccccccccccccccccccccccccccccccccccccccc";

fn file(path: &str) -> PullRequestFile {
    PullRequestFile {
        path: PathBuf::from(path),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: None,
    }
}

fn index(paths: &[&str]) -> PullRequestDiffIndex {
    PullRequestDiffIndex {
        total_files: paths.len(),
        files: paths.iter().map(|path| file(path)).collect(),
        truncated: false,
    }
}

fn comment(author: &str, body: &str, mine: bool) -> PullRequestReviewComment {
    PullRequestReviewComment {
        id: format!("COMMENT_{author}"),
        author: author.to_owned(),
        body: body.to_owned(),
        created_at: String::new(),
        updated_at: String::new(),
        url: String::new(),
        state: "SUBMITTED".to_owned(),
        viewer_did_author: mine,
        viewer_can_update: mine,
        viewer_can_delete: mine,
    }
}

fn thread(
    id: &str,
    path: &str,
    resolved: bool,
    outdated: bool,
    comments: Vec<PullRequestReviewComment>,
) -> PullRequestReviewThread {
    PullRequestReviewThread {
        id: id.to_owned(),
        path: PathBuf::from(path),
        side: PullRequestReviewSide::Right,
        line: Some(12),
        original_line: None,
        start_side: None,
        start_line: None,
        original_start_line: None,
        subject: PullRequestReviewThreadSubject::Line,
        is_resolved: resolved,
        is_outdated: outdated,
        resolved_by: None,
        viewer_can_reply: true,
        viewer_can_resolve: true,
        viewer_can_unresolve: false,
        comments,
        comments_truncated: false,
    }
}

fn commit(oid: &str, subject: &str) -> PullRequestCommit {
    PullRequestCommit {
        oid: oid.to_owned(),
        abbreviated_oid: oid.chars().take(7).collect(),
        subject: subject.to_owned(),
        ..PullRequestCommit::default()
    }
}

fn commits(oids: &[(&str, &str)]) -> PullRequestCommits {
    PullRequestCommits {
        commits: oids
            .iter()
            .map(|(oid, subject)| commit(oid, subject))
            .collect(),
        total_commits: oids.len(),
        truncated: false,
        base_oid: BASE.to_owned(),
        head_oid: HEAD.to_owned(),
        from_cache: false,
    }
}

fn record(viewed: &[(&str, &str)]) -> ReviewProgressRecord {
    ReviewProgressRecord {
        repository: "https://github.com/acme/project".to_owned(),
        number: 42,
        visited_oid: String::new(),
        visited_at: String::new(),
        viewed: viewed
            .iter()
            .map(|(path, oid)| ViewedFile {
                path: PathBuf::from(path),
                head_oid: (*oid).to_owned(),
            })
            .collect(),
    }
}

fn review(threads: Vec<PullRequestReviewThread>) -> PullRequestReviewSnapshot {
    PullRequestReviewSnapshot {
        pull_request_id: "PR_42".to_owned(),
        head_oid: HEAD.to_owned(),
        threads,
        ..PullRequestReviewSnapshot::default()
    }
}

fn changed(paths: &[&str]) -> BTreeSet<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

struct Scenario {
    index: PullRequestDiffIndex,
    review: PullRequestReviewSnapshot,
    commits: PullRequestCommits,
    record: ReviewProgressRecord,
    since: ReviewSince,
    changed_since: Option<BTreeSet<PathBuf>>,
    changed_since_viewed: Vec<(String, BTreeSet<PathBuf>)>,
}

impl Scenario {
    fn new() -> Self {
        Self {
            index: index(&["src/lib.rs", "src/main.rs", "README.md"]),
            review: review(Vec::new()),
            commits: commits(&[(OLDER, "first"), (HEAD, "second")]),
            record: record(&[]),
            since: ReviewSince {
                oid: OLDER.to_owned(),
                source: ReviewSinceSource::Visit,
                detail: String::new(),
            },
            changed_since: Some(changed(&["src/lib.rs"])),
            changed_since_viewed: Vec::new(),
        }
    }

    fn build(&self) -> ReviewProgress {
        build_progress(ReviewProgressInputs {
            repository: "acme/project",
            number: 42,
            head_oid: HEAD,
            since: self.since.clone(),
            record: &self.record,
            index: &self.index,
            review: &self.review,
            commits: &self.commits,
            changed_since: self.changed_since.as_ref(),
            changed_since_viewed: &self.changed_since_viewed,
            warnings: Vec::new(),
        })
    }
}

#[test]
fn nothing_read_yet_leaves_every_file_remaining() {
    let progress = Scenario::new().build();

    assert_eq!(progress.viewed, 0);
    assert_eq!(progress.remaining, 3);
    assert_eq!(progress.schema_version, ReviewProgress::SCHEMA_VERSION);
    assert!(
        progress
            .files
            .iter()
            .all(|file| file.state == ReviewFileState::Unviewed)
    );
    assert_eq!(
        progress.next,
        Some(ReviewNextStep::File {
            path: PathBuf::from("src/lib.rs"),
            state: ReviewFileState::Unviewed,
        })
    );
}

#[test]
fn a_file_read_at_the_current_head_is_done() {
    let mut scenario = Scenario::new();
    scenario.record = record(&[("src/lib.rs", HEAD)]);

    let progress = scenario.build();

    assert_eq!(progress.viewed, 1);
    assert_eq!(progress.remaining, 2);
    assert_eq!(progress.files[0].state, ReviewFileState::Viewed);
    assert_eq!(progress.files[0].viewed_at_oid, HEAD);
}

#[test]
fn a_file_read_earlier_that_did_not_move_stays_done() {
    let mut scenario = Scenario::new();
    scenario.record = record(&[("src/main.rs", OLDER)]);
    scenario.changed_since_viewed = vec![(OLDER.to_owned(), changed(&["src/lib.rs"]))];

    let progress = scenario.build();

    assert_eq!(progress.viewed, 1);
    assert_eq!(progress.changed_since_viewed, 0);
    assert_eq!(progress.files[1].state, ReviewFileState::Viewed);
}

#[test]
fn a_file_read_earlier_that_moved_since_reopens() {
    let mut scenario = Scenario::new();
    scenario.record = record(&[("src/lib.rs", OLDER)]);
    scenario.changed_since_viewed = vec![(OLDER.to_owned(), changed(&["src/lib.rs"]))];

    let progress = scenario.build();

    assert_eq!(progress.viewed, 0);
    assert_eq!(progress.changed_since_viewed, 1);
    assert_eq!(progress.files[0].state, ReviewFileState::ChangedSinceViewed);
    assert_eq!(
        progress.next,
        Some(ReviewNextStep::File {
            path: PathBuf::from("src/lib.rs"),
            state: ReviewFileState::ChangedSinceViewed,
        })
    );
}

#[test]
fn a_file_read_at_a_commit_that_cannot_be_compared_counts_as_remaining() {
    let mut scenario = Scenario::new();
    scenario.record = record(&[("src/lib.rs", OLDER)]);

    let progress = scenario.build();

    assert_eq!(
        progress.files[0].state,
        ReviewFileState::ViewedAtUnknownCommit
    );
    assert_eq!(progress.viewed, 0);
    assert_eq!(progress.remaining, 3);
    assert!(ReviewFileState::ViewedAtUnknownCommit.is_remaining());
}

#[test]
fn a_changed_file_that_moved_beats_one_never_read() {
    let mut scenario = Scenario::new();
    scenario.record = record(&[("src/main.rs", OLDER)]);
    scenario.changed_since_viewed = vec![(OLDER.to_owned(), changed(&["src/main.rs"]))];

    assert_eq!(
        scenario.build().next,
        Some(ReviewNextStep::File {
            path: PathBuf::from("src/main.rs"),
            state: ReviewFileState::ChangedSinceViewed,
        })
    );
}

#[test]
fn the_delta_marks_the_files_that_moved_since_the_named_commit() {
    let progress = Scenario::new().build();

    assert_eq!(progress.changed_since, 1);
    assert!(progress.files[0].changed_since);
    assert!(!progress.files[1].changed_since);
}

#[test]
fn commits_after_the_since_commit_are_the_new_ones() {
    let mut scenario = Scenario::new();
    scenario.commits = commits(&[(BASE, "base"), (OLDER, "older"), (HEAD, "head")]);
    scenario.since = ReviewSince {
        oid: OLDER.to_owned(),
        source: ReviewSinceSource::Review,
        detail: "APPROVED".to_owned(),
    };

    let progress = scenario.build();

    assert_eq!(progress.new_commits.len(), 1);
    assert_eq!(progress.new_commits[0].oid, HEAD);
}

#[test]
fn a_since_commit_the_pull_request_no_longer_holds_treats_everything_as_new() {
    let mut scenario = Scenario::new();
    scenario.since = ReviewSince {
        oid: "dddddddddddddddddddddddddddddddddddddddd".to_owned(),
        source: ReviewSinceSource::Visit,
        detail: String::new(),
    };

    assert_eq!(scenario.build().new_commits.len(), 2);
}

#[test]
fn threads_are_split_by_who_owes_the_next_word() {
    let mut scenario = Scenario::new();
    scenario.review = review(vec![
        thread(
            "T1",
            "src/lib.rs",
            false,
            false,
            vec![comment("hubot", "Please rename this", false)],
        ),
        thread(
            "T2",
            "src/main.rs",
            false,
            false,
            vec![
                comment("hubot", "And this", false),
                comment("octocat", "Done", true),
            ],
        ),
        thread(
            "T3",
            "README.md",
            false,
            true,
            vec![comment("hubot", "Outdated point", false)],
        ),
        thread("T4", "src/lib.rs", true, false, Vec::new()),
    ]);

    let progress = scenario.build();

    assert_eq!(progress.threads.total, 4);
    assert_eq!(progress.threads.unresolved, 3);
    assert_eq!(progress.threads.awaiting_your_reply, 2);
    assert_eq!(progress.threads.awaiting_others, 1);
    assert_eq!(progress.threads.outdated_unresolved, 1);
}

#[test]
fn threads_come_after_files_and_prefer_one_awaiting_your_reply() {
    let mut scenario = Scenario::new();
    scenario.index = index(&[]);
    scenario.review = review(vec![
        thread(
            "T1",
            "src/main.rs",
            false,
            false,
            vec![comment("octocat", "Fixed", true)],
        ),
        thread(
            "T2",
            "src/lib.rs",
            false,
            false,
            vec![comment("hubot", "Please rename this variable", false)],
        ),
    ]);

    let progress = scenario.build();

    assert_eq!(
        progress.next,
        Some(ReviewNextStep::Thread {
            id: "T2".to_owned(),
            path: PathBuf::from("src/lib.rs"),
            line: Some(12),
            outdated: false,
            author: "hubot".to_owned(),
            excerpt: "Please rename this variable".to_owned(),
        })
    );
    assert_eq!(progress.next_file(), None);
    assert_eq!(progress.next_thread(), progress.next);
}

#[test]
fn a_thread_step_is_available_even_while_files_remain() {
    let mut scenario = Scenario::new();
    scenario.review = review(vec![thread(
        "T1",
        "src/lib.rs",
        false,
        false,
        vec![comment("hubot", "Look here", false)],
    )]);

    let progress = scenario.build();

    assert!(matches!(progress.next, Some(ReviewNextStep::File { .. })));
    assert!(matches!(
        progress.next_thread(),
        Some(ReviewNextStep::Thread { .. })
    ));
    assert!(progress.next_file().is_some());
}

#[test]
fn a_finished_review_reports_complete_and_offers_no_next_step() {
    let mut scenario = Scenario::new();
    scenario.record = record(&[
        ("src/lib.rs", HEAD),
        ("src/main.rs", HEAD),
        ("README.md", HEAD),
    ]);

    let progress = scenario.build();

    assert!(progress.is_complete());
    assert_eq!(progress.next, None);
    assert_eq!(progress.remaining, 0);
}

#[test]
fn a_long_comment_is_cut_to_one_line_for_the_queue() {
    let mut scenario = Scenario::new();
    scenario.index = index(&[]);
    let long = "x".repeat(200);
    scenario.review = review(vec![thread(
        "T1",
        "src/lib.rs",
        false,
        false,
        vec![comment("hubot", &format!("\n\n{long}\nsecond line"), false)],
    )]);

    let Some(ReviewNextStep::Thread { excerpt, .. }) = scenario.build().next else {
        panic!("expected a thread step");
    };
    assert_eq!(excerpt.chars().count(), 72);
    assert!(excerpt.ends_with('…'));
}

#[test]
fn a_truncated_index_or_review_is_reported() {
    let mut scenario = Scenario::new();
    scenario.index.truncated = true;

    assert!(scenario.build().truncated);

    let mut scenario = Scenario::new();
    scenario.review.truncated = true;
    assert!(scenario.build().truncated);
}

#[test]
fn every_since_source_names_itself_for_the_reading() {
    let sources = [
        (ReviewSinceSource::Visit, "your last visit"),
        (ReviewSinceSource::Review, "your last review"),
        (ReviewSinceSource::Explicit, "the commit you named"),
        (ReviewSinceSource::MergeBase, "the merge base"),
    ];
    for (source, label) in sources {
        assert_eq!(source.label(), label);
    }
}

#[test]
fn every_file_state_has_a_word_and_says_whether_it_is_work() {
    let states = [
        (ReviewFileState::Unviewed, "unviewed", true),
        (ReviewFileState::Viewed, "viewed", false),
        (ReviewFileState::ChangedSinceViewed, "changed", true),
        (ReviewFileState::ViewedAtUnknownCommit, "unknown", true),
    ];
    for (state, word, remaining) in states {
        assert_eq!(state.word(), word);
        assert_eq!(state.is_remaining(), remaining);
    }
}

mod resolve;
