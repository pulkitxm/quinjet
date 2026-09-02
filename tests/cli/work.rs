use super::github::GitHubFixture;
use super::*;

#[doc = " A work session's whole point is that a coding process works inside it"]
#[doc = " rather than in the checkout the reviewer is using, so every test here"]
#[doc = " asks its own worktree for."]
pub(super) fn worktree(fixture: &GitHubFixture) -> PathBuf {
    fixture.repository.environment.join("work")
}

pub(super) fn start(fixture: &GitHubFixture, from: &str) -> Result<Run> {
    let into = worktree(fixture);
    let into = into.to_string_lossy().into_owned();
    fixture.run(&[
        "work", "start", "--pr", "42", "--from", from, "--into", &into,
    ])
}

#[test]
fn a_session_starts_at_the_pull_requests_exact_head_in_its_own_checkout() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = start(&fixture, "whole")?.success()?;

    ensure!(run.stdout.contains("w42-1"), "{}", run.stdout);
    ensure!(run.stdout.contains("quinjet/work/w42-1"), "{}", run.stdout);
    let path = worktree(&fixture);
    ensure!(path.is_dir(), "the worktree was not created");
    let head = Scratch::git_in(&path, &["rev-parse", "HEAD"])?;
    ensure!(
        head == fixture.head_oid,
        "the session started at {head}, not at the pull request head"
    );
    let branch = Scratch::git_in(&path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    ensure!(branch == "quinjet/work/w42-1", "{branch}");
    Ok(())
}

#[test]
fn a_session_states_the_operations_it_may_not_perform() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = start(&fixture, "whole")?.success()?;

    ensure!(
        run.stdout.contains("this session may not"),
        "{}",
        run.stdout
    );
    for forbidden in ["push", "comment on the pull request", "resolve", "merge"] {
        ensure!(
            run.stdout.contains(forbidden),
            "nothing forbids `{forbidden}` in:\n{}",
            run.stdout
        );
    }
    Ok(())
}

#[test]
fn the_session_json_records_the_boundary_and_the_starting_commit() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let into = worktree(&fixture);
    let into = into.to_string_lossy().into_owned();

    let value = fixture
        .run(&[
            "work", "start", "--pr", "42", "--from", "whole", "--into", &into, "--json",
        ])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["id"] == "w42-1");
    ensure!(value["repository"] == "acme/project");
    ensure!(value["number"] == 42);
    ensure!(value["source"] == "whole");
    ensure!(value["state"] == "open");
    ensure!(value["startOid"] == fixture.head_oid.as_str());
    ensure!(value["branch"] == "quinjet/work/w42-1");
    ensure!(
        value["forbidden"]
            .as_array()
            .is_some_and(|entries| entries.len() == 4),
        "{value}"
    );
    Ok(())
}

#[test]
fn a_feedback_session_carries_the_threads_and_what_resolves_each_one() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let into = worktree(&fixture);
    let into = into.to_string_lossy().into_owned();

    let value = fixture
        .run(&[
            "work", "start", "--pr", "42", "--from", "feedback", "--into", &into, "--json",
        ])?
        .success()?
        .json()?;

    ensure!(value["source"] == "feedback");
    let tasks = value["tasks"].as_array().context("tasks is an array")?;
    ensure!(!tasks.is_empty(), "{value}");
    ensure!(tasks.iter().any(|task| task["id"] == "THREAD_1"), "{value}");
    ensure!(
        tasks.iter().all(|task| task["resolvedBy"]
            .as_str()
            .is_some_and(|action| action.contains("quinjet "))),
        "every task names the Quinjet operation that resolves it: {value}"
    );
    ensure!(
        tasks.iter().all(|task| task["resolvedBy"]
            .as_str()
            .is_some_and(|action| !action.contains("git push"))),
        "{value}"
    );
    Ok(())
}

#[test]
fn a_failed_check_session_names_the_check_and_the_log_that_explains_it() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let into = worktree(&fixture);
    let into = into.to_string_lossy().into_owned();

    let value = fixture
        .run(&[
            "work",
            "start",
            "--pr",
            "42",
            "--from",
            "failed-checks",
            "--into",
            &into,
            "--json",
        ])?
        .success()?
        .json()?;

    ensure!(value["source"] == "failed-checks");
    let tasks = value["tasks"].as_array().context("tasks is an array")?;
    ensure!(
        tasks
            .iter()
            .all(|task| task["kind"] == "check" || task["kind"] == "annotation"),
        "{value}"
    );
    Ok(())
}

#[test]
fn sessions_list_with_their_state_and_what_they_were_started_for() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let plain = fixture.read(&["work", "list"])?.success()?;
    ensure!(plain.stdout.contains("w42-1"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("open"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("1 session(s)"), "{}", plain.stdout);

    let value = fixture.read(&["work", "list", "--json"])?.json()?;
    ensure!(value["sessions"][0]["id"] == "w42-1");
    Ok(())
}

#[test]
fn a_sessions_diff_is_measured_from_the_commit_it_started_at() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);
    let path = worktree(&fixture);
    fs::write(path.join("feature.txt"), "reworked by the session\n")?;

    let plain = fixture.read(&["work", "diff", "w42-1"])?.success()?;

    ensure!(plain.stdout.contains("feature.txt"), "{}", plain.stdout);
    ensure!(
        plain.stdout.contains("+reworked by the session"),
        "{}",
        plain.stdout
    );
    let value = fixture.read(&["work", "diff", "w42-1", "--json"])?.json()?;
    ensure!(value["startOid"] == fixture.head_oid.as_str());
    ensure!(value["files"][0] == "feature.txt");
    Ok(())
}

#[test]
fn a_session_that_has_changed_nothing_says_so_rather_than_printing_an_empty_patch() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let run = fixture.read(&["work", "diff", "w42-1"])?.success()?;

    ensure!(run.stdout.contains("has changed nothing"), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_verification_runs_inside_the_worktree_and_its_result_is_kept() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let passing = fixture
        .read(&["work", "verify", "w42-1", "--", "true"])?
        .success()?;
    ensure!(
        passing.stdout.contains("passed  true"),
        "{}",
        passing.stdout
    );

    let value = fixture
        .read(&["work", "inspect", "w42-1", "--json"])?
        .json()?;
    ensure!(value["verifications"][0]["command"][0] == "true");
    ensure!(value["verifications"][0]["passed"] == true);
    ensure!(value["verifications"][0]["exitCode"] == 0);
    Ok(())
}

#[test]
fn a_failing_verification_is_recorded_with_its_output_and_can_set_the_exit_code() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let run = fixture.run(&["work", "verify", "w42-1", "--exit-code", "--", "false"])?;

    ensure!(run.code == 1, "expected 1, got {}", run.code);
    ensure!(run.stdout.contains("failed  false"), "{}", run.stdout);
    let value = fixture
        .read(&["work", "inspect", "w42-1", "--json"])?
        .json()?;
    ensure!(value["verifications"][0]["passed"] == false);
    ensure!(value["verifications"][0]["exitCode"] == 1);
    Ok(())
}

#[test]
fn re_running_the_verifications_replays_exactly_what_was_recorded() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);
    drop(
        fixture
            .run(&["work", "verify", "w42-1", "--", "false"])?
            .success()?,
    );

    let replay = fixture.run(&["work", "verify", "w42-1"])?.success()?;

    ensure!(replay.stdout.contains("failed  false"), "{}", replay.stdout);
    let value = fixture
        .read(&["work", "inspect", "w42-1", "--json"])?
        .json()?;
    ensure!(
        value["verifications"]
            .as_array()
            .is_some_and(|runs| runs.len() == 1),
        "a replay stacked a second row: {value}"
    );
    Ok(())
}

#[test]
fn a_session_with_nothing_recorded_refuses_to_report_a_pass_it_did_not_earn() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let run = fixture.run(&["work", "verify", "w42-1"])?;

    ensure!(run.code == 1, "expected 1, got {}", run.code);
    ensure!(run.stderr.contains("no recorded command"), "{}", run.stderr);
    Ok(())
}
