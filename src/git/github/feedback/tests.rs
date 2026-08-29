use super::queue::FeedbackInputs;
use super::*;
use crate::git::github::{
    AnnotationPlacement, GitHubRepository, MergeGateReview, PullRequestReviewComment,
    PullRequestReviewSide, PullRequestReviewThreadSubject,
};

const VIEWER: &str = "octocat";

fn pull_request() -> PullRequest {
    PullRequest {
        number: 42,
        head_oid: "a".repeat(40),
        url: "https://github.com/acme/project/pull/42".to_owned(),
        base_repository: GitHubRepository {
            name_with_owner: "acme/project".to_owned(),
            url: "https://github.com/acme/project".to_owned(),
            remotes: Vec::new(),
        },
        ..PullRequest::default()
    }
}

fn comment(author: &str, body: &str) -> PullRequestReviewComment {
    PullRequestReviewComment {
        id: format!("COMMENT_{author}"),
        author: author.to_owned(),
        body: body.to_owned(),
        created_at: String::new(),
        updated_at: String::new(),
        url: "https://github.com/acme/project/pull/42#c1".to_owned(),
        state: "SUBMITTED".to_owned(),
        viewer_did_author: author == VIEWER,
        viewer_can_update: false,
        viewer_can_delete: false,
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

fn review(threads: Vec<PullRequestReviewThread>) -> PullRequestReviewSnapshot {
    PullRequestReviewSnapshot {
        threads,
        ..PullRequestReviewSnapshot::default()
    }
}

fn annotation(
    check: &str,
    path: &str,
    line: usize,
    severity: AnnotationSeverity,
) -> CheckAnnotation {
    CheckAnnotation {
        check: check.to_owned(),
        check_run_id: 900,
        check_url: "https://example.test/run".to_owned(),
        path: PathBuf::from(path),
        start_line: line,
        end_line: line,
        start_column: None,
        end_column: None,
        severity,
        title: format!("{check} finding"),
        message: "Something to fix".to_owned(),
        raw_details: String::new(),
        url: "https://example.test/a1".to_owned(),
        placement: AnnotationPlacement::InDiff,
    }
}

fn annotations(items: Vec<CheckAnnotation>) -> PullRequestAnnotations {
    let mut listing = PullRequestAnnotations {
        annotations: items,
        ..PullRequestAnnotations::default()
    };
    listing.finish();
    listing
}

fn gate_with(changes_requested: Vec<String>) -> MergeGate {
    MergeGate {
        schema_version: MergeGate::SCHEMA_VERSION,
        repository: "acme/project".to_owned(),
        number: 42,
        title: "Add feature".to_owned(),
        url: "https://github.com/acme/project/pull/42".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        verdict: crate::git::github::MergeGateVerdict::Blocked,
        blockers: Vec::new(),
        checks: crate::git::github::MergeGateChecks::default(),
        review: MergeGateReview {
            changes_requested_by: changes_requested,
            ..MergeGateReview::default()
        },
        branch: crate::git::github::MergeGateBranch::default(),
        queue: None,
        auto_merge: crate::git::github::MergeGateAutoMerge::default(),
        warnings: Vec::new(),
        from_cache: false,
    }
}

fn build(
    gate: Option<&MergeGate>,
    review: &PullRequestReviewSnapshot,
    annotations: Option<&PullRequestAnnotations>,
) -> PullRequestFeedback {
    build_feedback(&FeedbackInputs {
        pull_request: &pull_request(),
        viewer: VIEWER,
        gate,
        review,
        annotations,
        warnings: Vec::new(),
    })
}

#[test]
fn an_empty_pull_request_has_an_empty_queue() {
    let queue = build(None, &review(Vec::new()), None);

    assert_eq!(queue.items, Vec::new());
    assert_eq!(queue.counts.blocking, 0);
    assert_eq!(queue.next_blocker(), None);
    assert_eq!(queue.schema_version, PullRequestFeedback::SCHEMA_VERSION);
    assert_eq!(queue.viewer, VIEWER);
}

#[test]
fn rows_are_ordered_by_how_directly_they_stand_in_the_way() {
    let queue = build(
        Some(&gate_with(vec!["hubot".to_owned()])),
        &review(vec![
            thread(
                "T1",
                "src/lib.rs",
                false,
                false,
                vec![comment("hubot", "Fix")],
            ),
            thread(
                "T2",
                "src/lib.rs",
                false,
                true,
                vec![comment("hubot", "Old")],
            ),
        ]),
        Some(&annotations(vec![
            annotation("Clippy", "src/lib.rs", 5, AnnotationSeverity::Failure),
            annotation("Spell", "README.md", 2, AnnotationSeverity::Notice),
        ])),
    );

    let kinds: Vec<FeedbackKind> = queue.items.iter().map(|item| item.kind).collect();
    assert_eq!(
        kinds,
        vec![
            FeedbackKind::ChangesRequested,
            FeedbackKind::Failure,
            FeedbackKind::Thread,
            FeedbackKind::OutdatedThread,
            FeedbackKind::Advisory,
        ]
    );
    assert_eq!(queue.counts.blocking, 3);
    assert_eq!(queue.counts.advisory, 2);
    assert_eq!(
        queue.next_blocker().map(|item| item.kind),
        Some(FeedbackKind::ChangesRequested)
    );
}

#[test]
fn a_thread_whose_newest_word_is_yours_is_waiting_on_somebody_else() {
    let queue = build(
        None,
        &review(vec![
            thread(
                "T1",
                "src/lib.rs",
                false,
                false,
                vec![comment("hubot", "Fix"), comment(VIEWER, "Done")],
            ),
            thread(
                "T2",
                "src/main.rs",
                false,
                false,
                vec![comment("hubot", "Also")],
            ),
        ]),
        None,
    );

    assert_eq!(queue.items[0].owner, FeedbackOwner::Others);
    assert!(queue.items[0].mine);
    assert_eq!(
        queue.items[0].action,
        "waiting on a reply from somebody else"
    );
    assert_eq!(queue.items[1].owner, FeedbackOwner::You);
    assert!(!queue.items[1].mine);
    assert!(
        queue.items[1].action.contains("quinjet pr reviews reply"),
        "{}",
        queue.items[1].action
    );
    assert_eq!(queue.counts.awaiting_you, 1);
    assert_eq!(queue.counts.awaiting_others, 1);
}

#[test]
fn a_resolved_thread_never_reaches_the_queue() {
    let queue = build(
        None,
        &review(vec![
            thread(
                "T1",
                "src/lib.rs",
                true,
                false,
                vec![comment("hubot", "Fixed")],
            ),
            thread(
                "T2",
                "src/lib.rs",
                false,
                false,
                vec![comment("hubot", "Open")],
            ),
        ]),
        None,
    );

    assert_eq!(queue.items.len(), 1);
    assert_eq!(queue.items[0].id, "T2");
}

#[test]
fn an_outdated_thread_is_advisory_and_says_to_resolve_it() {
    let queue = build(
        None,
        &review(vec![thread(
            "T1",
            "src/lib.rs",
            false,
            true,
            vec![comment("hubot", "This moved")],
        )]),
        None,
    );

    assert_eq!(queue.items[0].kind, FeedbackKind::OutdatedThread);
    assert!(!FeedbackKind::OutdatedThread.is_blocking());
    assert!(
        queue.items[0].action.contains("quinjet pr reviews resolve"),
        "{}",
        queue.items[0].action
    );
    assert_eq!(queue.counts.blocking, 0);
}

#[test]
fn a_check_finding_is_owned_by_nobody_and_points_at_its_log() {
    let queue = build(
        None,
        &review(Vec::new()),
        Some(&annotations(vec![annotation(
            "windows / test",
            "src/lib.rs",
            5,
            AnnotationSeverity::Failure,
        )])),
    );

    assert_eq!(queue.items[0].owner, FeedbackOwner::Nobody);
    assert_eq!(queue.items[0].author, "windows / test");
    assert_eq!(queue.items[0].location(), "src/lib.rs:5");
    assert!(
        queue.items[0]
            .action
            .contains("quinjet pr logs <n> \"windows / test\""),
        "{}",
        queue.items[0].action
    );
    assert_eq!(queue.counts.awaiting_you, 0);
}

#[test]
fn your_own_request_for_changes_is_waiting_on_the_author_rather_than_on_you() {
    let queue = build(
        Some(&gate_with(vec![VIEWER.to_owned(), "hubot".to_owned()])),
        &review(Vec::new()),
        None,
    );

    let mine = queue
        .items
        .iter()
        .find(|item| item.author == VIEWER)
        .expect("your own row");
    assert_eq!(mine.owner, FeedbackOwner::Others);
    assert!(mine.mine);
    let theirs = queue
        .items
        .iter()
        .find(|item| item.author == "hubot")
        .expect("their row");
    assert_eq!(theirs.owner, FeedbackOwner::You);
}

#[test]
fn a_long_comment_is_cut_to_one_line_for_the_row() {
    let long = "x".repeat(200);
    let queue = build(
        None,
        &review(vec![thread(
            "T1",
            "src/lib.rs",
            false,
            false,
            vec![comment("hubot", &format!("\n\n{long}\nsecond"))],
        )]),
        None,
    );

    assert_eq!(queue.items[0].summary.chars().count(), 72);
    assert!(queue.items[0].summary.ends_with('…'));
    assert!(
        queue.items[0].body.contains("second"),
        "the whole body is kept"
    );
}

#[test]
fn the_filters_narrow_the_rows_and_the_counts_together() {
    let queue = build(
        Some(&gate_with(vec!["hubot".to_owned()])),
        &review(vec![
            thread(
                "T1",
                "src/lib.rs",
                false,
                false,
                vec![comment("hubot", "Fix")],
            ),
            thread(
                "T2",
                "src/main.rs",
                false,
                false,
                vec![comment("hubot", "Fix"), comment(VIEWER, "Done")],
            ),
        ]),
        Some(&annotations(vec![annotation(
            "Spell",
            "README.md",
            2,
            AnnotationSeverity::Notice,
        )])),
    );
    assert_eq!(queue.items.len(), 4);

    let blocking = FeedbackFilter {
        blocking_only: true,
        mine_only: false,
    }
    .apply(queue.clone());
    assert_eq!(blocking.items.len(), 3);
    assert_eq!(blocking.counts.advisory, 0);

    let mine = FeedbackFilter {
        blocking_only: false,
        mine_only: true,
    }
    .apply(queue.clone());
    assert_eq!(mine.items.len(), 2);
    assert!(
        mine.items
            .iter()
            .all(|item| item.owner == FeedbackOwner::You)
    );

    let both = FeedbackFilter {
        blocking_only: true,
        mine_only: true,
    }
    .apply(queue);
    assert_eq!(both.items.len(), 2);
    assert_eq!(both.counts.blocking, 2);
    assert_eq!(both.counts.awaiting_others, 0);
}

#[test]
fn a_truncated_review_or_annotation_listing_is_reported() {
    let mut truncated = review(Vec::new());
    truncated.truncated = true;
    assert!(build(None, &truncated, None).truncated);

    let mut listing = annotations(Vec::new());
    listing.truncated = true;
    assert!(build(None, &review(Vec::new()), Some(&listing)).truncated);
}

#[test]
fn every_kind_and_owner_names_itself() {
    let kinds = [
        (FeedbackKind::ChangesRequested, "changes", true),
        (FeedbackKind::Failure, "failure", true),
        (FeedbackKind::Thread, "thread", true),
        (FeedbackKind::OutdatedThread, "outdated", false),
        (FeedbackKind::Advisory, "advisory", false),
    ];
    for (kind, word, blocking) in kinds {
        assert_eq!(kind.word(), word);
        assert_eq!(kind.is_blocking(), blocking);
    }
    assert_eq!(FeedbackOwner::You.word(), "you");
    assert_eq!(FeedbackOwner::Others.word(), "others");
    assert_eq!(FeedbackOwner::Nobody.word(), "-");
}
