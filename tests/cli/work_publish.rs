use super::github::GitHubFixture;
use super::work::{start, worktree};
use super::*;

#[test]
fn publishing_previews_first_and_names_the_steps_it_will_not_take() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);
    let path = worktree(&fixture);
    fs::write(path.join("feature.txt"), "reworked by the session\n")?;
    let before = Scratch::git_in(&path, &["rev-parse", "HEAD"])?;

    let preview = fixture.read(&["work", "publish", "w42-1"])?.success()?;

    ensure!(
        preview.stdout.contains("Would commit 1 file(s)"),
        "{}",
        preview.stdout
    );
    ensure!(
        preview
            .stdout
            .contains("publishing writes one local commit and nothing else"),
        "{}",
        preview.stdout
    );
    ensure!(
        preview
            .stdout
            .contains("git push origin quinjet/work/w42-1"),
        "{}",
        preview.stdout
    );
    ensure!(preview.stdout.contains("Pass --yes"), "{}", preview.stdout);
    let after = Scratch::git_in(&path, &["rev-parse", "HEAD"])?;
    ensure!(after == before, "the preview committed something");
    Ok(())
}

#[test]
fn publishing_records_one_commit_on_the_session_branch_and_pushes_nothing() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);
    let path = worktree(&fixture);
    fs::write(path.join("feature.txt"), "reworked by the session\n")?;
    fs::write(path.join("added.txt"), "a file the session added\n")?;
    let before = Scratch::git_in(&path, &["rev-parse", "HEAD"])?;
    fixture.clear_gh_calls();

    let run = fixture
        .run(&[
            "work",
            "publish",
            "w42-1",
            "-m",
            "fix: address review",
            "--yes",
        ])?
        .success()?;

    ensure!(run.stdout.contains("published"), "{}", run.stdout);
    let after = Scratch::git_in(&path, &["rev-parse", "HEAD"])?;
    ensure!(after != before, "nothing was committed");
    ensure!(
        Scratch::git_in(&path, &["log", "-1", "--format=%s"])? == "fix: address review",
        "the message was not used"
    );
    ensure!(
        Scratch::git_in(&path, &["show", "--name-only", "--format=", "HEAD"])?
            .contains("added.txt"),
        "the untracked file was left out of the commit"
    );
    ensure!(
        !fixture.gh_calls_or_none().contains("api"),
        "publishing reached GitHub"
    );
    Ok(())
}

#[test]
fn publishing_a_session_that_changed_nothing_writes_no_commit() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);
    let path = worktree(&fixture);
    let before = Scratch::git_in(&path, &["rev-parse", "HEAD"])?;

    let run = fixture
        .run(&["work", "publish", "w42-1", "--yes"])?
        .success()?;

    ensure!(run.stdout.contains("Nothing to publish"), "{}", run.stdout);
    let after = Scratch::git_in(&path, &["rev-parse", "HEAD"])?;
    ensure!(after == before, "an empty session still committed");
    Ok(())
}

#[test]
fn publishing_says_when_nothing_has_been_verified() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);
    fs::write(worktree(&fixture).join("feature.txt"), "reworked\n")?;

    let run = fixture.read(&["work", "publish", "w42-1"])?.success()?;

    ensure!(
        run.stdout.contains("nothing has been verified"),
        "{}",
        run.stdout
    );
    Ok(())
}

#[test]
fn publishing_over_a_failing_verification_names_it_rather_than_refusing() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);
    fs::write(worktree(&fixture).join("feature.txt"), "reworked\n")?;
    drop(fixture.run(&["work", "verify", "w42-1", "--", "false"])?);

    let run = fixture.read(&["work", "publish", "w42-1"])?.success()?;

    ensure!(
        run.stdout.contains("verification `false` last failed"),
        "{}",
        run.stdout
    );
    Ok(())
}

#[test]
fn aborting_previews_first_and_leaves_the_worktree_alone() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let preview = fixture.read(&["work", "abort", "w42-1"])?.success()?;

    ensure!(
        preview.stdout.contains("Would abandon w42-1"),
        "{}",
        preview.stdout
    );
    ensure!(
        preview.stdout.contains("The pull request is not touched"),
        "{}",
        preview.stdout
    );
    ensure!(
        worktree(&fixture).is_dir(),
        "the preview removed the worktree"
    );
    Ok(())
}

#[test]
fn aborting_removes_the_worktree_and_the_branch_and_forgets_the_session() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let run = fixture
        .run(&["work", "abort", "w42-1", "--yes"])?
        .success()?;

    ensure!(run.stdout.contains("Abandoned w42-1"), "{}", run.stdout);
    ensure!(!worktree(&fixture).is_dir(), "the worktree survived");
    ensure!(
        !fixture
            .repository
            .git(&["for-each-ref", "--format=%(refname)"])?
            .contains("quinjet/work/w42-1"),
        "the session branch survived"
    );
    let listing = fixture.read(&["work", "list"])?.success()?;
    ensure!(
        listing.stdout.contains("No work sessions"),
        "{}",
        listing.stdout
    );
    Ok(())
}

#[test]
fn a_session_name_nothing_matches_is_a_not_found_with_a_way_to_look() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    for args in [
        vec!["work", "inspect", "nothing"],
        vec!["work", "diff", "nothing"],
        vec!["work", "publish", "nothing"],
        vec!["work", "abort", "nothing", "--yes"],
    ] {
        let run = fixture.run(&args)?;
        ensure!(run.code == 3, "expected 3 for {args:?}, got {}", run.code);
        ensure!(
            run.stderr.contains("quinjet work list"),
            "{args:?}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn two_sessions_on_one_pull_request_get_distinct_names_and_branches() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    drop(start(&fixture, "whole")?.success()?);

    let second = fixture.run(&[
        "work",
        "start",
        "--pr",
        "42",
        "--from",
        "whole",
        "--into",
        &fixture
            .repository
            .environment
            .join("work-2")
            .to_string_lossy(),
    ])?;

    let second = second.success()?;
    ensure!(second.stdout.contains("w42-2"), "{}", second.stdout);
    ensure!(
        second.stdout.contains("quinjet/work/w42-2"),
        "{}",
        second.stdout
    );
    Ok(())
}

#[test]
fn a_session_can_record_tasks_without_a_checkout_at_all() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .run(&[
            "work", "start", "--pr", "42", "--from", "feedback", "--json",
        ])?
        .success()?
        .json()?;

    ensure!(value["worktree"].is_null(), "{value}");
    let run = fixture.read(&["work", "diff", "w42-1"])?;
    ensure!(run.code == 1, "expected 1, got {}", run.code);
    ensure!(run.stderr.contains("no worktree"), "{}", run.stderr);
    Ok(())
}
