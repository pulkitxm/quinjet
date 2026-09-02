#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Wrap replacement text in the fence GitHub renders as a suggestion, with"]
#[doc = " an optional note above it. A replacement that already contains a"]
#[doc = " closing fence would break out of the block, so it is refused rather"]
#[doc = " than escaped into something the author did not write."]
pub(crate) fn suggestion_body(replacement: &str, note: &str) -> Result<String> {
    let replacement = replacement.strip_suffix('\n').unwrap_or(replacement);
    if replacement
        .lines()
        .any(|line| line.trim_start().starts_with("```"))
    {
        bail!("a suggestion cannot contain a fenced code block");
    }
    let mut body = String::new();
    let note = note.trim();
    if !note.is_empty() {
        body.push_str(note);
        body.push_str("\n\n");
    }
    body.push_str("```");
    body.push_str(SUGGESTION_FENCE);
    body.push('\n');
    body.push_str(replacement);
    body.push('\n');
    body.push_str("```\n");
    Ok(body)
}

#[doc = " Pull every suggestion block out of a pull request's review threads. A"]
#[doc = " comment can carry more than one, and each becomes its own row keyed by"]
#[doc = " the comment plus its position in that comment."]
pub(crate) fn collect_suggestions(review: &PullRequestReviewSnapshot) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();
    for thread in &review.threads {
        for comment in &thread.comments {
            for (index, (replacement, note)) in blocks(&comment.body).into_iter().enumerate() {
                suggestions.push(Suggestion {
                    id: if index == 0 {
                        comment.id.clone()
                    } else {
                        format!("{}#{index}", comment.id)
                    },
                    thread_id: thread.id.clone(),
                    author: comment.author.clone(),
                    path: thread.path.clone(),
                    start_line: 0,
                    end_line: 0,
                    replacement,
                    comment: note,
                    url: comment.url.clone(),
                    outdated: thread.is_outdated,
                    resolved: thread.is_resolved,
                    blocker: None,
                });
                if let Some(last) = suggestions.last_mut() {
                    apply_range(last, thread);
                }
            }
        }
    }
    suggestions
}

#[doc = " A suggestion replaces the thread's line range on the new side. GitHub"]
#[doc = " reports `startLine` only for a multi-line thread, so a single-line one"]
#[doc = " replaces exactly its own line."]
fn apply_range(suggestion: &mut Suggestion, thread: &PullRequestReviewThread) {
    let Some(end) = thread.line.or(thread.original_line) else {
        suggestion.blocker = Some(SuggestionBlocker::NoLineRange);
        return;
    };
    let start = thread
        .start_line
        .or(thread.original_start_line)
        .unwrap_or(end);
    suggestion.start_line = start.min(end);
    suggestion.end_line = end.max(start);
    if suggestion.resolved {
        suggestion.blocker = Some(SuggestionBlocker::Resolved);
    } else if suggestion.outdated {
        suggestion.blocker = Some(SuggestionBlocker::Outdated);
    } else if suggestion.end_line.saturating_sub(suggestion.start_line) + 1 > MAX_SUGGESTION_LINES {
        suggestion.blocker = Some(SuggestionBlocker::NoLineRange);
    }
}

#[doc = " Every suggestion block in one comment, with the prose that precedes"]
#[doc = " it. Nesting is not a case: GitHub does not render a nested fence as a"]
#[doc = " suggestion either."]
fn blocks(body: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut note = String::new();
    let mut current: Option<Vec<&str>> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        match current.as_mut() {
            None => {
                if is_open_fence(trimmed) {
                    current = Some(Vec::new());
                } else {
                    note.push_str(line);
                    note.push('\n');
                }
            }
            Some(collected) => {
                if trimmed.starts_with("```") {
                    blocks.push((collected.join("\n"), note.trim().to_owned()));
                    note.clear();
                    current = None;
                } else {
                    collected.push(line);
                }
            }
        }
    }
    blocks
}

fn is_open_fence(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("```") else {
        return false;
    };
    rest.trim().eq_ignore_ascii_case(SUGGESTION_FENCE)
}
