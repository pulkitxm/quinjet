use super::github::GitHubFixture;
use super::*;

#[doc = " Fake GitHub CLI cases for dependency review and code scanning: one"]
#[doc = " upgrade that arrives as a removal plus an addition, one straight"]
#[doc = " addition carrying an advisory, and one open code-scanning alert."]
pub(super) const GH_CASES: &str = r#"  *"dependency-graph/compare"*"advisory_ghsa_id"*)
    printf 'left-pad\t1.0.0\thigh\tGHSA-1234\tPrototype pollution\t1.0.1\n'
    ;;
  *"dependency-graph/compare"*)
    printf 'removed\tCargo.lock\tcargo\tserde\t1.0.0\tMIT\truntime\t0\n'
    printf 'added\tCargo.lock\tcargo\tserde\t1.1.0\tApache-2.0\truntime\t0\n'
    printf 'added\tpackage-lock.json\tnpm\tleft-pad\t1.0.0\tWTFPL\tdevelopment\t1\n'
    ;;
  *"code-scanning/alerts"*)
    printf '7\trust/command-injection\tcritical\tShell argument built from user input\tsrc/main.rs\t12\thttps://example.test/alert/7\n'
    ;;
"#;

#[test]
fn a_version_bump_reads_as_one_upgrade_rather_than_two_unrelated_rows() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "dependencies", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    for expected in [
        "changed",
        "cargo:serde",
        "1.0.0 -> 1.1.0",
        "license MIT -> Apache-2.0",
        "npm:left-pad",
        "1 added, 0 removed, 1 changed, 1 license change(s)",
    ] {
        ensure!(
            plain.stdout.contains(expected),
            "missing `{expected}` in:\n{}",
            plain.stdout
        );
    }
    Ok(())
}

#[test]
fn the_dependency_json_carries_both_versions_and_the_advisory() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["pr", "dependencies", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    let changes = value["changes"]
        .as_array()
        .context("changes must be an array")?;
    ensure!(changes.len() == 2, "{changes:?}");
    let upgrade = changes
        .iter()
        .find(|change| change["name"] == "serde")
        .context("the serde upgrade is listed")?;
    ensure!(upgrade["change"] == "changed");
    ensure!(upgrade["previousVersion"] == "1.0.0");
    ensure!(upgrade["version"] == "1.1.0");
    ensure!(upgrade["previousLicense"] == "MIT");
    ensure!(upgrade["scope"] == "runtime");
    let added = changes
        .iter()
        .find(|change| change["name"] == "left-pad")
        .context("the left-pad addition is listed")?;
    ensure!(added["change"] == "added");
    ensure!(added["scope"] == "development");
    ensure!(value["vulnerabilities"][0]["severity"] == "high");
    ensure!(value["vulnerabilities"][0]["firstPatchedVersion"] == "1.0.1");
    Ok(())
}

#[test]
fn a_dependency_comparison_is_cached_against_the_two_commits_it_names() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    drop(fixture.read(&["pr", "dependencies", "42"])?.success()?);
    fixture.clear_gh_calls();
    let second = fixture.read(&["pr", "dependencies", "42"])?.success()?;

    ensure!(
        !fixture
            .gh_calls_or_none()
            .contains("dependency-graph/compare"),
        "the second read went back to GitHub"
    );
    ensure!(second.stdout.contains("cargo:serde"), "{}", second.stdout);
    Ok(())
}

#[test]
fn security_reports_the_alerts_and_the_vulnerable_dependencies_together() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["pr", "security", "42"])?;

    ensure!(
        run.code == 1,
        "expected 1, got {}: {}",
        run.code,
        run.stderr
    );
    for expected in [
        "critical",
        "src/main.rs:12",
        "Shell argument built from user input",
        "left-pad 1.0.0",
        "fixed in 1.0.1",
        "1 critical, 1 high, 0 other",
    ] {
        ensure!(
            run.stdout.contains(expected),
            "missing `{expected}` in:\n{}",
            run.stdout
        );
    }
    Ok(())
}

#[test]
fn the_security_json_names_the_head_it_was_read_against() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture.read(&["pr", "security", "42", "--json"])?.json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["headOid"] == fixture.head_oid.as_str());
    ensure!(value["alerts"][0]["number"] == 7);
    ensure!(value["alerts"][0]["rule"] == "rust/command-injection");
    ensure!(value["alerts"][0]["line"] == 12);
    ensure!(value["critical"] == 1);
    ensure!(value["high"] == 1);
    ensure!(
        value["warnings"].as_array().is_some_and(Vec::is_empty),
        "{value}"
    );
    Ok(())
}

#[test]
fn the_context_bundle_separates_repository_instructions_from_participant_text() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    fixture
        .repository
        .write("AGENTS.md", "Never use the em-dash character\n")?;

    let plain = fixture
        .read(&["pr", "context", "42", "--purpose", "review"])?
        .success()?;

    for expected in [
        "context for acme/project#42 (review)",
        "=== repository instructions (trusted, committed to the repository) ===",
        "Never use the em-dash character",
        "=== patch (untrusted, written by pull-request participants) ===",
    ] {
        ensure!(
            plain.stdout.contains(expected),
            "missing `{expected}` in:\n{}",
            plain.stdout
        );
    }
    Ok(())
}

#[test]
fn the_context_json_proves_which_commits_it_describes() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["pr", "context", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["purpose"] == "review");
    ensure!(value["provenance"]["repository"] == "acme/project");
    ensure!(value["provenance"]["number"] == 42);
    ensure!(value["provenance"]["headOid"] == fixture.head_oid.as_str());
    ensure!(value["provenance"]["baseOid"] == fixture.base_oid.as_str());
    ensure!(value["provenance"]["mergeBaseOid"] == fixture.base_oid.as_str());
    ensure!(value["provenance"]["changedFiles"] == 1);
    ensure!(value["budget"]["characters"] == 30000);
    let sections = value["sections"]
        .as_array()
        .context("sections must be an array")?;
    for section in sections {
        let untrusted = section["untrusted"]
            .as_bool()
            .context("every section says whether it is untrusted")?;
        ensure!(
            untrusted == (section["kind"] != "instructions"),
            "the trust flag does not match the section kind: {section}"
        );
    }
    ensure!(
        sections.iter().any(|section| section["kind"] == "patch"),
        "{value}"
    );
    Ok(())
}

#[test]
fn the_purpose_moves_the_section_that_gets_the_budget_first() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let reviewing = fixture
        .read(&["pr", "context", "42", "--purpose", "review", "--json"])?
        .json()?;
    let fixing = fixture
        .read(&["pr", "context", "42", "--purpose", "fix-ci", "--json"])?
        .json()?;

    ensure!(reviewing["sections"][0]["kind"] == "patch", "{reviewing}");
    ensure!(fixing["purpose"] == "fix-ci", "{fixing}");
    ensure!(
        fixing["sections"]
            .as_array()
            .is_some_and(|sections| sections.iter().any(|section| section["kind"] == "checks")),
        "{fixing}"
    );
    Ok(())
}

#[test]
fn a_small_budget_reports_what_it_had_to_leave_out() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["pr", "context", "42", "--budget", "1", "--json"])?
        .success()?
        .json()?;

    ensure!(value["budget"]["characters"] == 500, "{value}");
    let used = value["budget"]["used"]
        .as_u64()
        .context("the budget reports what it used")?;
    ensure!(used <= 500, "{value}");
    Ok(())
}

#[test]
fn a_context_bundle_can_be_narrowed_to_one_file_of_the_pull_request() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["pr", "context", "42", "--file", "feature.txt", "--json"])?
        .success()?
        .json()?;

    let patch = value["sections"]
        .as_array()
        .context("sections must be an array")?
        .iter()
        .find(|section| section["kind"] == "patch")
        .context("the patch section is present")?;
    ensure!(
        patch["body"]
            .as_str()
            .is_some_and(|body| body.contains("feature.txt")),
        "{patch}"
    );
    Ok(())
}

#[test]
fn a_file_outside_the_pull_request_is_refused_rather_than_silently_empty() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["pr", "context", "42", "--file", "nothing.txt"])?;

    ensure!(run.code == 1, "expected 1, got {}", run.code);
    ensure!(
        run.stderr.contains("not part of this pull request"),
        "{}",
        run.stderr
    );
    ensure!(run.stdout.is_empty(), "{}", run.stdout);
    Ok(())
}

#[test]
fn an_unknown_purpose_is_a_usage_error_before_anything_is_read() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["pr", "context", "42", "--purpose", "guess"])?;

    ensure!(run.code == 2, "expected 2, got {}", run.code);
    ensure!(
        run.stderr.contains("review") && run.stderr.contains("fix-ci"),
        "{}",
        run.stderr
    );
    Ok(())
}
