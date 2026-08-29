#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Where the `since` commit came from, so a reader knows whether the delta"]
#[doc = " is measured from their own last visit, their last review on GitHub, or"]
#[doc = " the whole pull request because there is nothing earlier to measure from."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ReviewSinceSource {
    Visit,
    Review,
    Explicit,
    MergeBase,
}

impl ReviewSinceSource {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Visit => "your last visit",
            Self::Review => "your last review",
            Self::Explicit => "the commit you named",
            Self::MergeBase => "the merge base",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewSince {
    pub oid: String,
    pub source: ReviewSinceSource,
    #[doc = " When the source is a review, the state that review carried."]
    pub detail: String,
}

#[doc = " What a reviewer still owes one file."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ReviewFileState {
    Unviewed,
    Viewed,
    #[doc = " Marked as read, then changed by a later commit."]
    ChangedSinceViewed,
    #[doc = " Marked as read at a commit Quinjet could not compare against, so"]
    #[doc = " whether it changed since is unknown and it counts as remaining."]
    ViewedAtUnknownCommit,
}

impl ReviewFileState {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Unviewed => "unviewed",
            Self::Viewed => "viewed",
            Self::ChangedSinceViewed => "changed",
            Self::ViewedAtUnknownCommit => "unknown",
        }
    }

    #[doc = " A file counts as remaining unless it is viewed at the current head or"]
    #[doc = " provably unchanged since it was read. Anything uncertain is work."]
    pub(crate) const fn is_remaining(self) -> bool {
        !matches!(self, Self::Viewed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewFileProgress {
    pub path: PathBuf,
    pub status: PullRequestFileStatus,
    pub state: ReviewFileState,
    pub viewed_at_oid: String,
    #[doc = " The file is part of the delta since the `since` commit."]
    pub changed_since: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewThreadProgress {
    pub total: usize,
    pub unresolved: usize,
    #[doc = " Unresolved threads a later commit made outdated, which is where a"]
    #[doc = " comment about code that no longer exists hides."]
    pub outdated_unresolved: usize,
    #[doc = " Unresolved threads whose newest comment is not yours."]
    pub awaiting_your_reply: usize,
    #[doc = " Unresolved threads whose newest comment is yours."]
    pub awaiting_others: usize,
}

#[doc = " The one thing to look at next, in the order a reviewer would work."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum ReviewNextStep {
    File {
        path: PathBuf,
        state: ReviewFileState,
    },
    Thread {
        id: String,
        path: PathBuf,
        line: Option<usize>,
        outdated: bool,
        author: String,
        excerpt: String,
    },
}

impl ReviewNextStep {
    pub(crate) fn summary(&self) -> String {
        match self {
            Self::File { path, state } => {
                format!("{} {}", state.word(), path.display())
            }
            Self::Thread {
                path, line, author, ..
            } => {
                let line = line.map_or_else(|| "file".to_owned(), |line| line.to_string());
                format!("thread {}:{line} from @{author}", path.display())
            }
        }
    }
}

#[doc = " What is left to review, measured against a commit rather than against"]
#[doc = " the whole pull request."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewProgress {
    pub schema_version: u8,
    pub repository: String,
    pub number: u64,
    pub head_oid: String,
    pub since: ReviewSince,
    pub visited_at: String,
    pub files: Vec<ReviewFileProgress>,
    pub viewed: usize,
    pub remaining: usize,
    pub changed_since_viewed: usize,
    pub changed_since: usize,
    pub new_commits: Vec<PullRequestCommit>,
    pub threads: ReviewThreadProgress,
    pub next: Option<ReviewNextStep>,
    pub truncated: bool,
    pub warnings: Vec<String>,
    #[doc = " The next unresolved thread, kept beside `next` so a caller asking"]
    #[doc = " only for threads does not have to re-read the review."]
    pub thread_step: Option<ReviewNextStep>,
}

impl ReviewProgress {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(crate) const fn is_complete(&self) -> bool {
        self.remaining == 0 && self.threads.unresolved == 0
    }

    #[doc = " The next changed file to read, ignoring threads."]
    pub(crate) fn next_file(&self) -> Option<ReviewNextStep> {
        match &self.next {
            Some(step @ ReviewNextStep::File { .. }) => Some(step.clone()),
            _ => None,
        }
    }

    #[doc = " The next unresolved thread, ignoring files."]
    pub(crate) fn next_thread(&self) -> Option<ReviewNextStep> {
        match &self.next {
            Some(step @ ReviewNextStep::Thread { .. }) => Some(step.clone()),
            _ => self.thread_step.clone(),
        }
    }
}
