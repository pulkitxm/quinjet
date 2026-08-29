use super::*;

fn run(id: u64, name: &str, status: &str, conclusion: &str) -> WorkflowRun {
    WorkflowRun {
        id,
        name: name.to_owned(),
        state: WorkflowRunState::parse(status, conclusion),
        status: status.to_owned(),
        conclusion: conclusion.to_owned(),
        url: format!("https://example.test/{id}"),
        attempt: 1,
        event: "pull_request".to_owned(),
    }
}

fn artifact(id: u64, name: &str, size: u64, expired: bool) -> WorkflowArtifact {
    WorkflowArtifact {
        id,
        name: name.to_owned(),
        size_in_bytes: size,
        expired,
        expires_at: String::new(),
        created_at: String::new(),
        run_id: 1,
        workflow: "CI".to_owned(),
        download_url: String::new(),
    }
}

fn pending(run_id: u64, environment: &str, environment_id: u64) -> PendingDeployment {
    PendingDeployment {
        run_id,
        workflow: "Deploy".to_owned(),
        environment: environment.to_owned(),
        environment_id,
        wait_timer: 0,
        viewer_can_approve: true,
        reviewers: vec!["octocat".to_owned()],
    }
}

#[test]
fn a_run_state_is_read_from_its_status_before_its_conclusion() {
    let cases = [
        ("queued", "", WorkflowRunState::Queued),
        ("waiting", "", WorkflowRunState::Queued),
        ("in_progress", "", WorkflowRunState::Running),
        ("in_progress", "success", WorkflowRunState::Running),
        ("completed", "success", WorkflowRunState::Passed),
        ("completed", "failure", WorkflowRunState::Failed),
        ("completed", "timed_out", WorkflowRunState::Failed),
        ("completed", "action_required", WorkflowRunState::Failed),
        ("completed", "cancelled", WorkflowRunState::Cancelled),
        ("completed", "skipped", WorkflowRunState::Skipped),
        ("completed", "something-new", WorkflowRunState::Unknown),
    ];
    for (status, conclusion, expected) in cases {
        assert_eq!(
            WorkflowRunState::parse(status, conclusion),
            expected,
            "{status}/{conclusion}"
        );
    }
}

#[test]
fn only_unsettled_runs_can_be_cancelled_and_only_unhappy_ones_rerun() {
    let listing = PullRequestWorkflowRuns {
        head_oid: "a".repeat(40),
        runs: vec![
            run(1, "CI", "completed", "failure"),
            run(2, "CI", "in_progress", ""),
            run(3, "CI", "completed", "success"),
            run(4, "CI", "queued", ""),
            run(5, "CI", "completed", "cancelled"),
        ],
        truncated: false,
        from_cache: false,
    };

    let active: Vec<u64> = listing.active().map(|run| run.id).collect();
    let failed: Vec<u64> = listing.failed().map(|run| run.id).collect();

    assert_eq!(active, vec![2, 4]);
    assert_eq!(failed, vec![1, 5]);
}

#[test]
fn an_empty_operation_previews_its_reason_rather_than_promising_an_action() {
    let rerun = WorkflowOperation::RerunFailedJobs { runs: Vec::new() };
    assert!(rerun.is_empty());
    assert_eq!(
        rerun.preview_message(),
        "Nothing to act on: no workflow run on this pull request has failed"
    );

    let cancel = WorkflowOperation::CancelRuns { runs: Vec::new() };
    assert_eq!(
        cancel.preview_message(),
        "Nothing to act on: no workflow run on this pull request is still going"
    );

    let review = WorkflowOperation::ReviewDeployments {
        environment: "staging".to_owned(),
        approve: true,
        comment: String::new(),
        pending: Vec::new(),
    };
    assert_eq!(
        review.preview_message(),
        "Nothing to act on: no run is waiting on `staging`"
    );
}

#[test]
fn a_preview_names_every_run_it_would_act_on() {
    let operation = WorkflowOperation::RerunFailedJobs {
        runs: vec![
            run(11, "CI", "completed", "failure"),
            run(12, "Lint", "completed", "failure"),
        ],
    };

    assert!(!operation.is_empty());
    assert_eq!(
        operation.preview_message(),
        "Would rerun the failed jobs of `CI` (run 11), `Lint` (run 12). Pass --yes to do it."
    );
    assert_eq!(
        operation.success_message(),
        "Reran the failed jobs of `CI` (run 11), `Lint` (run 12)"
    );
}

#[test]
fn a_run_without_a_workflow_name_is_still_named_by_its_id() {
    let operation = WorkflowOperation::CancelRuns {
        runs: vec![run(7, "", "in_progress", "")],
    };

    assert_eq!(
        operation.preview_message(),
        "Would cancel run 7. Pass --yes to do it."
    );
}

#[test]
fn a_single_job_rerun_names_the_check_the_caller_gave() {
    let operation = WorkflowOperation::RerunJob {
        check: "windows / test".to_owned(),
        job_id: 42,
    };

    assert!(!operation.is_empty());
    assert_eq!(
        operation.preview_message(),
        "Would rerun `windows / test`. Pass --yes to do it."
    );
    assert_eq!(operation.success_message(), "Reran `windows / test`");
}

#[test]
fn a_deployment_review_names_the_environment_and_the_runs_it_holds() {
    let approve = WorkflowOperation::ReviewDeployments {
        environment: "staging".to_owned(),
        approve: true,
        comment: "shipping".to_owned(),
        pending: vec![pending(3, "staging", 9), pending(4, "staging", 9)],
    };

    assert_eq!(
        approve.preview_message(),
        "Would approve `staging` for run 3, `staging` for run 4. Pass --yes to do it."
    );
    assert_eq!(approve.label(), "Approving deployments");

    let reject = WorkflowOperation::ReviewDeployments {
        environment: "staging".to_owned(),
        approve: false,
        comment: String::new(),
        pending: vec![pending(3, "staging", 9)],
    };
    assert_eq!(reject.label(), "Rejecting deployments");
    assert_eq!(reject.success_message(), "Rejected `staging` for run 3");
}

#[test]
fn an_artifact_name_that_could_escape_its_directory_is_refused() {
    for name in [
        "../escape",
        "nested/path",
        "windows\\path",
        "drive:name",
        "",
        "   ",
        ".",
        "..",
        "-flag",
    ] {
        drop(
            artifact(1, name, 10, false)
                .safe_file_name()
                .expect_err(&format!("`{name}` must be refused")),
        );
    }
    drop(
        artifact(1, "snapshots\u{7}", 10, false)
            .safe_file_name()
            .expect_err("a control character is refused"),
    );
    assert_eq!(
        artifact(1, "snapshots", 10, false)
            .safe_file_name()
            .expect("a plain name is accepted"),
        "snapshots.zip"
    );
    assert_eq!(
        artifact(1, "  spaced name  ", 10, false)
            .safe_file_name()
            .expect("surrounding space is trimmed"),
        "spaced name.zip"
    );
}

#[test]
fn an_artifact_size_reads_in_whole_units() {
    let cases = [
        (0_u64, "0 B"),
        (512, "512 B"),
        (2048, "2 KiB"),
        (5 * 1024 * 1024, "5 MiB"),
        (3 * 1024 * 1024 * 1024, "3 GiB"),
    ];
    for (bytes, expected) in cases {
        assert_eq!(artifact(1, "a", bytes, false).size_label(), expected);
    }
}

#[test]
fn an_artifact_is_selected_by_name_or_by_a_unique_part_of_one() {
    let listing = PullRequestArtifacts {
        head_oid: "a".repeat(40),
        artifacts: vec![
            artifact(1, "snapshots", 10, false),
            artifact(2, "snapshots-windows", 20, false),
            artifact(3, "coverage", 30, false),
        ],
        truncated: false,
        warnings: Vec::new(),
    };

    assert_eq!(listing.select("snapshots").expect("exact match").id, 1);
    assert_eq!(listing.select("COVER").expect("unique part").id, 3);
    assert_eq!(
        listing.select("windows").expect("a unique part matches").id,
        2
    );
    drop(
        listing
            .select("snap")
            .expect_err("an ambiguous part is refused"),
    );
    drop(
        listing
            .select("nothing")
            .expect_err("an unknown name is refused"),
    );
    assert_eq!(listing.names(), "snapshots, snapshots-windows, coverage");
}

#[test]
fn pending_deployments_are_matched_by_environment_regardless_of_case_or_space() {
    let listing = PullRequestDeployments {
        head_oid: "a".repeat(40),
        pending: vec![
            pending(3, "staging", 9),
            pending(4, "staging", 9),
            pending(5, "production", 10),
        ],
        deployments: Vec::new(),
        warnings: Vec::new(),
    };

    assert_eq!(listing.pending_for("  STAGING ").len(), 2);
    assert_eq!(listing.pending_for("production").len(), 1);
    assert_eq!(listing.pending_for("preview").len(), 0);
    assert_eq!(listing.environments(), "production, staging");
}
