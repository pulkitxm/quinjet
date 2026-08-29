#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Reduce the pieces a caller already fetched into one queue. The fetches"]
#[doc = " stay in the caller so the reduction is pure and cannot disagree with"]
#[doc = " what the gate and the annotations said on their own."]
pub(super) fn feedback(
    pull_request: &PullRequest,
    gate: Option<&MergeGate>,
    review: &PullRequestReviewSnapshot,
    annotations: Option<&PullRequestAnnotations>,
    viewer: &str,
) -> Outcome {
    Outcome::Feedback(Box::new(build_feedback(&FeedbackInputs {
        pull_request,
        viewer,
        gate,
        review,
        annotations,
        warnings: Vec::new(),
    })))
}

pub(super) fn suggestions(
    pull_request: &PullRequest,
    review: &PullRequestReviewSnapshot,
) -> Outcome {
    let mut listing = PullRequestSuggestions {
        number: pull_request.number,
        head_oid: pull_request.head_oid.clone(),
        suggestions: collect_suggestions(review),
        truncated: review.truncated,
        ..PullRequestSuggestions::default()
    };
    listing.finish();
    Outcome::Suggestions(Box::new(listing))
}

#[doc = " Write a plan and, when a message is given, record exactly the files it"]
#[doc = " touched. The checkout is checked first: a suggestion's line numbers"]
#[doc = " only mean something against the commit it was written for."]
pub(super) fn apply(
    repository: &Repository,
    pull_request: &PullRequest,
    plan: &SuggestionPlan,
    message: Option<&str>,
) -> Result<Outcome> {
    let paths: Vec<std::path::PathBuf> = plan.files.iter().map(|file| file.path.clone()).collect();
    repository.ensure_suggestions_apply_cleanly(pull_request, &paths)?;
    repository.write_suggestion_plan(plan)?;
    let committed = match message {
        None => String::new(),
        Some(message) => {
            repository.commit_suggestion_paths(&paths, message)?;
            " and committed them".to_owned()
        }
    };
    Ok(Outcome::Operation {
        label: "Applying suggested changes".to_owned(),
        changes_history: message.is_some(),
        message: format!("Applied {}{committed}", plan.summary()),
    })
}
