use super::github::GitHubFixture;
use super::*;

#[doc = " Fake GitHub CLI cases for the merge gate: one blocked pull request, one"]
#[doc = " clean stack member below it, and the comparison behind the behind-count."]
pub(super) const GH_CASES: &str = r#"  *"refUpdateRule"*'"number":41'*)
    printf '{"data":{"repository":{"pullRequest":{"number":41,"title":"Build stack model","url":"https://github.com/acme/project/pull/41","state":"OPEN","isDraft":false,"merged":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","baseRefName":"main","baseRefOid":"%s","headRefOid":"%s","reviewDecision":"APPROVED","mergeQueueEntry":null,"autoMergeRequest":null,"reviewRequests":{"totalCount":0,"nodes":[]},"latestOpinionatedReviews":{"nodes":[{"state":"APPROVED","author":{"login":"octocat"},"commit":{"oid":"%s"}}]},"reviewThreads":{"totalCount":0,"pageInfo":{"hasNextPage":false},"nodes":[]},"commits":{"nodes":[{"commit":{"oid":"%s","statusCheckRollup":{"contexts":{"totalCount":1,"pageInfo":{"hasNextPage":false},"nodes":[{"__typename":"CheckRun","name":"test","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"","isRequired":true,"checkSuite":{"workflowRun":{"workflow":{"name":"CI"}}}}]}}}}]},"baseRef":{"name":"main","target":{"oid":"%s"},"refUpdateRule":{"requiredApprovingReviewCount":1,"requiredStatusCheckContexts":["test"],"requiresCodeOwnerReviews":false,"requiresConversationResolution":true,"requiresLinearHistory":false,"requiresSignatures":false}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID"
    ;;
  *"refUpdateRule"*)
    printf '{"data":{"repository":{"pullRequest":{"number":42,"title":"Add feature","url":"https://github.com/acme/project/pull/42","state":"OPEN","isDraft":false,"merged":false,"mergeable":"MERGEABLE","mergeStateStatus":"BEHIND","baseRefName":"main","baseRefOid":"%s","headRefOid":"%s","reviewDecision":"REVIEW_REQUIRED","mergeQueueEntry":null,"autoMergeRequest":null,"reviewRequests":{"totalCount":1,"nodes":[{"requestedReviewer":{"__typename":"User","login":"hubot"}}]},"latestOpinionatedReviews":{"nodes":[{"state":"APPROVED","author":{"login":"octocat"},"commit":{"oid":"0000000000000000000000000000000000000000"}}]},"reviewThreads":{"totalCount":1,"pageInfo":{"hasNextPage":false},"nodes":[{"isResolved":false,"isOutdated":false}]},"commits":{"nodes":[{"commit":{"oid":"%s","statusCheckRollup":{"contexts":{"totalCount":2,"pageInfo":{"hasNextPage":false},"nodes":[{"__typename":"CheckRun","name":"test","status":"COMPLETED","conclusion":"FAILURE","detailsUrl":"https://example.test/windows","isRequired":true,"checkSuite":{"workflowRun":{"workflow":{"name":"windows"}}}},{"__typename":"CheckRun","name":"lint","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"","isRequired":true,"checkSuite":{"workflowRun":{"workflow":{"name":"Quality"}}}}]}}}}]},"baseRef":{"name":"main","target":{"oid":"%s"},"refUpdateRule":{"requiredApprovingReviewCount":1,"requiredStatusCheckContexts":["test"],"requiresCodeOwnerReviews":false,"requiresConversationResolution":true,"requiresLinearHistory":false,"requiresSignatures":false}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID" "$FAKE_BASE_OID"
    ;;
  *"compare/$FAKE_HEAD_OID...$FAKE_BASE_OID"*)
    printf '4\n'
    ;;
"#;

#[test]
fn pull_request_gate_explains_every_blocker_and_exits_one() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "gate", "42"])?;

    ensure!(
        plain.code == 1,
        "expected blocked to exit 1: {}",
        plain.code
    );
    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    let expected = [
        "blocked  #42  Add feature",
        "  CI: 1 required check failed",
        "      windows / test failed",
        "  approval: the latest push has not been approved",
        "  threads: 1 unresolved thread",
        "  branch: head is 4 commits behind main",
        "checks    1 of 2 required passed, 1 failed",
        "branch    main (4 behind), behind / mergeable",
    ];
    for line in expected {
        ensure!(
            plain.stdout.lines().any(|out| out == line),
            "missing `{line}` in:\n{}",
            plain.stdout
        );
    }
    ensure!(
        plain.stdout.contains("requested from hubot"),
        "{}",
        plain.stdout
    );
    Ok(())
}

#[test]
fn pull_request_gate_json_is_a_stable_machine_contract() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["pr", "gate", "42", "--json"])?;
    ensure!(run.code == 1, "expected blocked to exit 1: {}", run.code);
    let value = run.json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["verdict"] == "blocked");
    ensure!(value["repository"] == "acme/project");
    ensure!(value["number"] == 42);
    let kinds: Vec<String> = value["blockers"]
        .as_array()
        .context("blockers must be an array")?
        .iter()
        .map(|blocker| blocker["kind"].as_str().unwrap_or_default().to_owned())
        .collect();
    ensure!(
        kinds == ["ci", "approval", "threads", "branch"],
        "unexpected blocker order: {kinds:?}"
    );
    ensure!(value["blockers"][0]["details"][0] == "windows / test failed");
    ensure!(value["checks"]["requiredTotal"] == 2);
    ensure!(value["checks"]["requiredFailed"] == 1);
    ensure!(value["checks"]["requiredPassed"] == 1);
    ensure!(value["review"]["approvals"] == 1);
    ensure!(value["review"]["currentApprovals"] == 0);
    ensure!(value["review"]["staleApprovals"] == 1);
    ensure!(value["review"]["unresolvedThreads"] == 1);
    ensure!(value["review"]["requiredApprovals"] == 1);
    ensure!(value["review"]["requestedReviewers"][0] == "hubot");
    ensure!(value["branch"]["behindBy"] == 4);
    ensure!(value["branch"]["mergeState"] == "BEHIND");
    ensure!(value["queue"].is_null());
    ensure!(value["autoMerge"]["enabled"] == false);
    Ok(())
}

#[test]
fn pull_request_gate_suppresses_its_exit_code_on_request() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["pr", "gate", "42", "--no-exit-code"])?;

    ensure!(run.code == 0, "expected 0, got {}", run.code);
    ensure!(run.stdout.starts_with("blocked"), "{}", run.stdout);
    Ok(())
}

#[test]
fn pull_request_gate_answers_a_repeat_from_the_cache() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let first = fixture.read(&["pr", "gate", "42", "--json"])?.json()?;
    ensure!(first["fromCache"] == false);
    fixture.clear_gh_calls();

    let second = fixture.read(&["pr", "gate", "42", "--json"])?.json()?;
    ensure!(second["fromCache"] == true);
    ensure!(second["verdict"] == first["verdict"]);
    ensure!(second["blockers"] == first["blockers"]);

    let calls = fs::read_to_string(&fixture.gh_capture).unwrap_or_default();
    ensure!(
        !calls.contains("refUpdateRule"),
        "a cached gate re-asked GitHub: {calls}"
    );

    let refreshed = fixture
        .read(&["pr", "gate", "42", "--refresh", "--json"])?
        .json()?;
    ensure!(refreshed["fromCache"] == false);
    ensure!(fixture.gh_calls()?.contains("refUpdateRule"));
    Ok(())
}

#[test]
fn stack_gate_orders_members_and_names_the_critical_position() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["stack", "gate", "42"])?;

    ensure!(
        plain.code == 1,
        "expected blocked to exit 1: {}",
        plain.code
    );
    ensure!(
        plain.stdout.starts_with("blocked  stack #12  2 layers"),
        "{}",
        plain.stdout
    );
    ensure!(
        plain
            .stdout
            .contains("merge order  positions 1 can merge now"),
        "{}",
        plain.stdout
    );
    ensure!(
        plain.stdout.contains("critical     position 2 (#42) CI:"),
        "{}",
        plain.stdout
    );

    let json = fixture.read(&["stack", "gate", "42", "--json"])?.json()?;
    ensure!(json["schemaVersion"] == 1);
    ensure!(json["verdict"] == "blocked");
    ensure!(json["members"][0]["position"] == 1);
    ensure!(json["members"][0]["gate"]["verdict"] == "mergeable");
    ensure!(json["members"][0]["selected"] == false);
    ensure!(json["members"][1]["position"] == 2);
    ensure!(json["members"][1]["gate"]["verdict"] == "blocked");
    ensure!(json["members"][1]["selected"] == true);
    ensure!(json["mergeablePrefix"][0] == 1);
    ensure!(
        json["mergeablePrefix"]
            .as_array()
            .is_some_and(|prefix| prefix.len() == 1)
    );
    ensure!(json["criticalPosition"] == 2);
    Ok(())
}

#[test]
fn stack_gate_suppresses_its_exit_code_on_request() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&["stack", "gate", "42", "--no-exit-code"])?;

    ensure!(run.code == 0, "expected 0, got {}", run.code);
    Ok(())
}
