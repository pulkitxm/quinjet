use super::feedback::feedback_row;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn stack_review(review: &StackReview) -> String {
    let mut out = Report::default();
    for member in &review.members {
        out.line(&member_row(member));
    }
    if out.empty() {
        out.line("No stack members were readable");
        for warning in &review.warnings {
            out.line(&format!("note  {warning}"));
        }
        return out.finish();
    }
    out.blank();
    out.line(&merge_order_line(review));
    if let Some(member) = review.critical_member() {
        out.line(&format!(
            "critical  position {} (#{}) {}",
            member.position,
            member.number,
            member.headline()
        ));
        if review.critical_path.len() > 1 {
            out.line(&format!(
                "          holding up {}",
                counted(review.critical_path.len() - 1, "member", "members")
            ));
        }
    }
    if let Some(failure) = &review.earliest_failing_check {
        out.line(&format!(
            "first red position {} (#{}) {}",
            failure.position, failure.number, failure.check
        ));
    }
    if review.stale_approvals > 0 {
        out.line(&stale_line(review));
    }
    if review.unresolved_threads > 0 {
        out.line(&format!(
            "threads   {}",
            counted(review.unresolved_threads, "unresolved", "unresolved")
        ));
    }
    if !review.duplicated_paths.is_empty() {
        out.blank();
        out.line("touched by more than one member");
        for duplicate in &review.duplicated_paths {
            out.line(&format!(
                "  {:<48} positions {}",
                truncate(&duplicate.path.display().to_string(), 48),
                positions(&duplicate.positions)
            ));
        }
    }
    if review.truncated {
        out.line("[the stack reached Quinjet's size cap]");
    }
    for warning in &review.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}

fn member_row(member: &StackReviewMember) -> String {
    format!(
        "{}{:<3} {:<10} {:<10} #{:<6} +{} -{}  {}",
        if member.selected { ">" } else { " " },
        member.position,
        member.verdict.word(),
        member.block_source.word(),
        member.number,
        member.additions,
        member.deletions,
        truncate(&member.headline(), 52)
    )
}

fn merge_order_line(review: &StackReview) -> String {
    if review.merge_order.is_empty() {
        return String::from("merge     nothing can merge yet");
    }
    let mut line = format!("merge     {}", positions(&review.merge_order));
    if review.merge_order.len() < review.members.len() {
        line.push_str(", then stop");
    }
    line
}

fn stale_line(review: &StackReview) -> String {
    let mut line = format!(
        "stale     {} invalidated by a later push",
        counted(review.stale_approvals, "approval", "approvals")
    );
    let reviewers: Vec<&str> = review
        .members
        .iter()
        .flat_map(|member| &member.stale_approvals)
        .map(|approval| approval.reviewer.as_str())
        .collect();
    if !reviewers.is_empty() {
        line.push_str(" (");
        line.push_str(&reviewers.join(", "));
        line.push(')');
    }
    line
}

#[doc = " `1 approval` but `2 approvals`. Both forms are given rather than"]
#[doc = " derived, because a rule that appends an s is wrong often enough to"]
#[doc = " be worse than saying what you mean."]
fn counted(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn positions(positions: &[usize]) -> String {
    positions
        .iter()
        .map(usize::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}

pub(crate) fn stack_feedback(queue: &StackFeedback) -> String {
    let mut out = Report::default();
    for member in &queue.members {
        if member.items.is_empty() {
            continue;
        }
        out.line(&format!(
            "{}{} #{} {}",
            if member.selected { "> " } else { "  " },
            member.position,
            member.number,
            truncate(&member.title, 56)
        ));
        for item in &member.items {
            out.line(&format!("  {}", feedback_row(item)));
        }
    }
    if out.empty() {
        out.line("Nothing outstanding across the stack");
    } else {
        out.line(&format!(
            "\n{} blocking, {} advisory · {} on you, {} on others",
            queue.counts.blocking,
            queue.counts.advisory,
            queue.counts.awaiting_you,
            queue.counts.awaiting_others
        ));
    }
    if let Some((position, item)) = queue.next_blocker() {
        out.line(&format!(
            "next  position {position} #{} {} {}",
            queue
                .members
                .iter()
                .find(|member| member.position == position)
                .map_or(0, |member| member.number),
            item.kind.word(),
            if item.location().is_empty() {
                format!("@{}", item.author)
            } else {
                item.location()
            }
        ));
    }
    if queue.truncated {
        out.line("[the stack reached Quinjet's size cap]");
    }
    for warning in &queue.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}
