#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn merge_gate(gate: &MergeGate) -> String {
    let mut out = Report::default();
    out.line(&format!(
        "{}  #{}  {}",
        gate.verdict.word(),
        gate.number,
        gate.title
    ));
    for blocker in &gate.blockers {
        out.line(&format!("  {}: {}", blocker.kind.label(), blocker.summary));
        for detail in &blocker.details {
            out.line(&format!("      {detail}"));
        }
    }
    out.blank();
    out.line(&gate_checks_line(&gate.checks));
    out.line(&gate_review_line(&gate.review));
    out.line(&gate_branch_line(&gate.branch));
    if let Some(queue) = &gate.queue {
        out.line(&format!(
            "queue     position {} ({})",
            queue.position,
            queue.state.to_lowercase()
        ));
    }
    if gate.auto_merge.enabled {
        out.line(&format!(
            "auto      {} enabled by @{}",
            gate.auto_merge.method.to_lowercase(),
            gate.auto_merge.enabled_by
        ));
    }
    for warning in &gate.warnings {
        out.line(&format!("note      {warning}"));
    }
    if gate.from_cache {
        out.line("note      answered from the cache; pass --refresh to ask GitHub again");
    }
    out.finish()
}

fn gate_checks_line(checks: &MergeGateChecks) -> String {
    let mut line = format!(
        "checks    {} of {} required passed",
        checks.required_passed, checks.required_total
    );
    for (count, word) in [
        (checks.required_failed, "failed"),
        (checks.required_pending, "pending"),
        (checks.optional_failed, "optional failed"),
    ] {
        if count > 0 {
            line.push_str(", ");
            line.push_str(&count.to_string());
            line.push(' ');
            line.push_str(word);
        }
    }
    line
}

fn gate_review_line(review: &MergeGateReview) -> String {
    let decision = if review.decision.is_empty() {
        "none".to_owned()
    } else {
        review.decision.to_lowercase()
    };
    let mut line = format!(
        "review    {decision}, {} of {} approvals",
        review.current_approvals, review.required_approvals
    );
    for (count, word) in [
        (review.stale_approvals, "stale"),
        (review.unresolved_threads, "unresolved"),
    ] {
        if count > 0 {
            line.push_str(", ");
            line.push_str(&count.to_string());
            line.push(' ');
            line.push_str(word);
        }
    }
    if !review.requested_reviewers.is_empty() {
        line.push_str(", requested from ");
        line.push_str(&review.requested_reviewers.join(", "));
    }
    line
}

fn gate_branch_line(branch: &MergeGateBranch) -> String {
    let base = if branch.base_ref.is_empty() {
        "the base branch"
    } else {
        branch.base_ref.as_str()
    };
    let behind = branch.behind_by.map_or_else(
        || " (freshness unknown)".to_owned(),
        |behind| {
            if behind == 0 {
                " (up to date)".to_owned()
            } else {
                format!(" ({behind} behind)")
            }
        },
    );
    format!(
        "branch    {base}{behind}, {} / {}",
        branch.merge_state.to_lowercase(),
        branch.mergeable.to_lowercase()
    )
}

pub(crate) fn stack_gate(gate: &StackGate) -> String {
    let mut out = Report::default();
    out.line(&format!(
        "{}  stack #{}  {} layers  destination {}",
        gate.verdict.word(),
        gate.number,
        gate.size,
        gate.base_ref
    ));
    for member in gate.members.iter().rev() {
        let marker = if member.selected { ">" } else { " " };
        out.line(&format!(
            "{marker} {:>3}  #{:<7} {:<10} {}",
            member.position,
            member.number,
            member.gate.verdict.word(),
            truncate(&member.title, 44)
        ));
        for blocker in &member.gate.blockers {
            out.line(&format!(
                "         {}: {}",
                blocker.kind.label(),
                blocker.summary
            ));
        }
    }
    out.blank();
    if gate.mergeable_prefix.is_empty() {
        out.line("merge order  nothing in this stack can merge yet");
    } else {
        out.line(&format!(
            "merge order  positions {} can merge now, bottom first",
            gate.mergeable_prefix
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(member) = gate.critical_member() {
        out.line(&format!(
            "critical     position {} (#{}) {}",
            member.position,
            member.number,
            member.gate.headline()
        ));
    }
    if gate.truncated {
        out.line("note         the stack response was incomplete");
    }
    for warning in &gate.warnings {
        out.line(&format!("note         {warning}"));
    }
    out.finish()
}
