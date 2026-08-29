use super::github::GitHubFixture;
use super::*;

#[doc = " Fake GitHub CLI cases for the Actions verbs: one failed run, one still"]
#[doc = " going and holding a deployment, and one that passed and uploaded an"]
#[doc = " artifact."]
pub(super) const GH_CASES: &str = r#"  *"actions/runs?head_sha="*)
    printf '7701\tCI\tcompleted\tfailure\thttps://github.com/acme/project/actions/runs/7701\t1\tpull_request\n7702\tDeploy\tin_progress\t\thttps://github.com/acme/project/actions/runs/7702\t1\tpull_request\n7703\tDocs\tcompleted\tsuccess\thttps://github.com/acme/project/actions/runs/7703\t1\tpull_request\n'
    ;;
  *"actions/runs/7701/artifacts"*)
    printf '9001\tsnapshots\t2048\tfalse\t2026-09-20T00:00:00Z\t2026-08-21T01:00:00Z\t7701\thttps://example.test/9001\n9002\told-logs\t512\ttrue\t2026-08-01T00:00:00Z\t2026-07-01T01:00:00Z\t7701\thttps://example.test/9002\n'
    ;;
  *"actions/runs/7703/artifacts"*)
    printf '9003\tcoverage\t5242880\tfalse\t2026-09-20T00:00:00Z\t2026-08-21T01:00:00Z\t7703\thttps://example.test/9003\n'
    ;;
  *"actions/runs/7702/artifacts"*)
    ;;
  *"actions/runs/7702/pending_deployments"*)
    printf '55\tstaging\t0\ttrue\toctocat\n'
    ;;
  *"deployments?sha="*)
    printf '4100\tpreview\tPreview build\t2026-08-21T02:30:00Z\thttps://api.github.com/deployments/4100\ttrue\n'
    ;;
  *"actions/artifacts/9001/zip"*)
    printf 'PK\\003\\004fake-artifact-archive'
    ;;
  *"--method POST"*)
    ;;
"#;

#[test]
fn workflow_runs_list_with_their_state() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "checks", "runs", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    for expected in ["failed", "running", "passed", "run 7701", "Deploy"] {
        ensure!(
            plain.stdout.contains(expected),
            "missing {expected}: {}",
            plain.stdout
        );
    }

    let json = fixture
        .read(&["pr", "checks", "runs", "42", "--json"])?
        .json()?;
    ensure!(json["headOid"] == fixture.head_oid);
    ensure!(json["runs"].as_array().is_some_and(|runs| runs.len() == 3));
    ensure!(json["runs"][0]["id"] == 7703);
    ensure!(json["runs"][0]["state"] == "passed");
    ensure!(json["runs"][2]["state"] == "failed");
    Ok(())
}

#[test]
fn rerun_previews_the_failed_runs_and_changes_nothing_without_yes() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let preview = fixture
        .read(&["pr", "checks", "rerun", "42", "--failed"])?
        .success()?;

    ensure!(
        preview.stdout == "Would rerun the failed jobs of `CI` (run 7701). Pass --yes to do it.\n",
        "{}",
        preview.stdout
    );
    ensure!(
        !fixture.gh_calls()?.contains("--method"),
        "a preview called a mutation"
    );
    Ok(())
}

#[test]
fn rerun_failed_jobs_posts_once_per_failed_run() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture
        .read(&["pr", "checks", "rerun", "42", "--failed", "--yes"])?
        .success()?;

    ensure!(
        run.stdout == "Reran the failed jobs of `CI` (run 7701)\n",
        "{}",
        run.stdout
    );
    let calls = fixture.gh_calls()?;
    ensure!(
        calls.contains("repos/acme/project/actions/runs/7701/rerun-failed-jobs"),
        "{calls}"
    );
    ensure!(
        !calls.contains("runs/7703/rerun"),
        "a passing run was rerun: {calls}"
    );
    Ok(())
}

#[test]
fn rerun_all_reruns_whole_runs_instead_of_their_failed_jobs() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    fixture
        .read(&["pr", "checks", "rerun", "42", "--all", "--yes"])?
        .success()?;

    let calls = fixture.gh_calls()?;
    ensure!(calls.contains("actions/runs/7701/rerun\n"), "{calls}");
    ensure!(!calls.contains("rerun-failed-jobs"), "{calls}");
    Ok(())
}

#[test]
fn rerun_by_check_name_reruns_that_one_actions_job() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let preview = fixture
        .read(&["pr", "checks", "rerun", "42", "--check", "Unit"])?
        .success()?;
    ensure!(
        preview.stdout == "Would rerun `Unit tests`. Pass --yes to do it.\n",
        "{}",
        preview.stdout
    );

    let run = fixture
        .read(&["pr", "checks", "rerun", "42", "--check", "Unit", "--yes"])?
        .success()?;
    ensure!(run.stdout == "Reran `Unit tests`\n", "{}", run.stdout);
    ensure!(
        fixture.gh_calls()?.contains("actions/jobs/123/rerun"),
        "{}",
        fixture.gh_calls()?
    );
    Ok(())
}

#[test]
fn rerunning_an_unknown_or_unrerunnable_check_is_named_rather_than_guessed() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let unknown = fixture.read(&["pr", "checks", "rerun", "42", "--check", "nothing"])?;
    ensure!(unknown.code == 3, "expected 3, got {}", unknown.code);
    ensure!(
        unknown.stderr.contains("no check on this pull request"),
        "{}",
        unknown.stderr
    );
    Ok(())
}

#[test]
fn cancel_acts_only_on_runs_that_have_not_settled() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let preview = fixture.read(&["pr", "checks", "cancel", "42"])?.success()?;
    ensure!(
        preview.stdout == "Would cancel `Deploy` (run 7702). Pass --yes to do it.\n",
        "{}",
        preview.stdout
    );

    let run = fixture
        .read(&["pr", "checks", "cancel", "42", "--yes"])?
        .success()?;
    ensure!(
        run.stdout == "Cancelled `Deploy` (run 7702)\n",
        "{}",
        run.stdout
    );
    let calls = fixture.gh_calls()?;
    ensure!(calls.contains("actions/runs/7702/cancel"), "{calls}");
    ensure!(!calls.contains("actions/runs/7701/cancel"), "{calls}");
    Ok(())
}

#[test]
fn artifacts_list_across_runs_with_their_size_and_expiry() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "artifacts", "42"])?.success()?;
    for expected in [
        "ready",
        "expired",
        "2 KiB",
        "5 MiB",
        "snapshots",
        "coverage",
        "old-logs",
    ] {
        ensure!(
            plain.stdout.contains(expected),
            "missing {expected}: {}",
            plain.stdout
        );
    }

    let json = fixture.read(&["pr", "artifacts", "42", "--json"])?.json()?;
    ensure!(
        json["artifacts"]
            .as_array()
            .is_some_and(|artifacts| artifacts.len() == 3)
    );
    ensure!(json["artifacts"][0]["name"] == "coverage");
    ensure!(json["artifacts"][0]["sizeInBytes"] == 5_242_880);
    ensure!(json["artifacts"][0]["workflow"] == "Docs");
    ensure!(json["artifacts"][1]["name"] == "old-logs");
    ensure!(json["artifacts"][1]["expired"] == true);
    ensure!(json["artifacts"][2]["name"] == "snapshots");
    Ok(())
}

#[test]
fn downloading_an_artifact_writes_one_archive_where_it_was_asked_to() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let into = fixture.repository.environment.join("downloads");

    let run = fixture.run(&[
        "pr",
        "artifacts",
        "download",
        "42",
        "snapshots",
        "--into",
        &into.display().to_string(),
    ])?;
    ensure!(
        run.code == 0,
        "expected 0, got {}: {}",
        run.code,
        run.stderr
    );

    let archive = into.join("snapshots.zip");
    ensure!(archive.is_file(), "the archive was not written");
    ensure!(
        fs::read(&archive)?.starts_with(b"PK"),
        "the archive is not a zip"
    );
    ensure!(
        run.stdout.trim().ends_with("snapshots.zip"),
        "{}",
        run.stdout
    );
    ensure!(
        !into.join("snapshots.zip.part").exists(),
        "the staging file was left behind"
    );
    Ok(())
}

#[test]
fn downloading_an_expired_or_unknown_artifact_is_refused() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let into = fixture.repository.environment.join("downloads");

    let expired = fixture.run(&[
        "pr",
        "artifacts",
        "download",
        "42",
        "old-logs",
        "--into",
        &into.display().to_string(),
    ])?;
    ensure!(expired.code == 1, "expected 1, got {}", expired.code);
    ensure!(expired.stderr.contains("has expired"), "{}", expired.stderr);

    let unknown = fixture.run(&["pr", "artifacts", "download", "42", "nothing"])?;
    ensure!(unknown.code == 3, "expected 3, got {}", unknown.code);
    ensure!(
        unknown.stderr.contains("no artifact on this pull request"),
        "{}",
        unknown.stderr
    );
    ensure!(
        unknown.stderr.contains("coverage, old-logs, snapshots"),
        "{}",
        unknown.stderr
    );
    Ok(())
}

#[test]
fn deployments_separate_what_is_waiting_from_what_is_deployed() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "deployments", "42"])?.success()?;
    ensure!(
        plain.stdout.contains("Waiting for approval"),
        "{}",
        plain.stdout
    );
    ensure!(plain.stdout.contains("staging"), "{}", plain.stdout);
    ensure!(
        plain.stdout.contains("reviewers octocat"),
        "{}",
        plain.stdout
    );
    ensure!(plain.stdout.contains("Deployed"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("preview"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("transient"), "{}", plain.stdout);

    let json = fixture
        .read(&["pr", "deployments", "42", "--json"])?
        .json()?;
    ensure!(json["pending"][0]["environment"] == "staging");
    ensure!(json["pending"][0]["runId"] == 7702);
    ensure!(json["pending"][0]["environmentId"] == 55);
    ensure!(json["pending"][0]["viewerCanApprove"] == true);
    ensure!(json["deployments"][0]["environment"] == "preview");
    ensure!(json["deployments"][0]["transient"] == true);
    Ok(())
}

#[test]
fn approving_a_deployment_previews_first_and_then_posts_the_decision() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let preview = fixture
        .read(&["pr", "deployments", "approve", "42", "staging"])?
        .success()?;
    ensure!(
        preview.stdout == "Would approve `staging` for run 7702. Pass --yes to do it.\n",
        "{}",
        preview.stdout
    );
    ensure!(!fixture.gh_calls()?.contains("--method POST"));

    let run = fixture
        .read(&[
            "pr",
            "deployments",
            "approve",
            "42",
            "staging",
            "--comment",
            "shipping it",
            "--yes",
        ])?
        .success()?;
    ensure!(
        run.stdout == "Approved `staging` for run 7702\n",
        "{}",
        run.stdout
    );
    let calls = fixture.gh_calls()?;
    ensure!(
        calls.contains("actions/runs/7702/pending_deployments"),
        "{calls}"
    );
    ensure!(calls.contains("\"state\":\"approved\""), "{calls}");
    ensure!(calls.contains("\"environment_ids\":[55]"), "{calls}");
    ensure!(calls.contains("shipping it"), "{calls}");
    Ok(())
}

#[test]
fn rejecting_a_deployment_sends_the_other_state() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture
        .read(&["pr", "deployments", "reject", "42", "staging", "--yes"])?
        .success()?;

    ensure!(
        run.stdout == "Rejected `staging` for run 7702\n",
        "{}",
        run.stdout
    );
    ensure!(
        fixture.gh_calls()?.contains("\"state\":\"rejected\""),
        "{}",
        fixture.gh_calls()?
    );
    Ok(())
}

#[test]
fn reviewing_an_environment_nothing_is_waiting_on_says_which_ones_are() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture
        .read(&["pr", "deployments", "approve", "42", "production", "--yes"])?
        .success()?;

    ensure!(
        run.stdout == "Nothing to act on: no run is waiting on `production`\n",
        "{}",
        run.stdout
    );
    ensure!(
        run.stderr.contains("the waiting environments are: staging"),
        "{}",
        run.stderr
    );
    ensure!(!fixture.gh_calls()?.contains("--method POST"));
    Ok(())
}

#[test]
fn the_actions_verbs_require_a_number_before_they_read_anything() -> Result<()> {
    for verb in [
        vec!["pr", "artifacts"],
        vec!["pr", "deployments"],
        vec!["pr", "checks"],
    ] {
        let run = run_in(None, &verb)?;
        ensure!(run.code == 2, "{verb:?} expected 2, got {}", run.code);
        ensure!(run.stderr.contains("<NUMBER>"), "{verb:?}: {}", run.stderr);
    }
    Ok(())
}
