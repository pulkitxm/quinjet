#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Everything the queue combines. Fetching happens in the caller so the"]
#[doc = " reduction stays pure and two callers cannot disagree about what is"]
#[doc = " outstanding."]
pub(crate) struct FeedbackInputs<'a> {
    pub pull_request: &'a PullRequest,
    pub viewer: &'a str,
    pub gate: Option<&'a MergeGate>,
    pub review: &'a PullRequestReviewSnapshot,
    pub annotations: Option<&'a PullRequestAnnotations>,
    pub warnings: Vec<String>,
}

#[doc = " One queue out of the conversation, the review threads and CI, so an"]
#[doc = " author does not have to read three views to find what is outstanding."]
pub(crate) fn build_feedback(inputs: &FeedbackInputs<'_>) -> PullRequestFeedback {
    let mut feedback = PullRequestFeedback {
        number: inputs.pull_request.number,
        head_oid: inputs.pull_request.head_oid.clone(),
        viewer: inputs.viewer.to_owned(),
        truncated: inputs.review.truncated,
        warnings: inputs.warnings.clone(),
        ..PullRequestFeedback::default()
    };
    if let Some(gate) = inputs.gate {
        feedback
            .items
            .extend(changes_requested(gate, inputs.viewer));
        feedback.truncated |= gate.review.threads_truncated;
    }
    for thread in inputs
        .review
        .threads
        .iter()
        .filter(|thread| !thread.is_resolved)
    {
        feedback.items.push(thread_item(thread, inputs.viewer));
    }
    if let Some(annotations) = inputs.annotations {
        feedback.truncated |= annotations.truncated;
        for annotation in &annotations.annotations {
            feedback.items.push(annotation_item(annotation));
        }
    }
    feedback.finish();
    feedback
}

#[doc = " A reviewer who asked for changes is one row, not one per comment: the"]
#[doc = " verdict is what stands between the pull request and merging, and the"]
#[doc = " comments behind it are their own rows already."]
fn changes_requested(gate: &MergeGate, viewer: &str) -> Vec<FeedbackItem> {
    let detail = gate.blocker(MergeGateBlockerKind::Review).map_or_else(
        || "requested changes".to_owned(),
        |blocker| blocker.summary.clone(),
    );
    gate.review
        .changes_requested_by
        .iter()
        .map(|author| FeedbackItem {
            kind: FeedbackKind::ChangesRequested,
            id: author.clone(),
            path: None,
            line: None,
            author: author.clone(),
            summary: detail.clone(),
            body: String::new(),
            url: gate.url.clone(),
            owner: if author.eq_ignore_ascii_case(viewer) {
                FeedbackOwner::Others
            } else {
                FeedbackOwner::You
            },
            mine: author.eq_ignore_ascii_case(viewer),
            action: format!("address the review, then ask @{author} to look again"),
        })
        .collect()
}

fn thread_item(thread: &PullRequestReviewThread, viewer: &str) -> FeedbackItem {
    let newest = thread.comments.last();
    let mine = newest.is_some_and(|comment| {
        comment.viewer_did_author || comment.author.eq_ignore_ascii_case(viewer)
    });
    let kind = if thread.is_outdated {
        FeedbackKind::OutdatedThread
    } else {
        FeedbackKind::Thread
    };
    FeedbackItem {
        kind,
        id: thread.id.clone(),
        path: Some(thread.path.clone()),
        line: thread.line.or(thread.original_line),
        author: newest
            .map(|comment| comment.author.clone())
            .unwrap_or_default(),
        summary: newest
            .map(|comment| excerpt(&comment.body))
            .unwrap_or_default(),
        body: newest
            .map(|comment| comment.body.clone())
            .unwrap_or_default(),
        url: newest
            .map(|comment| comment.url.clone())
            .unwrap_or_default(),
        owner: if mine {
            FeedbackOwner::Others
        } else {
            FeedbackOwner::You
        },
        mine,
        action: thread_action(thread, mine),
    }
}

fn thread_action(thread: &PullRequestReviewThread, mine: bool) -> String {
    if thread.is_outdated {
        return format!(
            "the code moved; resolve with `quinjet pr reviews resolve <n> {}`",
            thread.id
        );
    }
    if mine {
        return "waiting on a reply from somebody else".to_owned();
    }
    format!(
        "reply with `quinjet pr reviews reply <n> {} --body ...`",
        thread.id
    )
}

fn annotation_item(annotation: &CheckAnnotation) -> FeedbackItem {
    let kind = if annotation.severity == AnnotationSeverity::Failure {
        FeedbackKind::Failure
    } else {
        FeedbackKind::Advisory
    };
    FeedbackItem {
        kind,
        id: annotation.check_run_id.to_string(),
        path: Some(annotation.path.clone()),
        line: (annotation.start_line > 0).then_some(annotation.start_line),
        author: annotation.check.clone(),
        summary: annotation.headline(),
        body: annotation.message.clone(),
        url: if annotation.url.is_empty() {
            annotation.check_url.clone()
        } else {
            annotation.url.clone()
        },
        owner: FeedbackOwner::Nobody,
        mine: false,
        action: format!(
            "read the log with `quinjet pr logs <n> \"{}\"`",
            annotation.check
        ),
    }
}

fn excerpt(body: &str) -> String {
    let first = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if first.chars().count() <= MAX_EXCERPT_CHARS {
        return first.to_owned();
    }
    let kept: String = first.chars().take(MAX_EXCERPT_CHARS - 1).collect();
    format!("{kept}…")
}
