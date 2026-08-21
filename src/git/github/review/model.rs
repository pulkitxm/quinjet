use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PullRequestReviewSide {
    Left,
    Right,
    #[serde(other)]
    Unknown,
}

impl PullRequestReviewSide {
    pub(super) const fn graphql(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Right | Self::Unknown => "RIGHT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PullRequestReviewThreadSubject {
    File,
    Line,
    #[serde(other)]
    Unknown,
}

impl PullRequestReviewThreadSubject {
    pub(super) const fn graphql(self) -> &'static str {
        match self {
            Self::File => "FILE",
            Self::Line | Self::Unknown => "LINE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PullRequestReviewDecision {
    Comment,
    Approve,
    RequestChanges,
}

impl PullRequestReviewDecision {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Comment => "Comment",
            Self::Approve => "Approve",
            Self::RequestChanges => "Request changes",
        }
    }

    pub(super) const fn graphql(self) -> &'static str {
        match self {
            Self::Comment => "COMMENT",
            Self::Approve => "APPROVE",
            Self::RequestChanges => "REQUEST_CHANGES",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestPendingReview {
    pub id: String,
    pub body: String,
    pub commit_oid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestReviewComment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub state: String,
    pub viewer_did_author: bool,
    pub viewer_can_update: bool,
    pub viewer_can_delete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the booleans are independent GitHub thread state and viewer capabilities"
)]
pub(crate) struct PullRequestReviewThread {
    pub id: String,
    pub path: PathBuf,
    pub side: PullRequestReviewSide,
    pub line: Option<usize>,
    pub original_line: Option<usize>,
    pub start_side: Option<PullRequestReviewSide>,
    pub start_line: Option<usize>,
    pub original_start_line: Option<usize>,
    pub subject: PullRequestReviewThreadSubject,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub resolved_by: Option<String>,
    pub viewer_can_reply: bool,
    pub viewer_can_resolve: bool,
    pub viewer_can_unresolve: bool,
    pub comments: Vec<PullRequestReviewComment>,
    pub comments_truncated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestReviewSnapshot {
    pub pull_request_id: String,
    pub head_oid: String,
    pub review_decision: Option<String>,
    pub pending_review: Option<PullRequestPendingReview>,
    pub threads: Vec<PullRequestReviewThread>,
    pub truncated: bool,
}

impl PullRequestReviewSnapshot {
    pub(crate) fn unresolved_count(&self) -> usize {
        self.threads
            .iter()
            .filter(|thread| !thread.is_resolved)
            .count()
    }

    pub(crate) fn pending_comment_count(&self) -> usize {
        self.threads
            .iter()
            .flat_map(|thread| &thread.comments)
            .filter(|comment| comment.state.eq_ignore_ascii_case("PENDING"))
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullRequestReviewOperation {
    AddThread {
        body: String,
        path: PathBuf,
        line: Option<usize>,
        side: Option<PullRequestReviewSide>,
        start_line: Option<usize>,
        start_side: Option<PullRequestReviewSide>,
        subject: PullRequestReviewThreadSubject,
    },
    Reply {
        thread_id: String,
        body: String,
    },
    UpdateComment {
        comment_id: String,
        body: String,
    },
    DeleteComment {
        comment_id: String,
    },
    Submit {
        body: String,
        decision: PullRequestReviewDecision,
    },
    Discard,
    Resolve {
        thread_id: String,
    },
    Unresolve {
        thread_id: String,
    },
}

impl PullRequestReviewOperation {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::AddThread { .. } => "Adding review comment",
            Self::Reply { .. } => "Replying to review thread",
            Self::UpdateComment { .. } => "Updating review comment",
            Self::DeleteComment { .. } => "Deleting review comment",
            Self::Submit { .. } => "Submitting pull-request review",
            Self::Discard => "Discarding pending review",
            Self::Resolve { .. } => "Resolving review thread",
            Self::Unresolve { .. } => "Reopening review thread",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_enums_have_stable_api_values() {
        assert_eq!(PullRequestReviewSide::Left.graphql(), "LEFT");
        assert_eq!(PullRequestReviewSide::Right.graphql(), "RIGHT");
        assert_eq!(PullRequestReviewSide::Unknown.graphql(), "RIGHT");
        assert_eq!(PullRequestReviewThreadSubject::File.graphql(), "FILE");
        assert_eq!(PullRequestReviewThreadSubject::Line.graphql(), "LINE");
        assert_eq!(PullRequestReviewThreadSubject::Unknown.graphql(), "LINE");
        let decisions = [
            (PullRequestReviewDecision::Comment, "Comment", "COMMENT"),
            (PullRequestReviewDecision::Approve, "Approve", "APPROVE"),
            (
                PullRequestReviewDecision::RequestChanges,
                "Request changes",
                "REQUEST_CHANGES",
            ),
        ];
        for (decision, label, graphql) in decisions {
            assert_eq!(decision.label(), label);
            assert_eq!(decision.graphql(), graphql);
        }
    }

    #[test]
    fn snapshot_counts_open_threads_and_pending_comments() {
        let comment = PullRequestReviewComment {
            id: "COMMENT_1".to_owned(),
            author: "reviewer".to_owned(),
            body: "Fix this".to_owned(),
            created_at: String::new(),
            updated_at: String::new(),
            url: String::new(),
            state: "pending".to_owned(),
            viewer_did_author: true,
            viewer_can_update: true,
            viewer_can_delete: true,
        };
        let thread = |is_resolved, comments| PullRequestReviewThread {
            id: "THREAD_1".to_owned(),
            path: PathBuf::from("src/main.rs"),
            side: PullRequestReviewSide::Right,
            line: Some(1),
            original_line: None,
            start_side: None,
            start_line: None,
            original_start_line: None,
            subject: PullRequestReviewThreadSubject::Line,
            is_resolved,
            is_outdated: false,
            resolved_by: None,
            viewer_can_reply: true,
            viewer_can_resolve: true,
            viewer_can_unresolve: false,
            comments,
            comments_truncated: false,
        };
        let snapshot = PullRequestReviewSnapshot {
            threads: vec![thread(false, vec![comment]), thread(true, Vec::new())],
            ..PullRequestReviewSnapshot::default()
        };
        assert_eq!(snapshot.unresolved_count(), 1);
        assert_eq!(snapshot.pending_comment_count(), 1);
    }
}
