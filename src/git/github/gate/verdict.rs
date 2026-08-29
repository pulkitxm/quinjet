use super::blockers::{VerdictContext, decide};
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    pub(super) fn gate_from_node(
        &self,
        pull_request: &PullRequest,
        node: GatePullRequestNode,
        cached: bool,
    ) -> MergeGate {
        let base_target = node
            .base_ref
            .as_ref()
            .and_then(|base| base.target.as_ref())
            .map(|target| target.oid.clone())
            .unwrap_or_default();
        let head = if node.head_ref_oid.is_empty() {
            pull_request.head_oid.clone()
        } else {
            node.head_ref_oid.clone()
        };
        let behind = if node.merged || node.state.eq_ignore_ascii_case("CLOSED") {
            Some(0)
        } else {
            self.commits_behind_base(&pull_request.base_repository, &head, &base_target)
        };
        let mut gate = build_gate(
            &pull_request.base_repository.name_with_owner,
            node,
            behind,
            base_target,
        );
        gate.from_cache = cached;
        gate
    }
}

#[doc = " The verdict is pure: the same inputs always produce the same blockers,"]
#[doc = " in the same order, for a human, a shell, an editor, and a coding tool."]
pub(super) fn build_gate(
    repository: &str,
    node: GatePullRequestNode,
    behind_by: Option<usize>,
    base_target: String,
) -> MergeGate {
    let head_oid = node.head_ref_oid.clone();
    let head_for_branch = head_oid.clone();
    let merge_state = node.merge_state_status.clone().unwrap_or_default();
    let mergeable = node.mergeable.clone().unwrap_or_default();
    let rule = node
        .base_ref
        .as_ref()
        .and_then(|base| base.ref_update_rule.as_ref());
    let checks = collect_checks(&node, rule);
    let review = collect_review(&node, &head_oid, rule);
    let branch = MergeGateBranch {
        base_ref: node.base_ref_name.clone(),
        base_oid: if base_target.is_empty() {
            node.base_ref_oid.clone()
        } else {
            base_target
        },
        head_oid: head_for_branch,
        merge_state: merge_state.clone(),
        mergeable: mergeable.clone(),
        behind_by,
        requires_linear_history: rule.is_some_and(|rule| rule.requires_linear_history),
        requires_signatures: rule.is_some_and(|rule| rule.requires_signatures),
    };
    let queue = node.merge_queue_entry.as_ref().map(|entry| MergeGateQueue {
        state: entry.state.clone(),
        position: entry.position,
        enqueued: true,
    });
    let auto_merge = node
        .auto_merge_request
        .as_ref()
        .map(|auto| MergeGateAutoMerge {
            enabled: true,
            method: auto.merge_method.clone().unwrap_or_default(),
            enabled_by: auto
                .enabled_by
                .as_ref()
                .and_then(GateActor::display)
                .unwrap_or_default(),
        })
        .unwrap_or_default();
    let mut warnings = Vec::new();
    if rule.is_none() {
        warnings.push(format!(
            "Quinjet could not read branch rules for `{}`; required checks and approvals are inferred from the pull request alone",
            node.base_ref_name
        ));
    }
    if checks.truncated {
        warnings.push("the head commit reports more checks than one page holds".to_owned());
    }
    if review.threads_truncated {
        warnings.push("this pull request has more review threads than one page holds".to_owned());
    }
    let context = VerdictContext {
        merged: node.merged,
        state: node.state.clone(),
        is_draft: node.is_draft,
        merge_state: &merge_state,
        mergeable: &mergeable,
        checks: &checks,
        review: &review,
        branch: &branch,
        queue: queue.as_ref(),
    };
    let (verdict, blockers, notes) = decide(&context);
    warnings.extend(notes);
    MergeGate {
        schema_version: MergeGate::SCHEMA_VERSION,
        repository: repository.to_owned(),
        number: node.number,
        title: node.title,
        url: node.url,
        state: node.state,
        is_draft: node.is_draft,
        verdict,
        blockers,
        checks,
        review,
        branch,
        queue,
        auto_merge,
        warnings,
        from_cache: false,
    }
}

fn collect_checks(node: &GatePullRequestNode, rule: Option<&GateRefUpdateRule>) -> MergeGateChecks {
    let contexts = node
        .commits
        .as_ref()
        .and_then(|commits| commits.nodes.first())
        .and_then(Option::as_ref)
        .and_then(|entry| entry.commit.as_ref())
        .and_then(|commit| commit.status_check_rollup.as_ref())
        .and_then(|rollup| rollup.contexts.as_ref());
    let mut checks = MergeGateChecks {
        truncated: contexts.is_some_and(|contexts| {
            contexts.page_info.has_next_page || contexts.total_count > contexts.nodes.len()
        }),
        ..MergeGateChecks::default()
    };
    for entry in contexts
        .into_iter()
        .flat_map(|contexts| contexts.nodes.iter())
        .flatten()
    {
        checks.checks.push(gate_check(entry));
    }
    checks.checks.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| left.display_name().cmp(&right.display_name()))
    });
    for check in &checks.checks {
        if check.required {
            checks.required_total += 1;
            match check.state {
                GateCheckState::Passed | GateCheckState::Skipped => checks.required_passed += 1,
                GateCheckState::Failed => checks.required_failed += 1,
                GateCheckState::Pending | GateCheckState::Unknown => checks.required_pending += 1,
            }
        } else if check.state == GateCheckState::Failed {
            checks.optional_failed += 1;
        }
    }
    checks.missing_required =
        rule.and_then(|rule| rule.required_status_check_contexts.as_ref())
            .map(|wanted| {
                wanted
                    .iter()
                    .flatten()
                    .filter(|context| {
                        !checks.checks.iter().any(|check| {
                            &&check.name == context || &&check.display_name() == context
                        })
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
    checks
}

fn gate_check(entry: &GateContextNode) -> GateCheck {
    if entry.typename == "StatusContext" {
        let name = entry.context.clone().unwrap_or_default();
        return GateCheck {
            name,
            workflow: String::new(),
            state: GateCheckState::from_status_context(entry.state.as_deref().unwrap_or_default()),
            required: entry.is_required,
            url: entry.target_url.clone().unwrap_or_default(),
            awaiting_approval: false,
        };
    }
    let status = entry.status.clone().unwrap_or_default();
    let awaiting_approval = status.eq_ignore_ascii_case("WAITING")
        || entry
            .conclusion
            .as_deref()
            .is_some_and(|conclusion| conclusion.eq_ignore_ascii_case("ACTION_REQUIRED"));
    GateCheck {
        name: entry.name.clone().unwrap_or_default(),
        workflow: entry
            .check_suite
            .as_ref()
            .and_then(|suite| suite.workflow_run.as_ref())
            .and_then(|run| run.workflow.as_ref())
            .map(|workflow| workflow.name.clone())
            .unwrap_or_default(),
        state: GateCheckState::from_rollup(
            &status,
            entry.conclusion.as_deref().unwrap_or_default(),
        ),
        required: entry.is_required,
        url: entry.details_url.clone().unwrap_or_default(),
        awaiting_approval,
    }
}

fn collect_review(
    node: &GatePullRequestNode,
    head_oid: &str,
    rule: Option<&GateRefUpdateRule>,
) -> MergeGateReview {
    let mut review = MergeGateReview {
        decision: node.review_decision.clone().unwrap_or_default(),
        required_approvals: rule
            .and_then(|rule| rule.required_approving_review_count)
            .unwrap_or_default(),
        requires_code_owner_review: rule.is_some_and(|rule| rule.requires_code_owner_reviews),
        requires_conversation_resolution: rule
            .is_some_and(|rule| rule.requires_conversation_resolution),
        ..MergeGateReview::default()
    };
    for entry in node
        .latest_opinionated_reviews
        .iter()
        .flat_map(|reviews| reviews.nodes.iter())
        .flatten()
    {
        let commit_oid = entry
            .commit
            .as_ref()
            .map(|commit| commit.oid.clone())
            .unwrap_or_default();
        let stale = !commit_oid.is_empty() && !head_oid.is_empty() && commit_oid != head_oid;
        let author = entry
            .author
            .as_ref()
            .and_then(GateActor::display)
            .unwrap_or_else(|| "ghost".to_owned());
        if entry.state.eq_ignore_ascii_case("APPROVED") {
            review.approvals += 1;
            if stale {
                review.stale_approvals += 1;
            } else {
                review.current_approvals += 1;
            }
        } else if entry.state.eq_ignore_ascii_case("CHANGES_REQUESTED") {
            review.changes_requested_by.push(author.clone());
        }
        review.reviews.push(GateReview {
            author,
            state: entry.state.clone(),
            commit_oid,
            stale,
        });
    }
    review.reviews.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| left.author.cmp(&right.author))
    });
    review.changes_requested_by.sort_unstable();
    review.requested_reviewers = node
        .review_requests
        .iter()
        .flat_map(|requests| requests.nodes.iter())
        .flatten()
        .filter_map(|request| request.requested_reviewer.as_ref())
        .filter_map(GateActor::display)
        .collect();
    review.requested_reviewers.sort_unstable();
    if let Some(threads) = &node.review_threads {
        review.threads_truncated =
            threads.page_info.has_next_page || threads.total_count > threads.nodes.len();
        for thread in threads.nodes.iter().flatten() {
            if !thread.is_resolved {
                review.unresolved_threads += 1;
                if thread.is_outdated {
                    review.outdated_unresolved_threads += 1;
                }
            }
        }
    }
    review
}
