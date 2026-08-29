use super::*;
use crate::git::github::{
    MergeGateAutoMerge, MergeGateBlocker, MergeGateBlockerKind, MergeGateQueue, MergeGateVerdict,
    StackGateMember,
};

fn gate_blocker(kind: MergeGateBlockerKind, summary: &str, details: &[&str]) -> MergeGateBlocker {
    MergeGateBlocker {
        kind,
        summary: summary.to_owned(),
        details: details.iter().map(|detail| (*detail).to_owned()).collect(),
    }
}

fn sample_gate() -> MergeGate {
    MergeGate {
        schema_version: MergeGate::SCHEMA_VERSION,
        repository: "acme/project".to_owned(),
        number: 42,
        title: "Add feature".to_owned(),
        url: "https://github.com/acme/project/pull/42".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        verdict: MergeGateVerdict::Blocked,
        blockers: vec![
            gate_blocker(
                MergeGateBlockerKind::Ci,
                "1 required check failed",
                &["windows / test failed"],
            ),
            gate_blocker(
                MergeGateBlockerKind::Branch,
                "head is 4 commits behind main",
                &[],
            ),
        ],
        checks: MergeGateChecks {
            required_total: 2,
            required_passed: 1,
            required_failed: 1,
            ..MergeGateChecks::default()
        },
        review: MergeGateReview {
            decision: "REVIEW_REQUIRED".to_owned(),
            required_approvals: 1,
            stale_approvals: 1,
            unresolved_threads: 2,
            requested_reviewers: vec!["hubot".to_owned()],
            ..MergeGateReview::default()
        },
        branch: MergeGateBranch {
            base_ref: "main".to_owned(),
            merge_state: "BEHIND".to_owned(),
            mergeable: "MERGEABLE".to_owned(),
            behind_by: Some(4),
            ..MergeGateBranch::default()
        },
        queue: None,
        auto_merge: MergeGateAutoMerge::default(),
        warnings: vec!["2 unresolved threads do not block merging".to_owned()],
        from_cache: false,
    }
}

#[test]
fn a_merge_gate_leads_with_its_verdict_and_then_its_blockers() {
    let text = merge_gate(&sample_gate());
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(lines[0], "blocked  #42  Add feature");
    assert_eq!(lines[1], "  CI: 1 required check failed");
    assert_eq!(lines[2], "      windows / test failed");
    assert_eq!(lines[3], "  branch: head is 4 commits behind main");
    assert!(
        text.contains("checks    1 of 2 required passed, 1 failed"),
        "{text}"
    );
    assert!(
        text.contains(
            "review    review_required, 0 of 1 approvals, 1 stale, 2 unresolved, requested from hubot"
        ),
        "{text}"
    );
    assert!(
        text.contains("branch    main (4 behind), behind / mergeable"),
        "{text}"
    );
    assert!(
        text.contains("note      2 unresolved threads do not block merging"),
        "{text}"
    );
}

#[test]
fn a_mergeable_gate_prints_no_blocker_lines() {
    let mut gate = sample_gate();
    gate.verdict = MergeGateVerdict::Mergeable;
    gate.blockers.clear();
    gate.warnings.clear();

    let text = merge_gate(&gate);

    assert!(
        text.starts_with("mergeable  #42  Add feature\n\n"),
        "{text}"
    );
    assert!(!text.contains("  CI:"), "{text}");
}

#[test]
fn a_gate_says_when_the_reading_came_off_disk_and_when_a_queue_holds_it() {
    let mut gate = sample_gate();
    gate.from_cache = true;
    gate.queue = Some(MergeGateQueue {
        state: "AWAITING_CHECKS".to_owned(),
        position: 3,
        enqueued: true,
    });
    gate.auto_merge = MergeGateAutoMerge {
        enabled: true,
        method: "SQUASH".to_owned(),
        enabled_by: "octocat".to_owned(),
    };

    let text = merge_gate(&gate);

    assert!(
        text.contains("queue     position 3 (awaiting_checks)"),
        "{text}"
    );
    assert!(
        text.contains("auto      squash enabled by @octocat"),
        "{text}"
    );
    assert!(text.contains("answered from the cache"), "{text}");
}

#[test]
fn an_unknown_freshness_is_named_rather_than_shown_as_up_to_date() {
    let mut gate = sample_gate();
    gate.branch.behind_by = None;

    assert!(
        merge_gate(&gate).contains("branch    main (freshness unknown)"),
        "{}",
        merge_gate(&gate)
    );

    gate.branch.behind_by = Some(0);
    assert!(
        merge_gate(&gate).contains("branch    main (up to date)"),
        "{}",
        merge_gate(&gate)
    );
}

#[test]
fn a_stack_gate_prints_top_first_and_then_the_merge_order() {
    let mut blocked = sample_gate();
    blocked.number = 42;
    let mut clear = sample_gate();
    clear.number = 41;
    clear.title = "Build stack model".to_owned();
    clear.verdict = MergeGateVerdict::Mergeable;
    clear.blockers.clear();
    let gate = StackGate {
        schema_version: MergeGate::SCHEMA_VERSION,
        number: 12,
        base_ref: "main".to_owned(),
        size: 2,
        selected_position: 2,
        members: vec![
            StackGateMember {
                position: 1,
                number: 41,
                title: "Build stack model".to_owned(),
                selected: false,
                gate: clear,
            },
            StackGateMember {
                position: 2,
                number: 42,
                title: "Add feature".to_owned(),
                selected: true,
                gate: blocked,
            },
        ],
        verdict: MergeGateVerdict::Blocked,
        mergeable_prefix: vec![1],
        critical_position: Some(2),
        truncated: false,
        warnings: Vec::new(),
    };

    let text = stack_gate(&gate);
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(lines[0], "blocked  stack #12  2 layers  destination main");
    assert!(lines[1].starts_with(">   2  #42"), "{}", lines[1]);
    assert!(
        lines
            .iter()
            .any(|line| line.trim_start().starts_with("1  #41")),
        "{text}"
    );
    assert!(
        text.contains("merge order  positions 1 can merge now, bottom first"),
        "{text}"
    );
    assert!(
        text.contains("critical     position 2 (#42) CI: 1 required check failed"),
        "{text}"
    );
}

#[test]
fn a_stack_gate_with_nothing_ready_says_so() {
    let gate = StackGate {
        schema_version: MergeGate::SCHEMA_VERSION,
        number: 12,
        base_ref: "main".to_owned(),
        size: 1,
        selected_position: 1,
        members: Vec::new(),
        verdict: MergeGateVerdict::Blocked,
        mergeable_prefix: Vec::new(),
        critical_position: None,
        truncated: true,
        warnings: vec!["unable to read the merge gate for #41".to_owned()],
    };

    let text = stack_gate(&gate);

    assert!(
        text.contains("merge order  nothing in this stack can merge yet"),
        "{text}"
    );
    assert!(
        text.contains("note         the stack response was incomplete"),
        "{text}"
    );
    assert!(
        text.contains("note         unable to read the merge gate"),
        "{text}"
    );
}
