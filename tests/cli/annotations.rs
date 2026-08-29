use super::github::GitHubFixture;
use super::*;

#[doc = " Fake GitHub CLI cases for check annotations: two on a changed file, one"]
#[doc = " of them past the end of its patch, and one on a file the pull request"]
#[doc = " does not touch."]
pub(super) const GH_CASES: &str = r#"  *"check-runs?per_page=100"*)
    printf '123456\tClippy\t2\thttps://github.com/acme/project/runs/123456\tcompleted\n123457\tBuild\t0\thttps://github.com/acme/project/runs/123457\tcompleted\n123458\tSpell check\t1\thttps://github.com/acme/project/runs/123458\tcompleted\n'
    ;;
  *"check-runs/123456/annotations"*)
    printf 'feature.txt\t1\t1\t3\t9\tfailure\tuse a slice\tThis vector is never resized\tconsider &[T]\thttps://example.test/a1\nfeature.txt\t90\t92\t0\t0\twarning\t\tThis block is long\t\thttps://example.test/a2\n'
    ;;
  *"check-runs/123458/annotations"*)
    printf 'README.md\t2\t2\t0\t0\tnotice\tSpell Checker\tCheck your spelling\t\thttps://example.test/a3\n'
    ;;
"#;

#[test]
fn annotations_group_by_file_and_mark_what_the_patch_does_not_show() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture
        .read(&["pr", "checks", "annotations", "42"])?
        .success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    for line in [
        "README.md",
        "  i  notice   2        Spell Checker  [outside diff]  (Spell check)",
        "feature.txt",
        "  x  failure  1        use a slice  (Clippy)",
        "  !  warning  90-92    This block is long  [outside hunks]  (Clippy)",
        "1 failure, 1 warning, 1 notice · 1 on changed lines, 2 elsewhere",
    ] {
        ensure!(
            plain.stdout.lines().any(|out| out == line),
            "missing `{line}` in:\n{}",
            plain.stdout
        );
    }
    Ok(())
}

#[test]
fn annotations_json_is_a_stable_editor_contract() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["pr", "checks", "annotations", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["headOid"] == fixture.head_oid);
    ensure!(value["counts"]["failure"] == 1);
    ensure!(value["counts"]["warning"] == 1);
    ensure!(value["counts"]["notice"] == 1);
    ensure!(value["counts"]["inDiff"] == 1);
    ensure!(value["counts"]["outsideDiff"] == 2);
    let first = &value["annotations"][0];
    ensure!(first["severity"] == "failure");
    ensure!(first["path"] == "feature.txt");
    ensure!(first["startLine"] == 1);
    ensure!(first["endLine"] == 1);
    ensure!(first["startColumn"] == 3);
    ensure!(first["endColumn"] == 9);
    ensure!(first["check"] == "Clippy");
    ensure!(first["checkRunId"] == 123_456);
    ensure!(first["title"] == "use a slice");
    ensure!(first["message"] == "This vector is never resized");
    ensure!(first["rawDetails"] == "consider &[T]");
    ensure!(first["placement"] == "in-diff");
    ensure!(value["annotations"][1]["placement"] == "outside-hunks");
    ensure!(value["annotations"][2]["placement"] == "outside-diff");
    ensure!(value["annotations"][1]["startColumn"].is_null());
    Ok(())
}

#[test]
fn a_severity_filter_narrows_the_rows_and_the_counts_together() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&[
            "pr",
            "checks",
            "annotations",
            "42",
            "--severity",
            "failure",
            "--json",
        ])?
        .success()?
        .json()?;

    ensure!(
        value["annotations"]
            .as_array()
            .is_some_and(|rows| rows.len() == 1)
    );
    ensure!(value["counts"]["failure"] == 1);
    ensure!(value["counts"]["warning"] == 0);
    ensure!(value["counts"]["notice"] == 0);
    Ok(())
}

#[test]
fn check_path_and_in_diff_filters_each_narrow_the_listing() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let by_check = fixture
        .read(&[
            "pr",
            "checks",
            "annotations",
            "42",
            "--check",
            "spell",
            "--json",
        ])?
        .json()?;
    ensure!(by_check["annotations"][0]["check"] == "Spell check");
    ensure!(
        by_check["annotations"]
            .as_array()
            .is_some_and(|rows| rows.len() == 1)
    );

    let by_path = fixture
        .read(&[
            "pr",
            "checks",
            "annotations",
            "42",
            "--file",
            "feature.txt",
            "--json",
        ])?
        .json()?;
    ensure!(
        by_path["annotations"]
            .as_array()
            .is_some_and(|rows| rows.len() == 2)
    );

    let in_diff = fixture
        .read(&["pr", "checks", "annotations", "42", "--in-diff", "--json"])?
        .json()?;
    ensure!(
        in_diff["annotations"]
            .as_array()
            .is_some_and(|rows| rows.len() == 1)
    );
    ensure!(in_diff["counts"]["outsideDiff"] == 0);
    Ok(())
}

#[test]
fn grouping_by_check_and_by_severity_changes_the_headings() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let by_check = fixture
        .read(&["pr", "checks", "annotations", "42", "--group", "check"])?
        .success()?;
    ensure!(
        by_check.stdout.lines().any(|line| line == "Clippy"),
        "{}",
        by_check.stdout
    );
    ensure!(
        by_check.stdout.lines().any(|line| line == "Spell check"),
        "{}",
        by_check.stdout
    );
    ensure!(!by_check.stdout.contains("(Clippy)"), "{}", by_check.stdout);

    let by_severity = fixture
        .read(&["pr", "checks", "annotations", "42", "--group", "severity"])?
        .success()?;
    for heading in ["failure", "warning", "notice"] {
        ensure!(
            by_severity.stdout.lines().any(|line| line == heading),
            "missing `{heading}` in:\n{}",
            by_severity.stdout
        );
    }
    Ok(())
}

#[test]
fn the_full_face_prints_the_message_and_the_raw_details() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture
        .read(&[
            "pr",
            "checks",
            "annotations",
            "42",
            "--check",
            "Clippy",
            "--full",
        ])?
        .success()?;

    ensure!(
        run.stdout.contains("      This vector is never resized"),
        "{}",
        run.stdout
    );
    ensure!(run.stdout.contains("      consider &[T]"), "{}", run.stdout);
    Ok(())
}

#[test]
fn the_exit_code_reports_a_failure_only_when_one_survives_the_filter() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let failing = fixture.read(&["pr", "checks", "annotations", "42", "--exit-code"])?;
    ensure!(failing.code == 1, "expected 1, got {}", failing.code);
    ensure!(
        !failing.stdout.is_empty(),
        "a verdict still prints its rows"
    );

    let clean = fixture.read(&[
        "pr",
        "checks",
        "annotations",
        "42",
        "--severity",
        "notice",
        "--exit-code",
    ])?;
    ensure!(clean.code == 0, "expected 0, got {}", clean.code);

    let without = fixture.read(&["pr", "checks", "annotations", "42"])?;
    ensure!(without.code == 0, "expected 0, got {}", without.code);
    Ok(())
}

#[test]
fn a_filter_that_matches_nothing_says_so_rather_than_printing_an_empty_list() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture
        .read(&["pr", "checks", "annotations", "42", "--check", "nothing"])?
        .success()?;

    ensure!(run.stdout == "No annotations reported\n", "{}", run.stdout);
    Ok(())
}

#[test]
fn a_check_run_with_no_annotations_is_never_asked_for_them() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    fixture
        .read(&["pr", "checks", "annotations", "42"])?
        .success()?;

    let calls = fixture.gh_calls()?;
    ensure!(calls.contains("check-runs/123456/annotations"), "{calls}");
    ensure!(calls.contains("check-runs/123458/annotations"), "{calls}");
    ensure!(
        !calls.contains("check-runs/123457/annotations"),
        "a check run reporting no annotations was still read: {calls}"
    );
    Ok(())
}
