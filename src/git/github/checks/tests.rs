use super::*;

fn check(link: &str) -> PullRequestCheck {
    PullRequestCheck {
        name: "Format, lint, and test (ubuntu-latest)".to_owned(),
        workflow: "CI".to_owned(),
        state: "SUCCESS".to_owned(),
        status: PullRequestCheckStatus::Passed,
        description: String::new(),
        link: link.to_owned(),
        started_at: "2026-08-14T18:55:20Z".to_owned(),
        completed_at: "2026-08-14T18:55:52Z".to_owned(),
    }
}

#[test]
fn reads_the_job_identity_only_from_an_actions_check_link() {
    assert_eq!(
        check("https://github.com/acme/widget/actions/runs/123/job/456").job_id(),
        Some(456)
    );
    assert_eq!(
        check("https://github.com/acme/widget/actions/runs/123/job/456?pr=7").job_id(),
        Some(456)
    );
    assert_eq!(check("https://ci.example.test/build/9").job_id(), None);
    assert_eq!(check("").job_id(), None);
    assert_eq!(
        check("https://github.com/acme/widget/actions/runs/123/job/456").identity(),
        "456"
    );
    assert_eq!(
        check("https://ci.example.test/build/9").identity(),
        "https://ci.example.test/build/9"
    );
}

fn failing_status() -> std::process::ExitStatus {
    std::process::Command::new("false").status().unwrap()
}

#[test]
fn an_unpublished_log_is_pending_rather_than_a_failure() {
    let mut output = BoundedOutput {
        status: failing_status(),
        stdout: Vec::new(),
        stderr: b"gh: HTTP 404".to_vec(),
        stdout_truncated: false,
    };
    assert!(
        log_not_published(&output),
        "a job that has not finished writing its archive is not an error"
    );

    output.stderr = b"gh: Gone (HTTP 410)".to_vec();
    assert!(
        log_not_published(&output),
        "expired retention is not either"
    );

    output.stderr = b"gh: HTTP 500 Internal Server Error".to_vec();
    assert!(!log_not_published(&output));
}

#[test]
fn strips_timestamps_ansi_and_workflow_commands_from_log_lines() {
    let raw = "\u{feff}2026-08-14T18:59:57.3510133Z Current runner version: '2.336.0'\n\
2026-08-14T18:59:57.3533811Z ##[group]Runner Image Provisioner\n\
2026-08-14T18:59:57.3534599Z \u{1b}[36mHosted Compute Agent\u{1b}[0m\n\
2026-08-14T18:59:57.3539925Z ##[endgroup]\n\
2026-08-14T19:00:09.0000000Z ##[error]cargo test failed\n\
untimestamped trailing output\n";

    let (lines, limit_reached) = parse_check_log(raw.as_bytes());

    assert!(!limit_reached);
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0].text, "Current runner version: '2.336.0'");
    assert_eq!(lines[0].timestamp, "2026-08-14T18:59:57.3510133Z");
    assert_eq!(
        (lines[1].severity, lines[1].text.as_str()),
        (CheckLogSeverity::Command, "Runner Image Provisioner")
    );
    assert_eq!(
        lines[2].text, "Hosted Compute Agent",
        "color codes never reach the renderer"
    );
    assert_eq!(lines[3].text, "");
    assert_eq!(
        (lines[4].severity, lines[4].text.as_str()),
        (CheckLogSeverity::Error, "cargo test failed")
    );
    assert_eq!(lines[5].timestamp, "");
    assert_eq!(lines[5].text, "untimestamped trailing output");
}

#[test]
fn attaches_each_log_line_to_the_step_that_was_running() {
    let mut steps = vec![
        CheckStep {
            number: 1,
            name: "Set up job".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: "2026-08-14T18:00:00Z".to_owned(),
            completed_at: "2026-08-14T18:00:10Z".to_owned(),
            lines: Vec::new(),
        },
        CheckStep {
            number: 2,
            name: "Run cargo test".to_owned(),
            status: PullRequestCheckStatus::Failed,
            conclusion: "failure".to_owned(),
            started_at: "2026-08-14T18:00:10Z".to_owned(),
            completed_at: "2026-08-14T18:02:30Z".to_owned(),
            lines: Vec::new(),
        },
    ];
    let line = |timestamp: &str, text: &str| CheckLogLine {
        timestamp: timestamp.to_owned(),
        text: text.to_owned(),
        severity: CheckLogSeverity::Normal,
    };

    let loose = assign_lines_to_steps(
        &mut steps,
        vec![
            line("2026-08-14T17:59:59Z", "provisioning"),
            line("2026-08-14T18:00:01Z", "setting up"),
            line("2026-08-14T18:00:11Z", "running tests"),
            line("", "continuation of the previous line"),
            line("2026-08-14T18:05:00Z", "teardown"),
        ],
    );

    assert_eq!(
        steps[0]
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>(),
        vec!["setting up"]
    );
    assert_eq!(
        steps[1]
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>(),
        vec!["running tests", "continuation of the previous line"]
    );
    assert_eq!(
        loose.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        vec!["provisioning", "teardown"]
    );
    assert_eq!(steps[1].duration_label(0), "2m 20s");
    assert_eq!(steps[0].duration_label(0), "10s");
}

#[test]
fn a_step_boundary_splits_on_whole_seconds_not_on_text_order() {
    let mut steps = vec![
        CheckStep {
            number: 1,
            name: "Set up job".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: "2026-08-14T18:59:57Z".to_owned(),
            completed_at: "2026-08-14T18:59:58Z".to_owned(),
            lines: Vec::new(),
        },
        CheckStep {
            number: 2,
            name: "Run actions/checkout@v5".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: "2026-08-14T18:59:58Z".to_owned(),
            completed_at: "2026-08-14T19:00:09Z".to_owned(),
            lines: Vec::new(),
        },
    ];
    let line = |timestamp: &str, text: &str| CheckLogLine {
        timestamp: timestamp.to_owned(),
        text: text.to_owned(),
        severity: CheckLogSeverity::Normal,
    };

    let loose = assign_lines_to_steps(
        &mut steps,
        vec![
            line("2026-08-14T18:59:57.3510133Z", "Current runner version"),
            line("2026-08-14T18:59:58.4821004Z", "Run actions/checkout@v5"),
            line("2026-08-14T19:00:08.1200000Z", "Getting Git version info"),
        ],
    );

    assert_eq!(loose, []);
    assert_eq!(
        steps[0]
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Current runner version"]
    );
    assert_eq!(
        steps[1]
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Run actions/checkout@v5", "Getting Git version info"]
    );
}

#[test]
fn steps_without_a_log_keep_every_line_loose() {
    let mut steps = Vec::new();
    let lines = vec![CheckLogLine {
        timestamp: "2026-08-14T18:00:00Z".to_owned(),
        text: "only output".to_owned(),
        severity: CheckLogSeverity::Normal,
    }];

    let loose = assign_lines_to_steps(&mut steps, lines);

    assert_eq!(loose.len(), 1);
}

#[test]
fn parses_job_steps_and_derives_status_from_the_conclusion() {
    let output = b"1\tSet up job\tcompleted\tsuccess\t2026-08-14T18:00:00Z\t2026-08-14T18:00:10Z\n\
3\tRun cargo test\tcompleted\tfailure\t2026-08-14T18:00:10Z\t2026-08-14T18:02:30Z\n\
2\tCheckout\tin_progress\t\t2026-08-14T18:00:10Z\t\n";

    let steps = parse_check_steps(output).unwrap();

    assert_eq!(
        steps.iter().map(|step| step.number).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "steps render in the runner's own order"
    );
    assert_eq!(steps[0].status, PullRequestCheckStatus::Passed);
    assert_eq!(steps[1].status, PullRequestCheckStatus::Pending);
    assert_eq!(steps[2].status, PullRequestCheckStatus::Failed);
    let started = timestamp_seconds("2026-08-14T18:00:10Z").unwrap();
    assert_eq!(steps[1].duration_label(started + 95), "1m 35s…");
    assert_eq!(
        steps[1].duration_label(started),
        "",
        "a step reports nothing until at least a second has passed"
    );
}

#[test]
fn measures_elapsed_time_across_month_and_year_boundaries() {
    assert_eq!(
        elapsed_label("2026-02-28T23:59:30Z", "2026-03-01T00:00:30Z"),
        "1m 0s",
        "February ends on the 28th outside a leap year"
    );
    assert_eq!(
        elapsed_label("2024-02-28T12:00:00Z", "2024-03-01T12:30:00Z"),
        "48h 30m",
        "a leap year adds the extra day between the same two dates"
    );
    assert_eq!(
        elapsed_label("2025-12-31T23:00:00Z", "2026-01-01T01:15:00Z"),
        "2h 15m"
    );
    assert_eq!(elapsed_label("bad", "worse"), "");
    assert_eq!(
        elapsed_label("2026-08-14T18:00:00Z", "2026-08-14T17:00:00Z"),
        "",
        "a completion before its start is reported as unknown, never negative"
    );
}

#[test]
fn parses_live_pull_request_checks_in_stable_name_order() {
    let output = b"tests\tCI\tSUCCESS\tpass\tall good\thttps://example.test/pass\tstart\tdone\nlint\tCI\tFAILURE\tfail\tbroken\thttps://example.test/fail\tstart\tdone\nbuild\tCI\tIN_PROGRESS\tpending\t\thttps://example.test/pending\tstart\t\n";

    let checks = parse_pull_request_checks(output).unwrap();

    assert_eq!(checks.len(), 3);
    assert_eq!(checks[0].name, "build");
    assert_eq!(checks[0].status, PullRequestCheckStatus::Pending);
    assert_eq!(checks[1].name, "lint");
    assert_eq!(checks[1].status, PullRequestCheckStatus::Failed);
    assert_eq!(checks[2].name, "tests");
    assert_eq!(checks[2].status, PullRequestCheckStatus::Passed);
    assert_eq!(checks[2].description, "all good");
}

#[test]
fn a_warm_up_stops_as_soon_as_the_pull_request_it_serves_is_left() {
    let settled = |name: &str| PullRequestCheck {
        name: name.to_owned(),
        workflow: "CI".to_owned(),
        state: "SUCCESS".to_owned(),
        status: PullRequestCheckStatus::Passed,
        description: String::new(),
        link: "https://github.com/o/r/actions/runs/1/job/2".to_owned(),
        started_at: String::new(),
        completed_at: String::new(),
    };
    let repository = Repository {
        root: std::path::PathBuf::from("/nonexistent-on-purpose"),
    };
    let checks = [settled("one"), settled("two"), settled("three")];

    let warmed = repository.prefetch_check_run_logs(&PullRequest::default(), &checks, &|| false);
    assert_eq!(warmed, 0, "a superseded warm-up asks for nothing");
}
