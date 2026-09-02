use super::github::GitHubFixture;
use super::*;

#[test]
fn the_stack_review_says_what_can_merge_and_which_member_holds_up_the_rest() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["stack", "review", "42"])?;

    ensure!(
        run.code == 0,
        "expected 0, got {}: {}",
        run.code,
        run.stderr
    );
    for expected in [
        "merge     1, then stop",
        "critical  position 2 (#42) CI: 1 required check failed",
        "first red position 2 (#42) windows / test",
    ] {
        ensure!(
            run.stdout.lines().any(|line| line == expected),
            "missing `{expected}` in:\n{}",
            run.stdout
        );
    }
    Ok(())
}

#[test]
fn the_review_marks_the_clear_member_and_the_blocked_one_differently() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["stack", "review", "42"])?.success()?;

    ensure!(
        run.stdout.contains("mergeable  clear"),
        "the bottom member is not marked clear:\n{}",
        run.stdout
    );
    ensure!(
        run.stdout.contains("blocked    own"),
        "the blocked member is not marked as blocked by its own gate:\n{}",
        run.stdout
    );
    Ok(())
}

#[test]
fn the_review_json_carries_the_merge_order_and_the_critical_path() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["stack", "review", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["number"] == 12, "{value}");
    ensure!(value["baseRef"] == "main");
    ensure!(value["size"] == 2);
    ensure!(value["selectedPosition"] == 2);
    ensure!(value["mergeOrder"] == serde_json::json!([1]), "{value}");
    ensure!(value["criticalPosition"] == 2);
    ensure!(value["criticalPath"] == serde_json::json!([2]), "{value}");
    ensure!(value["members"][0]["position"] == 1);
    ensure!(value["members"][0]["verdict"] == "mergeable");
    ensure!(value["members"][0]["blockSource"] == "none");
    ensure!(value["members"][1]["verdict"] == "blocked");
    ensure!(value["members"][1]["blockSource"] == "own");
    ensure!(value["members"][1]["selected"] == true);
    ensure!(
        value["members"][1]["failingChecks"] == serde_json::json!(["windows / test"]),
        "{value}"
    );
    Ok(())
}

#[test]
fn an_approval_invalidated_by_a_later_push_names_the_reviewer_who_gave_it() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["stack", "review", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["staleApprovals"] == 1, "{value}");
    let stale = &value["members"][1]["staleApprovals"][0];
    ensure!(stale["reviewer"] == "octocat", "{stale}");
    ensure!(
        stale["approvedOid"] == "0000000000000000000000000000000000000000",
        "{stale}"
    );
    ensure!(stale["headOid"] == fixture.head_oid.as_str(), "{stale}");
    let plain = fixture.read(&["stack", "review", "42"])?.success()?;
    ensure!(
        plain
            .stdout
            .contains("stale     1 approval invalidated by a later push (octocat)"),
        "{}",
        plain.stdout
    );
    Ok(())
}

#[test]
fn the_incremental_review_measures_each_member_against_its_own_parent() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["stack", "review", "42", "--incremental", "--json"])?
        .success()?
        .json()?;

    let members = value["members"]
        .as_array()
        .context("members must be an array")?;
    ensure!(members.len() == 2, "{value}");
    for member in members {
        ensure!(
            member["changedFiles"].is_number(),
            "an incremental review counts each member's own files: {member}"
        );
        ensure!(member["pathsTruncated"] == false, "{member}");
    }
    Ok(())
}

#[test]
fn the_review_exit_code_reports_only_whether_the_stack_can_merge() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let blocked = fixture.read(&["stack", "review", "42", "--exit-code"])?;
    ensure!(blocked.code == 1, "expected 1, got {}", blocked.code);
    ensure!(
        !blocked.stdout.is_empty(),
        "a verdict still prints its members"
    );

    let quiet = fixture.read(&["stack", "review", "42"])?;
    ensure!(quiet.code == 0, "expected 0, got {}", quiet.code);
    Ok(())
}

#[test]
fn the_stack_queue_reads_bottom_to_top_and_names_where_it_unblocks_from() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["stack", "feedback", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    ensure!(plain.stdout.contains("#42"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("thread"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("next  position"), "{}", plain.stdout);
    Ok(())
}

#[test]
fn the_stack_queue_json_totals_every_member_together() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["stack", "feedback", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["number"] == 12);
    ensure!(value["size"] == 2);
    ensure!(value["viewer"] == "octocat");
    let members = value["members"]
        .as_array()
        .context("members must be an array")?;
    ensure!(members.len() == 2, "{value}");
    ensure!(members[0]["position"] == 1, "{value}");
    ensure!(members[1]["position"] == 2, "{value}");
    ensure!(value["counts"]["blocking"].is_number(), "{value}");
    Ok(())
}

#[test]
fn filtering_the_stack_queue_moves_the_totals_with_the_rows() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let all = fixture
        .read(&["stack", "feedback", "42", "--json"])?
        .json()?;
    let blocking = fixture
        .read(&["stack", "feedback", "42", "--unresolved", "--json"])?
        .json()?;

    ensure!(blocking["counts"]["advisory"] == 0, "{blocking}");
    let all_rows: usize = all["members"]
        .as_array()
        .context("members must be an array")?
        .iter()
        .map(|member| member["items"].as_array().map_or(0, Vec::len))
        .sum();
    let blocking_rows: usize = blocking["members"]
        .as_array()
        .context("members must be an array")?
        .iter()
        .map(|member| member["items"].as_array().map_or(0, Vec::len))
        .sum();
    ensure!(
        blocking_rows <= all_rows,
        "filtering added rows: {blocking_rows} > {all_rows}"
    );
    Ok(())
}

#[test]
fn the_stack_queue_exit_code_reports_only_what_the_merge_waits_on() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let blocking = fixture.read(&["stack", "feedback", "42", "--exit-code"])?;
    ensure!(blocking.code == 1, "expected 1, got {}", blocking.code);

    let quiet = fixture.read(&["stack", "feedback", "42"])?;
    ensure!(quiet.code == 0, "expected 0, got {}", quiet.code);
    Ok(())
}

#[test]
fn a_pull_request_outside_a_stack_is_a_not_found_for_both_verbs() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    for verb in ["review", "feedback"] {
        let run = fixture.read(&["stack", verb, "8"])?;
        ensure!(
            run.code == 3 || run.code == 1,
            "expected 3 or 1 for `stack {verb} 8`, got {}: {}",
            run.code,
            run.stderr
        );
        ensure!(run.stdout.is_empty(), "{verb}: {}", run.stdout);
    }
    Ok(())
}
