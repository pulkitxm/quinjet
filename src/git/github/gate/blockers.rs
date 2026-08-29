#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) struct VerdictContext<'a> {
    pub(super) merged: bool,
    pub(super) state: String,
    pub(super) is_draft: bool,
    pub(super) merge_state: &'a str,
    pub(super) mergeable: &'a str,
    pub(super) checks: &'a MergeGateChecks,
    pub(super) review: &'a MergeGateReview,
    pub(super) branch: &'a MergeGateBranch,
    pub(super) queue: Option<&'a MergeGateQueue>,
}

pub(super) fn decide(
    context: &VerdictContext<'_>,
) -> (MergeGateVerdict, Vec<MergeGateBlocker>, Vec<String>) {
    if context.merged {
        return (MergeGateVerdict::Merged, Vec::new(), Vec::new());
    }
    if context.state.eq_ignore_ascii_case("CLOSED") {
        return (
            MergeGateVerdict::Closed,
            vec![MergeGateBlocker::new(
                MergeGateBlockerKind::State,
                "the pull request is closed",
            )],
            Vec::new(),
        );
    }
    let mut blockers = Vec::new();
    let mut notes = Vec::new();
    if context.is_draft {
        blockers.push(MergeGateBlocker::new(
            MergeGateBlockerKind::State,
            "the pull request is a draft",
        ));
    }
    if context.mergeable.eq_ignore_ascii_case("CONFLICTING")
        || context.merge_state.eq_ignore_ascii_case("DIRTY")
    {
        blockers.push(MergeGateBlocker::new(
            MergeGateBlockerKind::Conflict,
            format!(
                "the head branch conflicts with {}",
                display_ref(&context.branch.base_ref)
            ),
        ));
    }
    push_check_blockers(context.checks, &mut blockers, &mut notes);
    push_review_blockers(context.review, &mut blockers, &mut notes);
    if context.review.requires_conversation_resolution && context.review.unresolved_threads > 0 {
        blockers.push(MergeGateBlocker::new(
            MergeGateBlockerKind::Threads,
            counted(
                context.review.unresolved_threads,
                "unresolved thread",
                "unresolved threads",
            ),
        ));
    } else if context.review.unresolved_threads > 0 {
        notes.push(format!(
            "{} do not block merging on this branch",
            counted(
                context.review.unresolved_threads,
                "unresolved thread",
                "unresolved threads"
            )
        ));
    }
    if context.merge_state.eq_ignore_ascii_case("BEHIND") {
        blockers.push(MergeGateBlocker::new(
            MergeGateBlockerKind::Branch,
            behind_summary(context.branch),
        ));
    } else if context.branch.behind_by.is_some_and(|behind| behind > 0) {
        notes.push(behind_summary(context.branch));
    }
    let awaiting: Vec<String> = context
        .checks
        .checks
        .iter()
        .filter(|check| check.awaiting_approval)
        .map(GateCheck::display_name)
        .collect();
    if !awaiting.is_empty() {
        blockers.push(
            MergeGateBlocker::new(
                MergeGateBlockerKind::Deployment,
                counted(
                    awaiting.len(),
                    "deployment is waiting for approval",
                    "deployments are waiting for approval",
                ),
            )
            .with_details(capped(awaiting)),
        );
    }
    if let Some(queue) = context.queue {
        if queue.state.eq_ignore_ascii_case("UNMERGEABLE")
            || queue.state.eq_ignore_ascii_case("LOCKED")
        {
            blockers.push(MergeGateBlocker::new(
                MergeGateBlockerKind::Queue,
                format!("the merge queue reports {}", queue.state.to_lowercase()),
            ));
        } else {
            notes.push(format!("queued to merge at position {}", queue.position));
        }
    }
    push_policy_blockers(context, &mut blockers, &mut notes);
    blockers.sort_by(|left, right| left.kind.cmp(&right.kind));
    if !blockers.is_empty() {
        return (MergeGateVerdict::Blocked, blockers, notes);
    }
    if context.mergeable.eq_ignore_ascii_case("UNKNOWN")
        || context.merge_state.eq_ignore_ascii_case("UNKNOWN")
        || context.merge_state.is_empty()
    {
        return (MergeGateVerdict::Unknown, blockers, notes);
    }
    (MergeGateVerdict::Mergeable, blockers, notes)
}

fn push_check_blockers(
    checks: &MergeGateChecks,
    blockers: &mut Vec<MergeGateBlocker>,
    notes: &mut Vec<String>,
) {
    let failing: Vec<String> = checks
        .failing()
        .filter(|check| check.required)
        .map(|check| format!("{} {}", check.display_name(), check.state.word()))
        .collect();
    let pending: Vec<String> = checks
        .pending_required()
        .map(GateCheck::display_name)
        .collect();
    let mut details = capped(failing.clone());
    details.extend(
        checks
            .missing_required
            .iter()
            .map(|context| format!("{context} never reported")),
    );
    if !details.is_empty() {
        let summary = if failing.is_empty() {
            counted(
                checks.missing_required.len(),
                "required check never reported",
                "required checks never reported",
            )
        } else {
            counted(
                failing.len(),
                "required check failed",
                "required checks failed",
            )
        };
        blockers
            .push(MergeGateBlocker::new(MergeGateBlockerKind::Ci, summary).with_details(details));
    }
    if !pending.is_empty() {
        blockers.push(
            MergeGateBlocker::new(
                MergeGateBlockerKind::Ci,
                counted(
                    pending.len(),
                    "required check has not finished",
                    "required checks have not finished",
                ),
            )
            .with_details(capped(pending)),
        );
    }
    if checks.optional_failed > 0 {
        notes.push(counted(
            checks.optional_failed,
            "optional check failed and does not block merging",
            "optional checks failed and do not block merging",
        ));
    }
}

fn push_review_blockers(
    review: &MergeGateReview,
    blockers: &mut Vec<MergeGateBlocker>,
    notes: &mut Vec<String>,
) {
    if !review.changes_requested_by.is_empty() {
        blockers.push(
            MergeGateBlocker::new(
                MergeGateBlockerKind::Review,
                counted(
                    review.changes_requested_by.len(),
                    "reviewer requested changes",
                    "reviewers requested changes",
                ),
            )
            .with_details(capped(
                review
                    .changes_requested_by
                    .iter()
                    .map(|author| format!("@{author}"))
                    .collect(),
            )),
        );
    }
    let short = review
        .required_approvals
        .saturating_sub(review.current_approvals);
    if short > 0 {
        let summary = if review.stale_approvals > 0 && review.current_approvals == 0 {
            "the latest push has not been approved".to_owned()
        } else {
            format!(
                "{} of {} approvals are in place",
                review.current_approvals, review.required_approvals
            )
        };
        let mut details = Vec::new();
        if review.stale_approvals > 0 {
            details.push(counted(
                review.stale_approvals,
                "approval applies to an older commit",
                "approvals apply to an older commit",
            ));
        }
        if review.requires_code_owner_review {
            details.push("the base branch also requires a code-owner review".to_owned());
        }
        blockers.push(
            MergeGateBlocker::new(MergeGateBlockerKind::Approval, summary).with_details(details),
        );
    } else if review.required_approvals == 0
        && review.decision.eq_ignore_ascii_case("REVIEW_REQUIRED")
    {
        blockers.push(MergeGateBlocker::new(
            MergeGateBlockerKind::Approval,
            "GitHub reports this pull request still needs a review",
        ));
    } else if review.stale_approvals > 0 {
        notes.push(counted(
            review.stale_approvals,
            "approval applies to an older commit",
            "approvals apply to an older commit",
        ));
    }
    if !review.requested_reviewers.is_empty() {
        notes.push(format!(
            "review still requested from {}",
            review
                .requested_reviewers
                .iter()
                .map(|reviewer| format!("@{reviewer}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn push_policy_blockers(
    context: &VerdictContext<'_>,
    blockers: &mut Vec<MergeGateBlocker>,
    notes: &mut Vec<String>,
) {
    if context.merge_state.eq_ignore_ascii_case("UNSTABLE") {
        notes.push(
            "GitHub reports the merge as unstable because a non-required check failed".to_owned(),
        );
    }
    if !context.merge_state.eq_ignore_ascii_case("BLOCKED") || !blockers.is_empty() {
        return;
    }
    let mut details = Vec::new();
    if context.branch.requires_linear_history {
        details.push("the base branch requires linear history".to_owned());
    }
    if context.branch.requires_signatures {
        details.push("the base branch requires signed commits".to_owned());
    }
    blockers.push(
        MergeGateBlocker::new(
            MergeGateBlockerKind::Policy,
            "GitHub blocks this merge on a rule Quinjet cannot name",
        )
        .with_details(details),
    );
}

fn behind_summary(branch: &MergeGateBranch) -> String {
    let base = display_ref(&branch.base_ref);
    branch.behind_by.map_or_else(
        || format!("the head branch is behind {base}"),
        |behind| {
            format!(
                "head is {} behind {base}",
                counted(behind, "commit", "commits")
            )
        },
    )
}

fn display_ref(base_ref: &str) -> String {
    if base_ref.is_empty() {
        "the base branch".to_owned()
    } else {
        base_ref.to_owned()
    }
}

#[doc = " A count with the wording that matches it. Both forms are spelled out"]
#[doc = " by the caller so no summary has to guess at English morphology."]
pub(super) fn counted(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn capped(mut values: Vec<String>) -> Vec<String> {
    if values.len() <= MAX_LISTED_BLOCKER_DETAILS {
        return values;
    }
    let hidden = values.len() - MAX_LISTED_BLOCKER_DETAILS;
    values.truncate(MAX_LISTED_BLOCKER_DETAILS);
    values.push(format!("and {} more", counted(hidden, "other", "others")));
    values
}
