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

#[test]
fn clicking_a_review_thread_opens_permission_aware_actions() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.focus = Focus::Content;
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_file_view = PullRequestFileView::SingleFile;
    app.pull_request_single_file = Some(PathBuf::from("src/lib.rs"));
    app.document = crate::git::diff::parse_diff(
        b"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -0,0 +1 @@\n+new\n",
        "src/lib.rs",
        Some(Path::new("src/lib.rs")),
        false,
    );
    app.pull_request_review = PullRequestReviewSnapshot {
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
            comments: vec![crate::git::github::PullRequestReviewComment {
                id: "comment-1".to_owned(),
                author: "reviewer".to_owned(),
                body: "Use the shared parser".to_owned(),
                created_at: String::new(),
                updated_at: String::new(),
                url: "https://github.com/acme/widget/pull/1#discussion_r1".to_owned(),
                state: "PENDING".to_owned(),
                viewer_did_author: true,
                viewer_can_update: true,
                viewer_can_delete: true,
            }],
            comments_truncated: false,
        }],
        truncated: false,
    };
    app.decorate_pull_request_review();
    let mut terminal = Terminal::new(TestBackend::new(140, 32)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let hit = app.geometry.content_review_hits.first().unwrap().clone();
    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.area.x,
            row: hit.area.y,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );

    let Some(Modal::PullRequestReviewThreadActions { items, .. }) = app.modal else {
        panic!("review actions did not open");
    };
    assert!(matches!(
        items.as_slice(),
        [
            PullRequestReviewThreadAction::Reply { .. },
            PullRequestReviewThreadAction::CopyComment { .. },
            PullRequestReviewThreadAction::OpenComment { .. },
            PullRequestReviewThreadAction::EditComment { .. },
            PullRequestReviewThreadAction::DeleteComment { .. },
            PullRequestReviewThreadAction::Resolve { .. }
        ]
    ));
}

#[test]
fn editing_a_review_thread_action_updates_the_comment() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request = Some(pull_request(12, "Review me", "acme/widget"));
    app.modal = Some(Modal::PullRequestReviewThreadActions {
        items: vec![PullRequestReviewThreadAction::EditComment {
            comment_id: "comment-1".to_owned(),
            body: "Original".to_owned(),
        }],
        selected: 0,
    });
    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    let Some(Modal::PullRequestReviewComment { input, .. }) = app.modal.as_mut() else {
        panic!("review editor did not open");
    };
    input.value = "Updated".to_owned();
    input.cursor = input.value.len();
    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::OperatePullRequestReview {
                operation: PullRequestReviewOperation::UpdateComment {
                    comment_id,
                    body,
                },
                ..
            } if comment_id == "comment-1" && body == "Updated"
        )
    ));
}
