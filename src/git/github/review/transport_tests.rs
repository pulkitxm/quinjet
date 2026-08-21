#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::git::github::GitHubRepository;

fn pull_request() -> PullRequest {
    let mut base_repository = GitHubRepository::default();
    "acme/rocket".clone_into(&mut base_repository.name_with_owner);
    PullRequest {
        number: 7,
        base_repository,
        ..PullRequest::default()
    }
}

fn fake_repository(pending_commit: Option<&str>) -> (tempfile::TempDir, Repository) {
    let directory = tempfile::tempdir().unwrap();
    let pending = pending_commit.map(|commit| {
        json!({
            "id": "PENDING_1",
            "body": "Pending summary",
            "viewerDidAuthor": true,
            "commit": { "oid": commit }
        })
    });
    let comment = json!({
        "id": "COMMENT_1",
        "author": null,
        "body": "Fix this",
        "createdAt": "2026-08-21T00:00:00Z",
        "updatedAt": "2026-08-21T00:00:00Z",
        "url": "https://example.test/comment",
        "state": "PENDING",
        "viewerDidAuthor": true,
        "viewerCanUpdate": true,
        "viewerCanDelete": true
    });
    let thread = json!({
        "id": "THREAD_1",
        "path": "src/main.rs",
        "diffSide": "RIGHT",
        "line": 42,
        "originalLine": 41,
        "startDiffSide": "RIGHT",
        "startLine": 40,
        "originalStartLine": 39,
        "subjectType": "LINE",
        "isResolved": false,
        "isOutdated": false,
        "resolvedBy": null,
        "viewerCanReply": true,
        "viewerCanResolve": true,
        "viewerCanUnresolve": false,
        "comments": { "totalCount": 1, "nodes": [comment] }
    });
    let review = json!({
        "id": "PR_1",
        "headRefOid": "abc",
        "reviewDecision": "REVIEW_REQUIRED",
        "reviews": { "nodes": pending.into_iter().collect::<Vec<_>>() },
        "reviewThreads": {
            "nodes": [thread],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        }
    });
    let snapshot = json!({ "data": { "repository": { "pullRequest": review } } });
    let created = json!({
        "data": {
            "addPullRequestReview": {
                "pullRequestReview": {
                    "id": "PENDING_2",
                    "body": "",
                    "commit": { "oid": "abc" }
                }
            }
        }
    });
    let script = format!(
        r#"#!/bin/sh
payload=$(cat)
case "$payload" in
  *reviewThreads*) printf '%s' '{snapshot}' ;;
  *addPullRequestReview*) printf '%s' '{created}' ;;
  *) printf '%s' '{{"data":{{}}}}' ;;
esac
"#
    );
    let executable = directory.path().join("gh");
    fs::write(&executable, script).unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).unwrap();
    let repository = Repository {
        root: directory.path().to_path_buf(),
        github_cli: Some(executable),
    };
    (directory, repository)
}

#[test]
fn every_review_operation_uses_the_graphql_transport() {
    let (_directory, repository) = fake_repository(Some("abc"));
    let request = pull_request();
    let operations = [
        (
            PullRequestReviewOperation::AddThread {
                body: "Fix this".to_owned(),
                path: PathBuf::from("src/main.rs"),
                line: Some(42),
                side: Some(PullRequestReviewSide::Right),
                start_line: None,
                start_side: None,
                subject: PullRequestReviewThreadSubject::Line,
            },
            "Added pending review comment",
        ),
        (
            PullRequestReviewOperation::Reply {
                thread_id: "THREAD_1".to_owned(),
                body: "Agreed".to_owned(),
            },
            "Added pending review reply",
        ),
        (
            PullRequestReviewOperation::UpdateComment {
                comment_id: "COMMENT_1".to_owned(),
                body: "Updated".to_owned(),
            },
            "Updated review comment",
        ),
        (
            PullRequestReviewOperation::DeleteComment {
                comment_id: "COMMENT_1".to_owned(),
            },
            "Deleted review comment",
        ),
        (
            PullRequestReviewOperation::Submit {
                body: "Ready".to_owned(),
                decision: PullRequestReviewDecision::Approve,
            },
            "Submitted review: Approve",
        ),
        (
            PullRequestReviewOperation::Discard,
            "Discarded pending review",
        ),
        (
            PullRequestReviewOperation::Resolve {
                thread_id: "THREAD_1".to_owned(),
            },
            "Resolved review thread",
        ),
        (
            PullRequestReviewOperation::Unresolve {
                thread_id: "THREAD_1".to_owned(),
            },
            "Reopened review thread",
        ),
    ];
    for (operation, expected) in operations {
        assert_eq!(
            repository
                .perform_pull_request_review_operation(&request, &operation)
                .unwrap(),
            expected
        );
        assert!(!operation.label().is_empty());
    }
    let snapshot = repository.pull_request_review(&request).unwrap();
    assert_eq!(snapshot.unresolved_count(), 1);
    assert_eq!(snapshot.pending_comment_count(), 1);
    assert_eq!(snapshot.threads[0].comments[0].author, "ghost");
}

#[test]
fn review_without_a_pending_draft_can_comment_and_submit() {
    let (_directory, repository) = fake_repository(None);
    let request = pull_request();
    let add = PullRequestReviewOperation::AddThread {
        body: "File note".to_owned(),
        path: PathBuf::from("src/main.rs"),
        line: None,
        side: None,
        start_line: None,
        start_side: None,
        subject: PullRequestReviewThreadSubject::File,
    };
    assert_eq!(
        repository
            .perform_pull_request_review_operation(&request, &add)
            .unwrap(),
        "Added pending review comment"
    );
    let submit = PullRequestReviewOperation::Submit {
        body: "General feedback".to_owned(),
        decision: PullRequestReviewDecision::RequestChanges,
    };
    assert_eq!(
        repository
            .perform_pull_request_review_operation(&request, &submit)
            .unwrap(),
        "Submitted review: Request changes"
    );
    drop(
        repository
            .perform_pull_request_review_operation(&request, &PullRequestReviewOperation::Discard)
            .unwrap_err(),
    );
}

#[test]
fn review_helpers_reject_invalid_input_and_stale_drafts() {
    let mut request = pull_request();
    request.base_repository.name_with_owner = "invalid".to_owned();
    drop(repository_parts(&request).unwrap_err());
    let snapshot = PullRequestReviewSnapshot {
        head_oid: "new".to_owned(),
        pending_review: Some(PullRequestPendingReview {
            id: "PENDING_1".to_owned(),
            body: String::new(),
            commit_oid: "old".to_owned(),
        }),
        ..PullRequestReviewSnapshot::default()
    };
    assert!(ensure_current_pending_review(&snapshot).is_err());
    assert_eq!(
        string_field(&json!({ "id": "NODE_1" }), "id").unwrap(),
        "NODE_1"
    );
    drop(string_field(&json!({}), "id").unwrap_err());
    let invalid = ReviewThreadDraft {
        body: " ",
        path: Path::new("src/main.rs"),
        line: None,
        side: None,
        start_line: None,
        start_side: None,
        subject: PullRequestReviewThreadSubject::Line,
    };
    drop(review_thread_input(&invalid).unwrap_err());
}
