#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " The review-progress record is local state rather than a GitHub read, so"]
#[doc = " these three arms never fail: a state root that cannot be written leaves"]
#[doc = " the record where it was."]
pub(super) fn record_review_visit(pull_request: &PullRequest) -> Outcome {
    let mut record =
        crate::state::load_review_progress(&pull_request.base_repository.url, pull_request.number);
    record.record_visit(&pull_request.head_oid, crate::date_time::now_timestamp());
    crate::state::record_review_progress(record);
    Outcome::Operation {
        label: "Recording review visit".to_owned(),
        changes_history: false,
        message: format!(
            "Recorded a visit to #{} at {}",
            pull_request.number,
            short_oid(&pull_request.head_oid)
        ),
    }
}

pub(super) fn mark_review_files(
    pull_request: &PullRequest,
    paths: &[PathBuf],
    viewed: bool,
) -> Outcome {
    let mut record =
        crate::state::load_review_progress(&pull_request.base_repository.url, pull_request.number);
    let mut changed = 0;
    for path in paths {
        if viewed {
            record.mark_viewed(path, &pull_request.head_oid);
            changed += 1;
        } else if record.mark_unviewed(path) {
            changed += 1;
        }
    }
    crate::state::record_review_progress(record);
    let verb = if viewed { "read" } else { "unread" };
    Outcome::Operation {
        label: "Marking reviewed files".to_owned(),
        changes_history: false,
        message: format!(
            "Marked {changed} file(s) as {verb} in #{}",
            pull_request.number
        ),
    }
}

pub(super) fn forget_review_progress(pull_request: &PullRequest) -> Outcome {
    crate::state::forget_review_progress(&pull_request.base_repository.url, pull_request.number);
    Outcome::Operation {
        label: "Clearing review progress".to_owned(),
        changes_history: false,
        message: format!("Cleared local review progress for #{}", pull_request.number),
    }
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(12).collect()
}
