use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{MAX_GH_METADATA_BYTES, PullRequest, Repository, bounded_command_error};

mod model;
mod query;

pub(crate) use model::*;

#[derive(Deserialize)]
struct GraphqlEnvelope {
    data: Option<Value>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

impl Repository {
    pub(crate) fn perform_pull_request_review_operation(
        &self,
        pull_request: &PullRequest,
        operation: &PullRequestReviewOperation,
    ) -> Result<String> {
        match operation {
            PullRequestReviewOperation::AddThread {
                body,
                path,
                line,
                side,
                start_line,
                start_side,
                subject,
            } => self.add_review_thread(
                pull_request,
                &ReviewThreadDraft {
                    body,
                    path,
                    line: *line,
                    side: *side,
                    start_line: *start_line,
                    start_side: *start_side,
                    subject: *subject,
                },
            ),
            PullRequestReviewOperation::Reply { thread_id, body } => {
                self.reply_review_thread(pull_request, thread_id, body)
            }
            PullRequestReviewOperation::UpdateComment { comment_id, body } => self
                .review_mutation(
                    pull_request,
                    "mutation($input: UpdatePullRequestReviewCommentInput!) { updatePullRequestReviewComment(input: $input) { pullRequestReviewComment { id } } }",
                    &json!({ "input": { "pullRequestReviewCommentId": comment_id, "body": body } }),
                    "unable to update the review comment",
                    "Updated review comment",
                ),
            PullRequestReviewOperation::DeleteComment { comment_id } => self.review_mutation(
                pull_request,
                "mutation($input: DeletePullRequestReviewCommentInput!) { deletePullRequestReviewComment(input: $input) { pullRequestReview { id } } }",
                &json!({ "input": { "id": comment_id } }),
                "unable to delete the review comment",
                "Deleted review comment",
            ),
            PullRequestReviewOperation::Submit { body, decision } => {
                self.submit_review(pull_request, body, *decision)
            }
            PullRequestReviewOperation::Discard => self.discard_review(pull_request),
            PullRequestReviewOperation::Resolve { thread_id } => self.review_mutation(
                pull_request,
                "mutation($input: ResolveReviewThreadInput!) { resolveReviewThread(input: $input) { thread { id } } }",
                &json!({ "input": { "threadId": thread_id } }),
                "unable to resolve the review thread",
                "Resolved review thread",
            ),
            PullRequestReviewOperation::Unresolve { thread_id } => self.review_mutation(
                pull_request,
                "mutation($input: UnresolveReviewThreadInput!) { unresolveReviewThread(input: $input) { thread { id } } }",
                &json!({ "input": { "threadId": thread_id } }),
                "unable to reopen the review thread",
                "Reopened review thread",
            ),
        }
    }

    fn add_review_thread(
        &self,
        pull_request: &PullRequest,
        draft: &ReviewThreadDraft<'_>,
    ) -> Result<String> {
        let snapshot = self.pull_request_review(pull_request)?;
        ensure_current_pending_review(&snapshot)?;
        let thread = review_thread_input(draft)?;
        if let Some(pending) = snapshot.pending_review {
            let mut input = thread;
            drop(input.insert("pullRequestReviewId".to_owned(), Value::String(pending.id)));
            self.review_mutation(
                pull_request,
                "mutation($input: AddPullRequestReviewThreadInput!) { addPullRequestReviewThread(input: $input) { thread { id } } }",
                &json!({ "input": input }),
                "unable to add the review comment",
                "Added pending review comment",
            )
        } else {
            self.review_mutation(
                pull_request,
                "mutation($input: AddPullRequestReviewInput!) { addPullRequestReview(input: $input) { pullRequestReview { id } } }",
                &json!({
                    "input": {
                        "pullRequestId": snapshot.pull_request_id,
                        "commitOID": snapshot.head_oid,
                        "threads": [thread],
                    }
                }),
                "unable to start the pull-request review",
                "Added pending review comment",
            )
        }
    }

    fn reply_review_thread(
        &self,
        pull_request: &PullRequest,
        thread_id: &str,
        body: &str,
    ) -> Result<String> {
        let snapshot = self.pull_request_review(pull_request)?;
        ensure_current_pending_review(&snapshot)?;
        let pending = match snapshot.pending_review {
            Some(pending) => pending,
            None => self.create_pending_review(pull_request, &snapshot)?,
        };
        self.review_mutation(
            pull_request,
            "mutation($input: AddPullRequestReviewThreadReplyInput!) { addPullRequestReviewThreadReply(input: $input) { comment { id } } }",
            &json!({
                "input": {
                    "pullRequestReviewThreadId": thread_id,
                    "pullRequestReviewId": pending.id,
                    "body": body,
                }
            }),
            "unable to reply to the review thread",
            "Added pending review reply",
        )
    }

    fn create_pending_review(
        &self,
        pull_request: &PullRequest,
        snapshot: &PullRequestReviewSnapshot,
    ) -> Result<PullRequestPendingReview> {
        let data = self.graphql_value(
            pull_request,
            "mutation($input: AddPullRequestReviewInput!) { addPullRequestReview(input: $input) { pullRequestReview { id body commit { oid } } } }",
            &json!({
                "input": {
                    "pullRequestId": snapshot.pull_request_id,
                    "commitOID": snapshot.head_oid,
                }
            }),
            "unable to start the pull-request review",
        )?;
        let review = data
            .pointer("/addPullRequestReview/pullRequestReview")
            .context("GitHub did not return the pending review")?;
        Ok(PullRequestPendingReview {
            id: string_field(review, "id")?,
            body: review
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            commit_oid: review
                .pointer("/commit/oid")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    fn submit_review(
        &self,
        pull_request: &PullRequest,
        body: &str,
        decision: PullRequestReviewDecision,
    ) -> Result<String> {
        let snapshot = self.pull_request_review(pull_request)?;
        ensure_current_pending_review(&snapshot)?;
        let input = snapshot.pending_review.map_or_else(
            || {
                json!({
                    "pullRequestId": snapshot.pull_request_id,
                    "commitOID": snapshot.head_oid,
                    "body": body,
                    "event": decision.graphql(),
                })
            },
            |pending| {
                json!({
                    "pullRequestReviewId": pending.id,
                    "body": body,
                    "event": decision.graphql(),
                })
            },
        );
        let (query, context) = if input.get("pullRequestReviewId").is_some() {
            (
                "mutation($input: SubmitPullRequestReviewInput!) { submitPullRequestReview(input: $input) { pullRequestReview { id } } }",
                "unable to submit the pending review",
            )
        } else {
            (
                "mutation($input: AddPullRequestReviewInput!) { addPullRequestReview(input: $input) { pullRequestReview { id } } }",
                "unable to submit the pull-request review",
            )
        };
        self.review_mutation(
            pull_request,
            query,
            &json!({ "input": input }),
            context,
            &format!("Submitted review: {}", decision.label()),
        )
    }

    fn discard_review(&self, pull_request: &PullRequest) -> Result<String> {
        let snapshot = self.pull_request_review(pull_request)?;
        let pending = snapshot
            .pending_review
            .context("there is no pending review to discard")?;
        self.review_mutation(
            pull_request,
            "mutation($input: DeletePullRequestReviewInput!) { deletePullRequestReview(input: $input) { pullRequest { id } } }",
            &json!({ "input": { "pullRequestReviewId": pending.id } }),
            "unable to discard the pending review",
            "Discarded pending review",
        )
    }

    fn review_mutation(
        &self,
        pull_request: &PullRequest,
        query: &str,
        variables: &Value,
        context: &str,
        message: &str,
    ) -> Result<String> {
        let _data = self.graphql_value(pull_request, query, variables, context)?;
        Ok(message.to_owned())
    }

    fn graphql<T: for<'de> Deserialize<'de>>(
        &self,
        pull_request: &PullRequest,
        query: &str,
        variables: &Value,
        context: &str,
    ) -> Result<T> {
        serde_json::from_value(self.graphql_value(pull_request, query, variables, context)?)
            .with_context(|| format!("{context}: GitHub returned an unexpected response"))
    }

    fn graphql_value(
        &self,
        pull_request: &PullRequest,
        query: &str,
        variables: &Value,
        context: &str,
    ) -> Result<Value> {
        let payload = serde_json::to_vec(&json!({ "query": query, "variables": variables }))?;
        let mut args = vec![OsString::from("api"), OsString::from("graphql")];
        let host = pull_request.base_repository.host();
        if !host.is_empty() {
            args.push(OsString::from("--hostname"));
            args.push(OsString::from(host));
        }
        args.push(OsString::from("--input"));
        args.push(OsString::from("-"));
        let output = self.run_gh_with_input(args, &payload, MAX_GH_METADATA_BYTES)?;
        if !output.status.success() {
            bail!("{}", bounded_command_error(context, &output));
        }
        if output.stdout_truncated {
            bail!("{context}: GitHub response exceeded the metadata limit");
        }
        let envelope: GraphqlEnvelope = serde_json::from_slice(&output.stdout)
            .with_context(|| format!("{context}: GitHub returned invalid JSON"))?;
        if !envelope.errors.is_empty() {
            bail!(
                "{context}: {}",
                envelope
                    .errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        envelope
            .data
            .with_context(|| format!("{context}: GitHub returned no data"))
    }
}

fn repository_parts(pull_request: &PullRequest) -> Result<(&str, &str)> {
    pull_request
        .base_repository
        .name_with_owner
        .split_once('/')
        .context("the pull request repository is not in owner/name form")
}

fn ensure_current_pending_review(snapshot: &PullRequestReviewSnapshot) -> Result<()> {
    if let Some(pending) = &snapshot.pending_review
        && !pending.commit_oid.is_empty()
        && pending.commit_oid != snapshot.head_oid
    {
        bail!(
            "the pending review targets an older head commit; submit or discard it before adding more comments"
        );
    }
    Ok(())
}

struct ReviewThreadDraft<'a> {
    body: &'a str,
    path: &'a Path,
    line: Option<usize>,
    side: Option<PullRequestReviewSide>,
    start_line: Option<usize>,
    start_side: Option<PullRequestReviewSide>,
    subject: PullRequestReviewThreadSubject,
}

fn review_thread_input(draft: &ReviewThreadDraft<'_>) -> Result<Map<String, Value>> {
    if draft.body.trim().is_empty() {
        bail!("a review comment cannot be empty");
    }
    let mut input = Map::new();
    drop(input.insert("body".to_owned(), Value::String(draft.body.to_owned())));
    drop(input.insert(
        "path".to_owned(),
        Value::String(draft.path.to_string_lossy().into_owned()),
    ));
    drop(input.insert(
        "subjectType".to_owned(),
        Value::String(draft.subject.graphql().to_owned()),
    ));
    if draft.subject == PullRequestReviewThreadSubject::Line {
        let line = draft
            .line
            .context("a line-level review comment needs a line")?;
        let side = draft
            .side
            .context("a line-level review comment needs a diff side")?;
        drop(input.insert("line".to_owned(), json!(line)));
        drop(input.insert("side".to_owned(), Value::String(side.graphql().to_owned())));
        if let Some(start_line) = draft.start_line {
            drop(input.insert("startLine".to_owned(), json!(start_line)));
            drop(input.insert(
                "startSide".to_owned(),
                Value::String(draft.start_side.unwrap_or(side).graphql().to_owned()),
            ));
        }
    }
    Ok(input)
}

fn string_field(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("GitHub did not return `{field}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_thread_input_uses_blob_coordinates() {
        let input = review_thread_input(&ReviewThreadDraft {
            body: "Fix this",
            path: Path::new("src/main.rs"),
            line: Some(42),
            side: Some(PullRequestReviewSide::Right),
            start_line: Some(40),
            start_side: Some(PullRequestReviewSide::Right),
            subject: PullRequestReviewThreadSubject::Line,
        })
        .unwrap();
        assert_eq!(input.get("line"), Some(&json!(42)));
        assert_eq!(input.get("side"), Some(&json!("RIGHT")));
        assert_eq!(input.get("startLine"), Some(&json!(40)));
    }

    #[test]
    fn file_thread_input_omits_line_coordinates() {
        let input = review_thread_input(&ReviewThreadDraft {
            body: "Consider splitting this file",
            path: Path::new("src/main.rs"),
            line: None,
            side: None,
            start_line: None,
            start_side: None,
            subject: PullRequestReviewThreadSubject::File,
        })
        .unwrap();
        assert!(!input.contains_key("line"));
        assert_eq!(input.get("subjectType"), Some(&json!("FILE")));
    }
}
