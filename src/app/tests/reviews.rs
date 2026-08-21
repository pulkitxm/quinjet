use super::*;

#[test]
fn pull_request_files_create_pending_line_reviews() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.focus = Focus::Content;
    app.pull_request = Some(pull_request(12, "Review me", "acme/widget"));
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_file_view = PullRequestFileView::SingleFile;
    app.pull_request_single_file = Some(PathBuf::from("src/lib.rs"));
    app.document = crate::git::diff::parse_diff(
        b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n",
        "src/lib.rs",
        Some(Path::new("src/lib.rs")),
        false,
    );

    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now);
    assert!(matches!(
        app.modal,
        Some(Modal::PullRequestReviewComment { .. })
    ));
    app.handle_paste("Use the shared parser here");
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL), now);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::OperatePullRequestReview {
                operation: PullRequestReviewOperation::AddThread {
                    path,
                    line: Some(1),
                    side: Some(PullRequestReviewSide::Left),
                    body,
                    ..
                },
                ..
            } if path == Path::new("src/lib.rs") && body == "Use the shared parser here"
        )
    ));
}

#[test]
fn pull_request_review_threads_render_below_their_diff_line() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_file_view = PullRequestFileView::SingleFile;
    app.pull_request_single_file = Some(PathBuf::from("src/lib.rs"));
    app.document = crate::git::diff::parse_diff(
        b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -0,0 +1 @@\n+new\n",
        "src/lib.rs",
        Some(Path::new("src/lib.rs")),
        false,
    );
    app.pull_request_review_generation = 4;

    app.handle_worker_event(
        WorkerEvent::PullRequestReview {
            generation: 4,
            result: Ok(PullRequestReviewSnapshot {
                pull_request_id: "PR_1".to_owned(),
                head_oid: "head".to_owned(),
                review_decision: None,
                pending_review: None,
                threads: vec![PullRequestReviewThread {
                    id: "thread-1".to_owned(),
                    path: PathBuf::from("src/lib.rs"),
                    side: PullRequestReviewSide::Right,
                    line: Some(1),
                    original_line: Some(1),
                    start_side: None,
                    start_line: None,
                    original_start_line: None,
                    subject: PullRequestReviewThreadSubject::Line,
                    is_resolved: false,
                    is_outdated: false,
                    resolved_by: None,
                    viewer_can_reply: true,
                    viewer_can_resolve: true,
                    viewer_can_unresolve: false,
                    comments: Vec::new(),
                    comments_truncated: false,
                }],
                truncated: false,
            }),
        },
        Instant::now(),
    );

    let added = app
        .document
        .lines
        .iter()
        .position(|line| line.kind == DiffLineKind::Added);
    let review = app
        .document
        .lines
        .iter()
        .position(|line| line.kind == DiffLineKind::Review);
    assert_eq!(review, added.map(|index| index + 1));
}
