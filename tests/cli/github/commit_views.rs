use super::*;

#[test]
fn selected_commit_files_and_diff_use_a_read_only_first_parent_comparison() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let files = fixture
        .read(&["pr", "files", "42", "--commit", &fixture.head_oid, "--json"])?
        .success()?
        .json()?;
    ensure!(files["files"][0]["path"] == "feature.txt");
    let prefix = fixture.head_oid.get(..7).context("missing hash prefix")?;
    let diff = fixture
        .read(&["pr", "diff", "42", "--commit", prefix])?
        .success()?;
    ensure!(diff.stdout.contains("+from pull request"));
    ensure!(!diff.stdout.contains("local worktree change"));
    Ok(())
}

#[test]
fn selected_commit_checks_and_logs_query_the_hash_not_pr_head() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let checks = fixture
        .read(&[
            "pr",
            "checks",
            "42",
            "--commit",
            &fixture.head_oid,
            "--json",
        ])?
        .success()?
        .json()?;
    ensure!(
        checks["checks"]
            .as_array()
            .is_some_and(|checks| checks.len() == 2)
    );
    let gated = fixture.read(&[
        "pr",
        "checks",
        "42",
        "--commit",
        &fixture.head_oid,
        "--exit-code",
    ])?;
    ensure!(gated.code == 1);
    let logs = fixture
        .read(&[
            "pr",
            "logs",
            "42",
            "Commit tests",
            "--commit",
            &fixture.head_oid,
        ])?
        .success()?;
    ensure!(logs.stdout.contains("tests passed"));
    let calls = fixture.gh_calls()?;
    ensure!(calls.contains(&format!("commits/{}/check-runs?", fixture.head_oid)));
    ensure!(calls.contains(&format!("commits/{}/status?", fixture.head_oid)));
    ensure!(!calls.contains("argv\tpr\tchecks"));
    Ok(())
}

#[test]
fn commit_inspection_rejects_unknown_and_short_hashes() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let unknown = fixture.read(&["pr", "files", "42", "--commit", &"f".repeat(40)])?;
    ensure!(unknown.code != 0);
    ensure!(unknown.stderr.contains("does not match"));
    let short = fixture.read(&["pr", "checks", "42", "--commit", "abc"])?;
    ensure!(short.code != 0);
    ensure!(short.stderr.contains("at least seven"));
    Ok(())
}

#[test]
fn pull_request_commits_use_local_history_before_github() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let plain = fixture.read(&["pr", "commits", "42"])?.success()?;
    ensure!(plain.stdout.contains("feature"));
    ensure!(!plain.stdout.contains("build the base"));
    let json = fixture
        .read(&["pr", "commits", "42", "--refresh", "--json"])?
        .success()?
        .json()?;
    ensure!(json["totalCommits"] == 2);
    ensure!(json["commits"][0]["oid"] == fixture.base_oid);
    ensure!(json["commits"][1]["subject"] == "feature");
    ensure!(json["commits"][0]["authorLogin"].is_null());
    ensure!(!fixture.gh_calls()?.contains("commits(last:100,before:"));
    Ok(())
}

#[test]
fn ancestor_and_initial_commits_support_files_diff_and_checks() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let files = fixture
        .read(&["pr", "files", "42", "--commit", &fixture.base_oid, "--json"])?
        .success()?
        .json()?;
    ensure!(files["files"][0]["path"] == "README.md");
    ensure!(files["files"][0]["status"] == "added");
    let diff = fixture
        .read(&["pr", "diff", "42", "--commit", &fixture.base_oid])?
        .success()?;
    ensure!(diff.stdout.contains("README.md"));
    ensure!(!diff.stdout.contains("local worktree change"));
    let checks = fixture
        .read(&[
            "pr",
            "checks",
            "42",
            "--commit",
            &fixture.base_oid,
            "--json",
        ])?
        .success()?
        .json()?;
    ensure!(
        checks["checks"]
            .as_array()
            .is_some_and(|checks| checks.len() == 2)
    );
    ensure!(
        fixture
            .gh_calls()?
            .contains(&format!("commits/{}/check-runs?", fixture.base_oid))
    );
    Ok(())
}
