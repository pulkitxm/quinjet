#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn feedback(queue: &PullRequestFeedback, full: bool) -> String {
    let mut out = Report::default();
    if queue.items.is_empty() {
        out.line("Nothing outstanding");
        for warning in &queue.warnings {
            out.line(&format!("note  {warning}"));
        }
        return out.finish();
    }
    for item in &queue.items {
        out.line(&feedback_row(item));
        if full && !item.body.trim().is_empty() {
            for line in item.body.trim_end().lines() {
                out.line(&format!("      {line}"));
            }
        }
        if full {
            out.line(&format!("      -> {}", item.action));
        }
    }
    out.line(&format!(
        "\n{} blocking, {} advisory · {} on you, {} on others",
        queue.counts.blocking,
        queue.counts.advisory,
        queue.counts.awaiting_you,
        queue.counts.awaiting_others
    ));
    if let Some(next) = queue.next_blocker() {
        out.line(&format!(
            "next  {} {}",
            next.kind.word(),
            if next.location().is_empty() {
                format!("@{}", next.author)
            } else {
                next.location()
            }
        ));
    }
    if queue.truncated {
        out.line("[the feedback queue reached Quinjet's size cap]");
    }
    for warning in &queue.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}

fn feedback_row(item: &FeedbackItem) -> String {
    let location = item.location();
    let location = if location.is_empty() {
        format!("@{}", item.author)
    } else {
        location
    };
    format!(
        "{:<9} {:<7} {:<32} {}",
        item.kind.word(),
        item.owner.word(),
        truncate(&location, 32),
        truncate(&item.summary, 60)
    )
}

pub(crate) fn suggestions(listing: &PullRequestSuggestions) -> String {
    let mut out = Report::default();
    if listing.suggestions.is_empty() {
        out.line("No suggested changes reported");
        return out.finish();
    }
    for suggestion in &listing.suggestions {
        let (removed, added) = suggestion.counts();
        let state = suggestion
            .blocker
            .as_ref()
            .map_or_else(|| "ready".to_owned(), SuggestionBlocker::message);
        out.line(&format!(
            "{:<24} {:<28} +{added} -{removed}  @{}  {state}",
            truncate(&suggestion.id, 24),
            truncate(&suggestion.location(), 28),
            suggestion.author
        ));
    }
    out.line(&format!(
        "\n{} ready to apply, {} blocked",
        listing.applicable, listing.blocked
    ));
    if listing.truncated {
        out.line("[the review reached Quinjet's size cap]");
    }
    for warning in &listing.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}
