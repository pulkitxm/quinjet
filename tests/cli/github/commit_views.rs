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
