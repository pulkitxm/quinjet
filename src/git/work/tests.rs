use std::path::PathBuf;

use super::start::build_work_session;
use super::*;
use crate::git::github::{
    FeedbackCounts, FeedbackItem, FeedbackKind, FeedbackOwner, GateCheck, GateCheckState,
    GitHubRepository, MergeGateAutoMerge, MergeGateBranch, MergeGateChecks, MergeGateReview,
    MergeGateVerdict,
};

fn pull_request() -> PullRequest {
    PullRequest {
        number: 42,
        title: "Add feature".to_owned(),
        url: "https://github.com/acme/project/pull/42".to_owned(),
        base_ref: "main".to_owned(),
        base_oid: "b".repeat(40),
        head_ref: "feature".to_owned(),
        head_oid: "a".repeat(40),
        base_repository: GitHubRepository {
            name_with_owner: "acme/project".to_owned(),
            url: "https://github.com/acme/project".to_owned(),
            remotes: Vec::new(),
        },
        ..PullRequest::default()
    }
}

fn feedback(items: Vec<FeedbackItem>) -> PullRequestFeedback {
    PullRequestFeedback {
        items,
        counts: FeedbackCounts::default(),
        ..PullRequestFeedback::default()
    }
}

fn item(kind: FeedbackKind, id: &str, summary: &str) -> FeedbackItem {
    FeedbackItem {
        kind,
        id: id.to_owned(),
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(9),
        author: "hubot".to_owned(),
        summary: summary.to_owned(),
        body: summary.to_owned(),
        url: String::new(),
        owner: FeedbackOwner::You,
        mine: false,
        action: format!("quinjet pr reviews reply 42 {id}"),
    }
}

fn gate_with(checks: Vec<GateCheck>) -> MergeGate {
    MergeGate {
        schema_version: MergeGate::SCHEMA_VERSION,
        repository: "acme/project".to_owned(),
        number: 42,
        title: String::new(),
        url: String::new(),
        state: "OPEN".to_owned(),
        is_draft: false,
        verdict: MergeGateVerdict::Blocked,
        blockers: Vec::new(),
        checks: MergeGateChecks {
            checks,
            ..MergeGateChecks::default()
        },
        review: MergeGateReview::default(),
        branch: MergeGateBranch::default(),
        queue: None,
        auto_merge: MergeGateAutoMerge::default(),
        warnings: Vec::new(),
        from_cache: false,
    }
}

fn check(name: &str, state: GateCheckState, required: bool) -> GateCheck {
    GateCheck {
        name: name.to_owned(),
        workflow: "CI".to_owned(),
        state,
        required,
        url: format!("https://example.test/{name}"),
        awaiting_approval: false,
    }
}

fn request<'a>(
    pull_request: &'a PullRequest,
    source: WorkSource,
    feedback: Option<&'a PullRequestFeedback>,
    gate: Option<&'a MergeGate>,
    annotations: Option<&'a PullRequestAnnotations>,
) -> WorkStartRequest<'a> {
    WorkStartRequest {
        id: "w42-1".to_owned(),
        pull_request,
        source,
        feedback,
        gate,
        annotations,
        created_at: "2026-08-29T10:00:00Z".to_owned(),
    }
}

#[test]
fn a_session_records_the_commit_it_starts_from_exactly() {
    let pull_request = pull_request();

    let session = build_work_session(&request(&pull_request, WorkSource::Whole, None, None, None));

    assert_eq!(session.schema_version, WorkSession::SCHEMA_VERSION);
    assert_eq!(session.start_oid, "a".repeat(40));
    assert_eq!(session.branch, "quinjet/work/w42-1");
    assert_eq!(session.base_ref, "main");
    assert_eq!(session.head_ref, "feature");
    assert_eq!(session.state(), WorkSessionState::Open);
    assert_eq!(session.tasks, Vec::new());
}

#[test]
fn every_session_states_what_it_may_not_do() {
    let pull_request = pull_request();

    let session = build_work_session(&request(&pull_request, WorkSource::Whole, None, None, None));

    assert_eq!(session.allowed.len(), WORK_ALLOWED.len());
    assert_eq!(session.forbidden.len(), WORK_FORBIDDEN.len());
    for forbidden in ["push", "comment", "resolve", "merge"] {
        assert!(
            session
                .forbidden
                .iter()
                .any(|entry| entry.contains(forbidden)),
            "nothing forbids {forbidden}: {:?}",
            session.forbidden
        );
    }
}

#[test]
fn a_feedback_session_takes_the_blocking_rows_and_leaves_the_advisories() {
    let pull_request = pull_request();
    let queue = feedback(vec![
        item(FeedbackKind::Thread, "THREAD_1", "Please rename this"),
        item(FeedbackKind::Advisory, "NOTE_1", "Worth a look one day"),
        item(
            FeedbackKind::ChangesRequested,
            "REVIEW_1",
            "This needs a test",
        ),
    ]);

    let session = build_work_session(&request(
        &pull_request,
        WorkSource::Feedback,
        Some(&queue),
        None,
        None,
    ));

    let ids: Vec<&str> = session.tasks.iter().map(|task| task.id.as_str()).collect();
    assert_eq!(ids, ["THREAD_1", "REVIEW_1"]);
    let first = session.tasks.first().expect("one task");
    assert_eq!(first.location, "src/lib.rs:9");
    assert!(
        first.resolved_by.contains("quinjet pr reviews reply"),
        "{}",
        first.resolved_by
    );
}

#[test]
fn a_failed_check_session_names_the_checks_and_never_the_threads() {
    let pull_request = pull_request();
    let queue = feedback(vec![item(
        FeedbackKind::Thread,
        "THREAD_1",
        "Please rename this",
    )]);
    let gate = gate_with(vec![
        check("windows", GateCheckState::Failed, true),
        check("macos", GateCheckState::Passed, true),
    ]);

    let session = build_work_session(&request(
        &pull_request,
        WorkSource::FailedChecks,
        Some(&queue),
        Some(&gate),
        None,
    ));

    assert_eq!(session.tasks.len(), 1);
    let task = session.tasks.first().expect("one task");
    assert_eq!(task.kind, "check");
    assert_eq!(task.id, "windows");
    assert!(task.summary.contains("required"), "{}", task.summary);
    assert_eq!(task.resolved_by, "quinjet pr logs 42 windows");
}

#[test]
fn a_check_name_with_a_space_is_quoted_in_the_command_it_suggests() {
    let pull_request = pull_request();
    let gate = gate_with(vec![check("build and test", GateCheckState::Failed, false)]);

    let session = build_work_session(&request(
        &pull_request,
        WorkSource::FailedChecks,
        None,
        Some(&gate),
        None,
    ));

    let task = session.tasks.first().expect("one task");
    assert_eq!(task.resolved_by, "quinjet pr logs 42 'build and test'");
}

#[test]
fn a_session_started_from_the_whole_change_carries_no_task_list() {
    let pull_request = pull_request();
    let queue = feedback(vec![item(
        FeedbackKind::Thread,
        "THREAD_1",
        "Please rename this",
    )]);
    let gate = gate_with(vec![check("windows", GateCheckState::Failed, true)]);

    let session = build_work_session(&request(
        &pull_request,
        WorkSource::Whole,
        Some(&queue),
        Some(&gate),
        None,
    ));

    assert_eq!(session.tasks, Vec::new());
    assert_eq!(session.source(), WorkSource::Whole);
}

#[test]
fn a_session_that_has_run_nothing_has_not_verified_anything() {
    let pull_request = pull_request();
    let mut session =
        build_work_session(&request(&pull_request, WorkSource::Whole, None, None, None));

    assert!(!session.verified());
    assert_eq!(session.failing_verification(), None);

    session.push_verification(WorkVerification {
        command: vec!["cargo".to_owned(), "test".to_owned()],
        exit_code: 0,
        passed: true,
        ran_at: String::new(),
        output: String::new(),
    });
    assert!(session.verified());
}

#[test]
fn re_running_a_command_replaces_its_result_rather_than_stacking_a_second_one() {
    let pull_request = pull_request();
    let mut session =
        build_work_session(&request(&pull_request, WorkSource::Whole, None, None, None));
    let command = vec!["cargo".to_owned(), "test".to_owned()];

    session.push_verification(WorkVerification {
        command: command.clone(),
        exit_code: 101,
        passed: false,
        ran_at: String::new(),
        output: "one test failed".to_owned(),
    });
    session.push_verification(WorkVerification {
        command,
        exit_code: 0,
        passed: true,
        ran_at: String::new(),
        output: String::new(),
    });

    assert_eq!(session.verifications.len(), 1);
    assert!(session.verified());
    assert_eq!(session.failing_verification(), None);
}

#[test]
fn a_failing_verification_is_the_one_the_session_reports() {
    let pull_request = pull_request();
    let mut session =
        build_work_session(&request(&pull_request, WorkSource::Whole, None, None, None));

    session.push_verification(WorkVerification {
        command: vec!["cargo".to_owned(), "fmt".to_owned()],
        exit_code: 0,
        passed: true,
        ran_at: String::new(),
        output: String::new(),
    });
    session.push_verification(WorkVerification {
        command: vec!["cargo".to_owned(), "clippy".to_owned()],
        exit_code: 101,
        passed: false,
        ran_at: String::new(),
        output: "one lint fired".to_owned(),
    });

    assert!(!session.verified());
    let failing = session.failing_verification().expect("one failure");
    assert_eq!(failing.display_command(), "cargo clippy");
}

#[test]
fn a_session_without_a_worktree_cannot_be_measured_or_verified() {
    let pull_request = pull_request();
    let session = build_work_session(&request(&pull_request, WorkSource::Whole, None, None, None));

    let error = work_diff(&session).expect_err("a session with no checkout has nothing to diff");

    assert!(format!("{error:#}").contains("no worktree"), "{error:#}");
}

#[test]
fn a_worktree_that_has_been_deleted_underneath_a_session_is_an_error() {
    let pull_request = pull_request();
    let mut session =
        build_work_session(&request(&pull_request, WorkSource::Whole, None, None, None));
    session.worktree = Some(PathBuf::from("/nonexistent/quinjet-work-session"));

    let error = work_diff(&session).expect_err("a missing checkout cannot be diffed");

    assert!(format!("{error:#}").contains("missing from"), "{error:#}");
}

#[test]
fn the_headline_says_what_the_session_is_for_and_where_it_got_to() {
    let pull_request = pull_request();
    let mut session = build_work_session(&request(
        &pull_request,
        WorkSource::Feedback,
        None,
        None,
        None,
    ));
    session.state = Some(WorkSessionState::Published);

    assert_eq!(
        session.headline(),
        "w42-1 on acme/project#42 from feedback, published"
    );
}
