#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

const MAX_REVIEW_THREADS: usize = 500;
const MAX_REVIEW_COMMENTS_PER_THREAD: usize = 100;

const REVIEW_QUERY: &str = "
query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      id
      headRefOid
      reviewDecision
      reviews(last: 20, states: [PENDING]) {
        nodes {
          id
          body
          viewerDidAuthor
          commit { oid }
        }
      }
      reviewThreads(first: 50, after: $after) {
        nodes {
          id
          path
          diffSide
          line
          originalLine
          startDiffSide
          startLine
          originalStartLine
          subjectType
          isResolved
          isOutdated
          resolvedBy { login }
          viewerCanReply
          viewerCanResolve
          viewerCanUnresolve
          comments(first: 100) {
            totalCount
            nodes {
              id
              author { login }
              body
              createdAt
              updatedAt
              url
              state
              viewerDidAuthor
              viewerCanUpdate
              viewerCanDelete
            }
          }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
";

#[derive(Deserialize)]
struct ReviewQueryData {
    repository: Option<ReviewRepositoryNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewRepositoryNode {
    pull_request: Option<ReviewPullRequestNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPullRequestNode {
    id: String,
    head_ref_oid: String,
    review_decision: Option<String>,
    reviews: PendingReviewConnection,
    review_threads: ReviewThreadConnection,
}

#[derive(Deserialize)]
struct PendingReviewConnection {
    nodes: Vec<Option<PendingReviewNode>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingReviewNode {
    id: String,
    body: String,
    viewer_did_author: bool,
    commit: Option<CommitNode>,
}

#[derive(Deserialize)]
struct CommitNode {
    oid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadConnection {
    nodes: Vec<Option<ReviewThreadNode>>,
    page_info: ReviewPageInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the response mirrors independent fields in GitHub's GraphQL schema"
)]
struct ReviewThreadNode {
    id: String,
    path: String,
    diff_side: PullRequestReviewSide,
    line: Option<usize>,
    original_line: Option<usize>,
    start_diff_side: Option<PullRequestReviewSide>,
    start_line: Option<usize>,
    original_start_line: Option<usize>,
    subject_type: PullRequestReviewThreadSubject,
    is_resolved: bool,
    is_outdated: bool,
    resolved_by: Option<ActorNode>,
    viewer_can_reply: bool,
    viewer_can_resolve: bool,
    viewer_can_unresolve: bool,
    comments: ReviewCommentConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCommentConnection {
    total_count: usize,
    nodes: Vec<Option<ReviewCommentNode>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCommentNode {
    id: String,
    author: Option<ActorNode>,
    body: String,
    created_at: String,
    updated_at: String,
    url: String,
    state: String,
    viewer_did_author: bool,
    viewer_can_update: bool,
    viewer_can_delete: bool,
}

#[derive(Deserialize)]
struct ActorNode {
    login: String,
}

impl Repository {
    pub(crate) fn pull_request_review(
        &self,
        pull_request: &PullRequest,
    ) -> Result<PullRequestReviewSnapshot> {
        let (owner, name) = repository_parts(pull_request)?;
        let mut after: Option<String> = None;
        let mut snapshot = PullRequestReviewSnapshot::default();
        loop {
            let variables = json!({
                "owner": owner,
                "name": name,
                "number": pull_request.number,
                "after": after,
            });
            let data: ReviewQueryData = self.graphql(
                pull_request,
                REVIEW_QUERY,
                &variables,
                "unable to load pull-request reviews",
            )?;
            let repository = data
                .repository
                .context("GitHub did not return the pull request's repository")?;
            let review = repository
                .pull_request
                .context("GitHub did not return the pull request")?;
            if snapshot.pull_request_id.is_empty() {
                snapshot.pull_request_id.clone_from(&review.id);
                snapshot.head_oid.clone_from(&review.head_ref_oid);
                snapshot.review_decision.clone_from(&review.review_decision);
                snapshot.pending_review = review
                    .reviews
                    .nodes
                    .into_iter()
                    .flatten()
                    .rfind(|pending| pending.viewer_did_author)
                    .map(|pending| PullRequestPendingReview {
                        id: pending.id,
                        body: pending.body,
                        commit_oid: pending.commit.map_or_else(String::new, |commit| commit.oid),
                    });
            }
            for thread in review.review_threads.nodes.into_iter().flatten() {
                let comments_truncated = thread.comments.total_count > thread.comments.nodes.len();
                let comments = thread
                    .comments
                    .nodes
                    .into_iter()
                    .flatten()
                    .take(MAX_REVIEW_COMMENTS_PER_THREAD)
                    .map(review_comment)
                    .collect();
                snapshot.truncated |= comments_truncated;
                snapshot.threads.push(PullRequestReviewThread {
                    id: thread.id,
                    path: PathBuf::from(thread.path),
                    side: thread.diff_side,
                    line: thread.line,
                    original_line: thread.original_line,
                    start_side: thread.start_diff_side,
                    start_line: thread.start_line,
                    original_start_line: thread.original_start_line,
                    subject: thread.subject_type,
                    is_resolved: thread.is_resolved,
                    is_outdated: thread.is_outdated,
                    resolved_by: thread.resolved_by.map(|actor| actor.login),
                    viewer_can_reply: thread.viewer_can_reply,
                    viewer_can_resolve: thread.viewer_can_resolve,
                    viewer_can_unresolve: thread.viewer_can_unresolve,
                    comments,
                    comments_truncated,
                });
                if snapshot.threads.len() >= MAX_REVIEW_THREADS {
                    snapshot.truncated = true;
                    return Ok(snapshot);
                }
            }
            if !review.review_threads.page_info.has_next_page {
                break;
            }
            let Some(cursor) = review.review_threads.page_info.end_cursor else {
                snapshot.truncated = true;
                break;
            };
            after = Some(cursor);
        }
        Ok(snapshot)
    }
}

fn review_comment(comment: ReviewCommentNode) -> PullRequestReviewComment {
    PullRequestReviewComment {
        id: comment.id,
        author: comment
            .author
            .map_or_else(|| "ghost".to_owned(), |actor| actor.login),
        body: comment.body,
        created_at: comment.created_at,
        updated_at: comment.updated_at,
        url: comment.url,
        state: comment.state,
        viewer_did_author: comment.viewer_did_author,
        viewer_can_update: comment.viewer_can_update,
        viewer_can_delete: comment.viewer_can_delete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_query_decodes_thread_state_permissions_and_comments() {
        let data: ReviewQueryData = serde_json::from_value(json!({
            "repository": {
                "pullRequest": {
                    "id": "PR_1",
                    "headRefOid": "abc",
                    "reviewDecision": "CHANGES_REQUESTED",
                    "reviews": { "nodes": [] },
                    "reviewThreads": {
                        "nodes": [{
                            "id": "THREAD_1",
                            "path": "src/main.rs",
                            "diffSide": "RIGHT",
                            "line": 42,
                            "originalLine": 41,
                            "startDiffSide": null,
                            "startLine": null,
                            "originalStartLine": null,
                            "subjectType": "LINE",
                            "isResolved": false,
                            "isOutdated": false,
                            "resolvedBy": null,
                            "viewerCanReply": true,
                            "viewerCanResolve": true,
                            "viewerCanUnresolve": false,
                            "comments": {
                                "totalCount": 1,
                                "nodes": [{
                                    "id": "COMMENT_1",
                                    "author": { "login": "reviewer" },
                                    "body": "Fix this",
                                    "createdAt": "2026-08-21T00:00:00Z",
                                    "updatedAt": "2026-08-21T00:00:00Z",
                                    "url": "https://example.test/comment",
                                    "state": "SUBMITTED",
                                    "viewerDidAuthor": false,
                                    "viewerCanUpdate": false,
                                    "viewerCanDelete": false
                                }]
                            }
                        }],
                        "pageInfo": { "hasNextPage": false, "endCursor": null }
                    }
                }
            }
        }))
        .unwrap();
        let pull_request = data.repository.unwrap().pull_request.unwrap();
        let thread = pull_request.review_threads.nodes[0].as_ref().unwrap();
        assert_eq!(thread.path, "src/main.rs");
        assert_eq!(thread.line, Some(42));
        assert!(thread.viewer_can_reply);
        assert_eq!(thread.comments.total_count, 1);
    }
}
