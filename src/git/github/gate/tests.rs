use super::blockers::counted;
use super::verdict::build_gate;
use super::*;

fn node(json: serde_json::Value) -> GatePullRequestNode {
    serde_json::from_value(json).expect("the fixture matches the gate query shape")
}

fn base_node() -> serde_json::Value {
    json!({
        "number": 42,
        "title": "Add feature",
        "url": "https://github.com/acme/project/pull/42",
        "state": "OPEN",
        "isDraft": false,
        "merged": false,
        "mergeable": "MERGEABLE",
        "mergeStateStatus": "CLEAN",
        "baseRefName": "main",
        "baseRefOid": "b".repeat(40),
        "headRefOid": "a".repeat(40),
        "reviewDecision": "APPROVED",
        "mergeQueueEntry": null,
        "autoMergeRequest": null,
        "reviewRequests": { "totalCount": 0, "nodes": [] },
        "latestOpinionatedReviews": { "nodes": [
            { "state": "APPROVED", "author": { "login": "octocat" }, "commit": { "oid": "a".repeat(40) } }
        ] },
        "reviewThreads": { "totalCount": 0, "pageInfo": { "hasNextPage": false }, "nodes": [] },
        "commits": { "nodes": [ { "commit": {
            "oid": "a".repeat(40),
            "statusCheckRollup": { "contexts": {
                "totalCount": 1,
                "pageInfo": { "hasNextPage": false },
                "nodes": [ {
                    "__typename": "CheckRun",
                    "name": "test",
                    "status": "COMPLETED",
                    "conclusion": "SUCCESS",
                    "detailsUrl": "https://example.test/1",
                    "isRequired": true,
                    "checkSuite": { "workflowRun": { "workflow": { "name": "CI" } } }
                } ]
            } }
        } } ] },
        "baseRef": {
            "name": "main",
            "target": { "oid": "b".repeat(40) },
            "refUpdateRule": {
                "requiredApprovingReviewCount": 1,
                "requiredStatusCheckContexts": ["test"],
                "requiresCodeOwnerReviews": false,
                "requiresConversationResolution": true,
                "requiresLinearHistory": false,
                "requiresSignatures": false
            }
        }
    })
}

fn gate_for(mutate: impl FnOnce(&mut serde_json::Value)) -> MergeGate {
    let mut value = base_node();
    mutate(&mut value);
    build_gate("acme/project", node(value), Some(0), "b".repeat(40))
}

fn kinds(gate: &MergeGate) -> Vec<MergeGateBlockerKind> {
    gate.blockers.iter().map(|blocker| blocker.kind).collect()
}

#[test]
fn a_clean_pull_request_is_mergeable_with_no_blockers() {
    let gate = gate_for(|_| {});
    assert_eq!(gate.verdict, MergeGateVerdict::Mergeable);
    assert!(gate.blockers.is_empty(), "{:?}", gate.blockers);
    assert_eq!(gate.checks.required_total, 1);
    assert_eq!(gate.checks.required_passed, 1);
    assert_eq!(gate.review.current_approvals, 1);
    assert_eq!(gate.verdict.exit_code(), 0);
    assert_eq!(gate.schema_version, MergeGate::SCHEMA_VERSION);
}

#[test]
fn a_failing_required_check_blocks_and_names_the_workflow() {
    let gate = gate_for(|value| {
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"][0]["conclusion"] =
            json!("FAILURE");
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"][0]["name"] =
            json!("test");
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"][0]["checkSuite"]
            ["workflowRun"]["workflow"]["name"] = json!("windows");
    });
    assert_eq!(gate.verdict, MergeGateVerdict::Blocked);
    assert_eq!(kinds(&gate), vec![MergeGateBlockerKind::Ci]);
    let blocker = &gate.blockers[0];
    assert_eq!(blocker.summary, "1 required check failed");
    assert_eq!(blocker.details, vec!["windows / test failed".to_owned()]);
    assert_eq!(gate.verdict.exit_code(), 1);
}

#[test]
fn an_optional_failure_is_a_note_rather_than_a_blocker() {
    let gate = gate_for(|value| {
        let contexts = &mut value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"];
        contexts["totalCount"] = json!(2);
        contexts["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(json!({
                "__typename": "CheckRun",
                "name": "docs",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
                "detailsUrl": "",
                "isRequired": false,
                "checkSuite": null
            }));
    });
    assert_eq!(gate.verdict, MergeGateVerdict::Mergeable);
    assert_eq!(gate.checks.optional_failed, 1);
    assert!(
        gate.warnings
            .iter()
            .any(|note| note.contains("optional check failed")),
        "{:?}",
        gate.warnings
    );
}

#[test]
fn a_required_context_that_never_reported_blocks() {
    let gate = gate_for(|value| {
        value["baseRef"]["refUpdateRule"]["requiredStatusCheckContexts"] =
            json!(["test", "windows / test"]);
    });
    assert_eq!(gate.verdict, MergeGateVerdict::Blocked);
    assert_eq!(
        gate.checks.missing_required,
        vec!["windows / test".to_owned()]
    );
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Ci)
        .expect("a CI blocker");
    assert_eq!(blocker.summary, "1 required check never reported");
}

#[test]
fn an_approval_of_an_older_commit_does_not_count_for_the_head() {
    let gate = gate_for(|value| {
        value["latestOpinionatedReviews"]["nodes"][0]["commit"]["oid"] = json!("c".repeat(40));
        value["reviewDecision"] = json!("REVIEW_REQUIRED");
    });
    assert_eq!(gate.verdict, MergeGateVerdict::Blocked);
    assert_eq!(gate.review.approvals, 1);
    assert_eq!(gate.review.current_approvals, 0);
    assert_eq!(gate.review.stale_approvals, 1);
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Approval)
        .expect("an approval blocker");
    assert_eq!(blocker.summary, "the latest push has not been approved");
}

#[test]
fn requested_changes_block_and_name_the_reviewer() {
    let gate = gate_for(|value| {
        value["latestOpinionatedReviews"]["nodes"] = json!([
            { "state": "CHANGES_REQUESTED", "author": { "login": "hubot" }, "commit": { "oid": "a".repeat(40) } }
        ]);
        value["reviewDecision"] = json!("CHANGES_REQUESTED");
    });
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Review)
        .expect("a review blocker");
    assert_eq!(blocker.summary, "1 reviewer requested changes");
    assert_eq!(blocker.details, vec!["@hubot".to_owned()]);
    assert_eq!(gate.review.changes_requested_by, vec!["hubot".to_owned()]);
}

#[test]
fn unresolved_threads_block_only_when_the_branch_requires_resolution() {
    let threads = json!({
        "totalCount": 2,
        "pageInfo": { "hasNextPage": false },
        "nodes": [
            { "isResolved": false, "isOutdated": false },
            { "isResolved": false, "isOutdated": true }
        ]
    });
    let required = gate_for(|value| value["reviewThreads"] = threads.clone());
    let blocker = required
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Threads)
        .expect("a thread blocker");
    assert_eq!(blocker.summary, "2 unresolved threads");
    assert_eq!(required.review.outdated_unresolved_threads, 1);

    let optional = gate_for(|value| {
        value["reviewThreads"] = threads;
        value["baseRef"]["refUpdateRule"]["requiresConversationResolution"] = json!(false);
    });
    assert_eq!(optional.verdict, MergeGateVerdict::Mergeable);
    assert!(
        optional
            .warnings
            .iter()
            .any(|note| note.contains("do not block merging")),
        "{:?}",
        optional.warnings
    );
}

#[test]
fn a_behind_head_blocks_and_says_how_far_behind_it_is() {
    let mut value = base_node();
    value["mergeStateStatus"] = json!("BEHIND");
    let gate = build_gate("acme/project", node(value), Some(4), "b".repeat(40));
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Branch)
        .expect("a branch blocker");
    assert_eq!(blocker.summary, "head is 4 commits behind main");
    assert_eq!(gate.branch.behind_by, Some(4));
}

#[test]
fn a_conflict_blocks_whatever_else_is_true() {
    let gate = gate_for(|value| {
        value["mergeable"] = json!("CONFLICTING");
        value["mergeStateStatus"] = json!("DIRTY");
    });
    assert_eq!(kinds(&gate), vec![MergeGateBlockerKind::Conflict]);
    assert_eq!(
        gate.blockers[0].summary,
        "the head branch conflicts with main"
    );
}

#[test]
fn a_draft_blocks_on_its_state() {
    let gate = gate_for(|value| value["isDraft"] = json!(true));
    assert_eq!(kinds(&gate), vec![MergeGateBlockerKind::State]);
    assert!(gate.is_draft);
}

#[test]
fn a_deployment_waiting_for_approval_blocks() {
    let gate = gate_for(|value| {
        let contexts = &mut value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"];
        contexts["totalCount"] = json!(2);
        contexts["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(json!({
                "__typename": "CheckRun",
                "name": "deploy staging",
                "status": "WAITING",
                "conclusion": null,
                "detailsUrl": "",
                "isRequired": false,
                "checkSuite": null
            }));
    });
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Deployment)
        .expect("a deployment blocker");
    assert_eq!(blocker.summary, "1 deployment is waiting for approval");
    assert_eq!(blocker.details, vec!["deploy staging".to_owned()]);
}

#[test]
fn a_block_github_reports_but_quinjet_cannot_name_is_never_reported_as_mergeable() {
    let gate = gate_for(|value| {
        value["mergeStateStatus"] = json!("BLOCKED");
        value["baseRef"]["refUpdateRule"]["requiresSignatures"] = json!(true);
    });
    assert_eq!(gate.verdict, MergeGateVerdict::Blocked);
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Policy)
        .expect("a policy blocker");
    assert_eq!(
        blocker.details,
        vec!["the base branch requires signed commits".to_owned()]
    );
}

#[test]
fn an_undecided_mergeability_is_unknown_rather_than_mergeable() {
    let gate = gate_for(|value| {
        value["mergeable"] = json!("UNKNOWN");
        value["mergeStateStatus"] = json!("UNKNOWN");
    });
    assert_eq!(gate.verdict, MergeGateVerdict::Unknown);
    assert_eq!(gate.verdict.exit_code(), 4);
    assert!(!gate.verdict.is_settled());
}

#[test]
fn a_merged_pull_request_reports_merged_and_exits_zero() {
    let gate = gate_for(|value| {
        value["merged"] = json!(true);
        value["state"] = json!("MERGED");
    });
    assert_eq!(gate.verdict, MergeGateVerdict::Merged);
    assert_eq!(gate.blockers, Vec::new());
    assert_eq!(gate.verdict.exit_code(), 0);
}

#[test]
fn a_closed_pull_request_blocks_on_its_state() {
    let gate = gate_for(|value| value["state"] = json!("CLOSED"));
    assert_eq!(gate.verdict, MergeGateVerdict::Closed);
    assert_eq!(kinds(&gate), vec![MergeGateBlockerKind::State]);
    assert_eq!(gate.verdict.exit_code(), 1);
}

#[test]
fn missing_branch_rules_are_a_warning_rather_than_a_silent_pass() {
    let gate = gate_for(|value| value["baseRef"]["refUpdateRule"] = json!(null));
    assert!(
        gate.warnings
            .iter()
            .any(|warning| warning.contains("could not read branch rules")),
        "{:?}",
        gate.warnings
    );
    assert_eq!(gate.review.required_approvals, 0);
}

#[test]
fn a_truncated_rollup_is_reported_rather_than_treated_as_complete() {
    let gate = gate_for(|value| {
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["pageInfo"]["hasNextPage"] =
            json!(true);
        value["reviewThreads"]["pageInfo"]["hasNextPage"] = json!(true);
    });
    assert!(gate.checks.truncated);
    assert!(gate.review.threads_truncated);
    assert_eq!(gate.warnings.len(), 2);
}

#[test]
fn a_legacy_status_context_is_read_alongside_check_runs() {
    let gate = gate_for(|value| {
        let contexts = &mut value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"];
        contexts["totalCount"] = json!(2);
        contexts["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(json!({
                "__typename": "StatusContext",
                "context": "continuous-integration/legacy",
                "state": "FAILURE",
                "targetUrl": "https://example.test/legacy",
                "isRequired": true
            }));
    });
    assert_eq!(gate.checks.required_total, 2);
    assert_eq!(gate.checks.required_failed, 1);
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Ci)
        .expect("a CI blocker");
    assert_eq!(
        blocker.details,
        vec!["continuous-integration/legacy failed".to_owned()]
    );
}

#[test]
fn blockers_are_ordered_by_kind_so_the_first_line_is_the_most_actionable() {
    let gate = gate_for(|value| {
        value["isDraft"] = json!(true);
        value["mergeable"] = json!("CONFLICTING");
        value["mergeStateStatus"] = json!("BEHIND");
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"][0]["conclusion"] =
            json!("FAILURE");
        value["reviewThreads"] = json!({
            "totalCount": 1,
            "pageInfo": { "hasNextPage": false },
            "nodes": [ { "isResolved": false, "isOutdated": false } ]
        });
    });
    assert_eq!(
        kinds(&gate),
        vec![
            MergeGateBlockerKind::State,
            MergeGateBlockerKind::Conflict,
            MergeGateBlockerKind::Ci,
            MergeGateBlockerKind::Threads,
            MergeGateBlockerKind::Branch,
        ]
    );
    assert_eq!(gate.headline(), "state: the pull request is a draft");
}

#[test]
fn pending_required_checks_block_separately_from_failures() {
    let gate = gate_for(|value| {
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"][0]["status"] =
            json!("IN_PROGRESS");
        value["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"][0]["conclusion"] =
            json!(null);
    });
    assert_eq!(gate.checks.required_pending, 1);
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Ci)
        .expect("a CI blocker");
    assert_eq!(blocker.summary, "1 required check has not finished");
}

#[test]
fn a_merge_queue_entry_is_reported_and_an_unmergeable_one_blocks() {
    let queued = gate_for(|value| {
        value["mergeQueueEntry"] = json!({ "position": 3, "state": "AWAITING_CHECKS" });
    });
    assert_eq!(queued.verdict, MergeGateVerdict::Mergeable);
    assert_eq!(queued.queue.as_ref().map(|queue| queue.position), Some(3));

    let stuck = gate_for(|value| {
        value["mergeQueueEntry"] = json!({ "position": 1, "state": "UNMERGEABLE" });
    });
    assert_eq!(
        stuck
            .blockers
            .iter()
            .map(|blocker| blocker.kind)
            .collect::<Vec<_>>(),
        vec![MergeGateBlockerKind::Queue]
    );
}

#[test]
fn blocker_details_are_capped_so_one_blocker_cannot_flood_the_answer() {
    let gate = gate_for(|value| {
        value["baseRef"]["refUpdateRule"]["requiredStatusCheckContexts"] = json!(
            (0..12)
                .map(|index| format!("gate-{index}"))
                .collect::<Vec<_>>()
        );
    });
    let blocker = gate
        .blockers
        .iter()
        .find(|blocker| blocker.kind == MergeGateBlockerKind::Ci)
        .expect("a CI blocker");
    assert_eq!(gate.checks.missing_required.len(), 12);
    assert_eq!(blocker.details.len(), 12);
}

#[test]
fn the_counted_helper_spells_both_forms_the_caller_gives_it() {
    assert_eq!(counted(1, "commit", "commits"), "1 commit");
    assert_eq!(counted(0, "commit", "commits"), "0 commits");
    assert_eq!(counted(9, "commit", "commits"), "9 commits");
}

#[test]
fn a_gate_round_trips_through_the_cache_encoding_unchanged() {
    let value = base_node();
    let direct = build_gate("acme/project", node(value.clone()), Some(0), "b".repeat(40));
    let encoded = serde_json::to_vec(&node(value)).expect("the node serializes");
    let decoded: GatePullRequestNode =
        serde_json::from_slice(&encoded).expect("the node round-trips");
    let restored = build_gate("acme/project", decoded, Some(0), "b".repeat(40));
    assert_eq!(direct, restored);
}
