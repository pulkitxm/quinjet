#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Everything a progress reading needs that Quinjet had to fetch, kept"]
#[doc = " apart from the reduction so the reduction stays pure and testable."]
pub(crate) struct ReviewProgressInputs<'a> {
    pub repository: &'a str,
    pub number: u64,
    pub head_oid: &'a str,
    pub since: ReviewSince,
    pub record: &'a ReviewProgressRecord,
    pub index: &'a PullRequestDiffIndex,
    pub review: &'a PullRequestReviewSnapshot,
    pub commits: &'a PullRequestCommits,
    #[doc = " Paths changed between the `since` commit and the head."]
    pub changed_since: Option<&'a BTreeSet<PathBuf>>,
    #[doc = " Per viewed commit, the paths it changed on the way to the head."]
    pub changed_since_viewed: &'a [(String, BTreeSet<PathBuf>)],
    pub warnings: Vec<String>,
}

#[doc = " Reduce the fetched pieces to one reading. This is a pure function so"]
#[doc = " that the same pull request always produces the same next step."]
pub(crate) fn build_progress(inputs: ReviewProgressInputs<'_>) -> ReviewProgress {
    let files = collect_files(&inputs);
    let viewed = files
        .iter()
        .filter(|file| file.state == ReviewFileState::Viewed)
        .count();
    let remaining = files
        .iter()
        .filter(|file| file.state.is_remaining())
        .count();
    let changed_since_viewed = files
        .iter()
        .filter(|file| file.state == ReviewFileState::ChangedSinceViewed)
        .count();
    let changed_since = files.iter().filter(|file| file.changed_since).count();
    let threads = collect_threads(inputs.review);
    let new_commits = commits_after(inputs.commits, &inputs.since.oid);
    let thread_step = next_thread(inputs.review);
    let next = next_step(&files, thread_step.clone());
    ReviewProgress {
        schema_version: ReviewProgress::SCHEMA_VERSION,
        repository: inputs.repository.to_owned(),
        number: inputs.number,
        head_oid: inputs.head_oid.to_owned(),
        since: inputs.since,
        visited_at: inputs.record.visited_at.clone(),
        files,
        viewed,
        remaining,
        changed_since_viewed,
        changed_since,
        new_commits,
        threads,
        next,
        thread_step,
        truncated: inputs.index.truncated || inputs.review.truncated,
        warnings: inputs.warnings,
    }
}

fn collect_files(inputs: &ReviewProgressInputs<'_>) -> Vec<ReviewFileProgress> {
    inputs
        .index
        .files
        .iter()
        .map(|file| {
            let viewed_at = inputs.record.viewed_at(&file.path).unwrap_or_default();
            ReviewFileProgress {
                path: file.path.clone(),
                status: file.status,
                state: file_state(viewed_at, inputs.head_oid, &file.path, inputs),
                viewed_at_oid: viewed_at.to_owned(),
                changed_since: inputs
                    .changed_since
                    .is_some_and(|changed| changed.contains(&file.path)),
            }
        })
        .collect()
}

fn file_state(
    viewed_at: &str,
    head_oid: &str,
    path: &Path,
    inputs: &ReviewProgressInputs<'_>,
) -> ReviewFileState {
    if viewed_at.is_empty() {
        return ReviewFileState::Unviewed;
    }
    if viewed_at == head_oid {
        return ReviewFileState::Viewed;
    }
    let Some((_, changed)) = inputs
        .changed_since_viewed
        .iter()
        .find(|(oid, _)| oid == viewed_at)
    else {
        return ReviewFileState::ViewedAtUnknownCommit;
    };
    if changed.contains(path) {
        ReviewFileState::ChangedSinceViewed
    } else {
        ReviewFileState::Viewed
    }
}

fn collect_threads(review: &PullRequestReviewSnapshot) -> ReviewThreadProgress {
    let mut progress = ReviewThreadProgress {
        total: review.threads.len(),
        ..ReviewThreadProgress::default()
    };
    for thread in review.threads.iter().filter(|thread| !thread.is_resolved) {
        progress.unresolved += 1;
        if thread.is_outdated {
            progress.outdated_unresolved += 1;
        }
        match thread.comments.last() {
            None => {}
            Some(comment) if comment.viewer_did_author => progress.awaiting_others += 1,
            Some(_) => progress.awaiting_your_reply += 1,
        }
    }
    progress
}

#[doc = " The commits a pull request gained after the `since` commit. The commit"]
#[doc = " list is oldest first, so everything after the match is newer; a `since`"]
#[doc = " the list does not hold means every commit is new to this reader."]
fn commits_after(commits: &PullRequestCommits, since: &str) -> Vec<PullRequestCommit> {
    if since.is_empty() {
        return Vec::new();
    }
    commits
        .commits
        .iter()
        .position(|commit| commit.oid == since)
        .map_or_else(
            || commits.commits.clone(),
            |index| commits.commits.iter().skip(index + 1).cloned().collect(),
        )
}

#[doc = " Files come before threads, and within files a file that changed under"]
#[doc = " the reviewer comes before one never read: re-reading what moved is more"]
#[doc = " urgent than starting something new."]
fn next_step(
    files: &[ReviewFileProgress],
    thread_step: Option<ReviewNextStep>,
) -> Option<ReviewNextStep> {
    let priority = [
        ReviewFileState::ChangedSinceViewed,
        ReviewFileState::ViewedAtUnknownCommit,
        ReviewFileState::Unviewed,
    ];
    for state in priority {
        if let Some(file) = files.iter().find(|file| file.state == state) {
            return Some(ReviewNextStep::File {
                path: file.path.clone(),
                state: file.state,
            });
        }
    }
    thread_step
}

fn next_thread(review: &PullRequestReviewSnapshot) -> Option<ReviewNextStep> {
    let thread = review
        .threads
        .iter()
        .filter(|thread| !thread.is_resolved)
        .find(|thread| {
            thread
                .comments
                .last()
                .is_none_or(|comment| !comment.viewer_did_author)
        })
        .or_else(|| review.threads.iter().find(|thread| !thread.is_resolved))?;
    Some(thread_step(thread))
}

fn thread_step(thread: &PullRequestReviewThread) -> ReviewNextStep {
    let comment = thread.comments.last();
    ReviewNextStep::Thread {
        id: thread.id.clone(),
        path: thread.path.clone(),
        line: thread.line.or(thread.original_line),
        outdated: thread.is_outdated,
        author: comment
            .map(|comment| comment.author.clone())
            .unwrap_or_default(),
        excerpt: comment
            .map(|comment| excerpt(&comment.body))
            .unwrap_or_default(),
    }
}

#[doc = " One line of a comment, so a queue row stays a row."]
fn excerpt(body: &str) -> String {
    let first = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if first.chars().count() <= 72 {
        return first.to_owned();
    }
    let kept: String = first.chars().take(71).collect();
    format!("{kept}…")
}
