use super::*;
use crate::git::github::{
    DeploymentRecord, PendingDeployment, WorkflowArtifact, WorkflowRun, WorkflowRunState,
};

fn run(id: u64, name: &str, state: WorkflowRunState) -> WorkflowRun {
    WorkflowRun {
        id,
        name: name.to_owned(),
        state,
        status: String::new(),
        conclusion: String::new(),
        url: String::new(),
        attempt: 1,
        event: "pull_request".to_owned(),
    }
}

#[test]
fn a_run_listing_leads_with_state_and_ends_with_the_run_id() {
    let listing = PullRequestWorkflowRuns {
        head_oid: "a".repeat(40),
        runs: vec![
            run(7701, "CI", WorkflowRunState::Failed),
            run(7702, "Deploy", WorkflowRunState::Running),
        ],
        truncated: true,
        from_cache: false,
    };

    let text = workflow_runs(&listing);
    let lines: Vec<&str> = text.lines().collect();

    assert!(
        lines[0].starts_with("failed     pull_request"),
        "{}",
        lines[0]
    );
    assert!(lines[0].ends_with("run 7701"), "{}", lines[0]);
    assert!(
        lines[1].starts_with("running    pull_request"),
        "{}",
        lines[1]
    );
    assert!(text.contains("reached Quinjet's size cap"), "{text}");
}

#[test]
fn an_empty_run_listing_says_so() {
    assert_eq!(
        workflow_runs(&PullRequestWorkflowRuns::default()),
        "No workflow runs reported\n"
    );
}

fn artifact(name: &str, size: u64, expired: bool) -> WorkflowArtifact {
    WorkflowArtifact {
        id: 1,
        name: name.to_owned(),
        size_in_bytes: size,
        expired,
        expires_at: String::new(),
        created_at: String::new(),
        run_id: 7701,
        workflow: "CI".to_owned(),
        download_url: String::new(),
    }
}

#[test]
fn an_artifact_listing_marks_what_has_expired_and_says_how_large_it_is() {
    let listing = PullRequestArtifacts {
        head_oid: "a".repeat(40),
        artifacts: vec![
            artifact("snapshots", 2048, false),
            artifact("old-logs", 512, true),
        ],
        truncated: false,
        warnings: vec!["one run could not be read".to_owned()],
    };

    let text = artifacts(&listing);

    assert!(text.contains("ready         2 KiB  snapshots"), "{text}");
    assert!(text.contains("expired       512 B  old-logs"), "{text}");
    assert!(text.contains("note  one run could not be read"), "{text}");
}

#[test]
fn an_empty_artifact_listing_says_so() {
    assert_eq!(
        artifacts(&PullRequestArtifacts::default()),
        "No artifacts reported\n"
    );
}

#[test]
fn a_deployment_listing_separates_waiting_from_deployed() {
    let listing = PullRequestDeployments {
        head_oid: "a".repeat(40),
        pending: vec![
            PendingDeployment {
                run_id: 7702,
                workflow: "Deploy".to_owned(),
                environment: "staging".to_owned(),
                environment_id: 55,
                wait_timer: 0,
                viewer_can_approve: true,
                reviewers: vec!["octocat".to_owned()],
            },
            PendingDeployment {
                run_id: 7702,
                workflow: "Deploy".to_owned(),
                environment: "production".to_owned(),
                environment_id: 56,
                wait_timer: 30,
                viewer_can_approve: false,
                reviewers: Vec::new(),
            },
        ],
        deployments: vec![DeploymentRecord {
            id: 4100,
            environment: "preview".to_owned(),
            description: "Preview build".to_owned(),
            created_at: "2026-08-21T02:30:00Z".to_owned(),
            url: String::new(),
            transient: true,
        }],
        warnings: Vec::new(),
    };

    let text = deployments(&listing);

    assert!(text.starts_with("Waiting for approval\n"), "{text}");
    assert!(text.contains("staging"), "{text}");
    assert!(text.contains("reviewers octocat"), "{text}");
    assert!(text.contains("(you cannot review this)"), "{text}");
    assert!(text.contains("\nDeployed\n"), "{text}");
    assert!(text.contains("preview"), "{text}");
    assert!(text.contains("transient"), "{text}");
}

#[test]
fn an_empty_deployment_listing_says_so() {
    assert_eq!(
        deployments(&PullRequestDeployments::default()),
        "No deployments reported\n"
    );
}

#[test]
fn a_deployment_listing_with_nothing_waiting_omits_that_heading() {
    let listing = PullRequestDeployments {
        head_oid: "a".repeat(40),
        pending: Vec::new(),
        deployments: vec![DeploymentRecord {
            id: 4100,
            environment: "preview".to_owned(),
            description: String::new(),
            created_at: String::new(),
            url: String::new(),
            transient: false,
        }],
        warnings: Vec::new(),
    };

    let text = deployments(&listing);

    assert!(text.starts_with("Deployed\n"), "{text}");
    assert!(!text.contains("Waiting"), "{text}");
    assert!(!text.contains("transient"), "{text}");
}
