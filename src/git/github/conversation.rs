use std::ffi::OsString;

use anyhow::{Context, Result, bail};

use super::{PullRequest, Repository, bounded_command_error, bounded_text, parse_tsv_record};

const MAX_CONVERSATION_ENTRIES: usize = 2_000;
const MAX_CONVERSATION_BODY_BYTES: usize = 64 * 1024;
const MAX_CONVERSATION_CONTEXT_BYTES: usize = 8 * 1024;
const CONVERSATION_FIELDS: usize = 8;
const CONVERSATION_PAGE_SIZE: usize = 100;

/// Events GitHub records but never renders in a pull-request conversation.
/// Dropping them in the query keeps the response small and the thread readable.
const HIDDEN_TIMELINE_EVENTS: &str = r#"["subscribed","unsubscribed","mentioned","referenced","milestoned","demilestoned","user_blocked","connected","disconnected","transferred","pinned","unpinned","locked","unlocked","marked_as_duplicate","unmarked_as_duplicate","comment_deleted","deployed","deployment_environment_changed","automatic_base_change_succeeded","automatic_base_change_failed"]"#;

const REVIEW_COMMENT_TSV_JQ: &str = r#".[] | ["review_comment", (.user.login // ""), (.created_at // ""), ((.path // "") + ":" + (((.line // .original_line) // 0)|tostring)), (.body // ""), (.html_url // ""), ((.pull_request_review_id // 0)|tostring), (.diff_hunk // "")] | @tsv"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationKind {
    Opened,
    Comment,
    Review,
    ReviewComment,
    Commit,
    ForcePush,
    Merged,
    Closed,
    Reopened,
    Labeled,
    Unlabeled,
    Renamed,
    ReadyForReview,
    ConvertedToDraft,
    ReviewRequested,
    ReviewRequestRemoved,
    Assigned,
    Unassigned,
    CrossReferenced,
    HeadRefDeleted,
    HeadRefRestored,
    BaseRefChanged,
    Other,
}

impl ConversationKind {
    fn parse(value: &str) -> Self {
        match value {
            "comment" => Self::Comment,
            "review" => Self::Review,
            "review_comment" => Self::ReviewComment,
            "commit" => Self::Commit,
            "force_push" => Self::ForcePush,
            "merged" => Self::Merged,
            "closed" => Self::Closed,
            "reopened" => Self::Reopened,
            "labeled" => Self::Labeled,
            "unlabeled" => Self::Unlabeled,
            "renamed" => Self::Renamed,
            "ready_for_review" => Self::ReadyForReview,
            "convert_to_draft" | "converted_to_draft" => Self::ConvertedToDraft,
            "review_requested" => Self::ReviewRequested,
            "review_request_removed" => Self::ReviewRequestRemoved,
            "assigned" => Self::Assigned,
            "unassigned" => Self::Unassigned,
            "cross_referenced" => Self::CrossReferenced,
            "head_ref_deleted" => Self::HeadRefDeleted,
            "head_ref_restored" => Self::HeadRefRestored,
            "base_ref_changed" => Self::BaseRefChanged,
            _ => Self::Other,
        }
    }

    /// Whether the entry carries prose worth rendering under its header.
    pub const fn has_body(self) -> bool {
        matches!(
            self,
            Self::Opened | Self::Comment | Self::Review | Self::ReviewComment | Self::Commit
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationEntry {
    pub kind: ConversationKind,
    pub actor: String,
    pub timestamp: String,
    /// Short qualifier for the header: a review verdict, a label name, a
    /// `path:line` anchor, an abbreviated commit, or a renamed title.
    pub detail: String,
    pub body: String,
    pub url: String,
    /// Stable identity for the underlying object: a commit OID, a review id, or
    /// the post-rename title.
    pub reference: String,
    /// Supporting text shown with the entry, currently a review comment's hunk.
    pub context: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestConversation {
    pub entries: Vec<ConversationEntry>,
    pub truncated: bool,
}

impl PullRequestConversation {
    pub fn comment_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.kind,
                    ConversationKind::Comment | ConversationKind::ReviewComment
                )
            })
            .count()
    }
}

impl Repository {
    /// Read the whole pull-request conversation: issue comments, reviews and
    /// their inline comments, pushed commits, force pushes, and the lifecycle
    /// events GitHub shows between them.
    ///
    /// Inline review comments are read from their own endpoint rather than
    /// trusted to the timeline, which only groups them into `line-commented`
    /// entries for some pull requests.
    pub(crate) fn pull_request_conversation(
        &self,
        pull_request: &PullRequest,
    ) -> Result<PullRequestConversation> {
        let (timeline, timeline_truncated) = self.conversation_records(
            timeline_args(pull_request),
            "unable to load the pull-request timeline",
        )?;
        let (comments, comments_truncated) = self.conversation_records(
            review_comment_args(pull_request),
            "unable to load pull-request review comments",
        )?;

        let mut entries = vec![opened_entry(pull_request)];
        entries.extend(timeline);
        for comment in comments {
            if entries
                .iter()
                .any(|entry| !entry.url.is_empty() && entry.url == comment.url)
            {
                continue;
            }
            entries.push(comment);
        }
        entries.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));

        let mut truncated = timeline_truncated || comments_truncated;
        if entries.len() > MAX_CONVERSATION_ENTRIES {
            // Keep the most recent activity; the opening post is restored so the
            // thread never loses the description it started from.
            let opened = entries.remove(0);
            entries.drain(..entries.len() - (MAX_CONVERSATION_ENTRIES - 1));
            entries.insert(0, opened);
            truncated = true;
        }
        Ok(PullRequestConversation { entries, truncated })
    }

    fn conversation_records(
        &self,
        args: Vec<OsString>,
        error_context: &str,
    ) -> Result<(Vec<ConversationEntry>, bool)> {
        let output = self.run_gh(args)?;
        if !output.status.success() && !output.stdout_truncated {
            bail!("{}", bounded_command_error(error_context, &output));
        }
        let mut data = output.stdout;
        if output.stdout_truncated {
            // A capped read can stop mid-record; drop the partial tail rather
            // than reporting a malformed entry.
            while data.last().is_some_and(|byte| *byte != b'\n') {
                data.pop();
            }
        }
        let entries = parse_conversation(&data).context(error_context.to_owned())?;
        Ok((entries, output.stdout_truncated))
    }
}

fn opened_entry(pull_request: &PullRequest) -> ConversationEntry {
    ConversationEntry {
        kind: ConversationKind::Opened,
        actor: pull_request.author.clone(),
        timestamp: pull_request.created_at.clone(),
        detail: format!("{} into {}", pull_request.head_ref, pull_request.base_ref),
        body: pull_request.description.clone(),
        url: pull_request.url.clone(),
        reference: pull_request.head_oid.clone(),
        context: String::new(),
    }
}

fn parse_conversation(output: &[u8]) -> Result<Vec<ConversationEntry>> {
    let mut entries = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let fields = parse_tsv_record(record, CONVERSATION_FIELDS)
            .with_context(|| format!("invalid conversation record {}", index + 1))?;
        entries.push(ConversationEntry {
            kind: ConversationKind::parse(&fields[0]),
            actor: fields[1].clone(),
            timestamp: fields[2].clone(),
            detail: fields[3].clone(),
            body: bounded_text(&fields[4], MAX_CONVERSATION_BODY_BYTES),
            url: fields[5].clone(),
            reference: fields[6].clone(),
            context: bounded_text(&fields[7], MAX_CONVERSATION_CONTEXT_BYTES),
        });
    }
    Ok(entries)
}

fn timeline_args(pull_request: &PullRequest) -> Vec<OsString> {
    api_args(
        format!(
            "repos/{}/issues/{}/timeline?per_page={CONVERSATION_PAGE_SIZE}",
            pull_request.base_repository.name_with_owner, pull_request.number
        ),
        timeline_tsv_jq(),
    )
}

fn review_comment_args(pull_request: &PullRequest) -> Vec<OsString> {
    api_args(
        format!(
            "repos/{}/pulls/{}/comments?per_page={CONVERSATION_PAGE_SIZE}",
            pull_request.base_repository.name_with_owner, pull_request.number
        ),
        REVIEW_COMMENT_TSV_JQ.to_owned(),
    )
}

fn api_args(endpoint: String, jq: String) -> Vec<OsString> {
    vec![
        OsString::from("api"),
        OsString::from("--paginate"),
        OsString::from(endpoint),
        OsString::from("--jq"),
        OsString::from(jq),
    ]
}

/// Flatten every timeline shape into one fixed-width record. GitHub gives each
/// event type its own field names, so the mapping has to be explicit; anything
/// unrecognized still arrives with an actor and a timestamp.
fn timeline_tsv_jq() -> String {
    format!(
        r#".[]
| select(.event as $event | ({HIDDEN_TIMELINE_EVENTS} | index($event)) == null)
| if .event == "commented" then
    ["comment", (.user.login // .actor.login // ""), (.created_at // ""), "", (.body // ""), (.html_url // ""), "", ""]
  elif .event == "reviewed" then
    ["review", (.user.login // ""), (.submitted_at // ""), (.state // ""), (.body // ""), (.html_url // ""), ((.id // 0)|tostring), ""]
  elif .event == "committed" then
    ["commit", (.author.name // .committer.name // ""), (.author.date // .committer.date // ""), ((.sha // "")[0:7]), (.message // ""), (.html_url // ""), (.sha // ""), ""]
  elif .event == "head_ref_force_pushed" then
    ["force_push", (.actor.login // ""), (.created_at // ""), "", "", "", (.commit_id // ""), ""]
  elif .event == "labeled" or .event == "unlabeled" then
    [.event, (.actor.login // ""), (.created_at // ""), (.label.name // ""), "", "", "", ""]
  elif .event == "renamed" then
    ["renamed", (.actor.login // ""), (.created_at // ""), (.rename.from // ""), "", "", (.rename.to // ""), ""]
  elif .event == "cross-referenced" then
    ["cross_referenced", (.actor.login // ""), (.created_at // ""), (((.source.issue.number // 0)|tostring) + " " + (.source.issue.title // "")), "", (.source.issue.html_url // ""), "", ""]
  elif .event == "review_requested" or .event == "review_request_removed" then
    [.event, (.actor.login // ""), (.created_at // ""), (.requested_reviewer.login // .requested_team.name // ""), "", "", "", ""]
  elif .event == "assigned" or .event == "unassigned" then
    [.event, (.actor.login // ""), (.created_at // ""), (.assignee.login // ""), "", "", "", ""]
  elif .event == "line-commented" or .event == "commit-commented" then
    (.comments[]? | ["review_comment", (.user.login // ""), (.created_at // ""), ((.path // "") + ":" + (((.line // .original_line) // 0)|tostring)), (.body // ""), (.html_url // ""), ((.pull_request_review_id // 0)|tostring), (.diff_hunk // "")])
  else
    [(.event // "event"), (.actor.login // .user.login // .author.name // ""), (.created_at // .submitted_at // .author.date // ""), "", "", (.html_url // ""), "", ""]
  end
| @tsv"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_conversation_shape_the_query_can_emit() {
        let output = b"comment\toctocat\t2026-08-01T10:00:00Z\t\tLooks good to me\\nship it\thttps://example.test/c/1\t\t\n\
review\treviewer\t2026-08-01T11:00:00Z\tAPPROVED\t\thttps://example.test/r/1\t99\t\n\
review_comment\treviewer\t2026-08-01T11:00:01Z\tsrc/main.rs:42\tExtract this\thttps://example.test/rc/1\t99\t@@ -1 +1 @@\n\
commit\tAda\t2026-08-01T12:00:00Z\tabc1234\tAdd the thing\thttps://example.test/commit\tabc1234567890\t\n\
force_push\toctocat\t2026-08-01T13:00:00Z\t\t\t\tdeadbeef\t\n\
renamed\toctocat\t2026-08-01T14:00:00Z\tOld title\t\t\tNew title\t\n\
weird_new_event\tsomebody\t2026-08-01T15:00:00Z\t\t\t\t\t\n";

        let entries = parse_conversation(output).unwrap();

        assert_eq!(entries.len(), 7);
        assert_eq!(entries[0].kind, ConversationKind::Comment);
        assert_eq!(entries[0].body, "Looks good to me\nship it");
        assert_eq!(entries[1].kind, ConversationKind::Review);
        assert_eq!(entries[1].detail, "APPROVED");
        assert_eq!(entries[2].kind, ConversationKind::ReviewComment);
        assert_eq!(entries[2].detail, "src/main.rs:42");
        assert_eq!(entries[2].context, "@@ -1 +1 @@");
        assert_eq!(entries[3].kind, ConversationKind::Commit);
        assert_eq!(entries[3].reference, "abc1234567890");
        assert_eq!(entries[4].kind, ConversationKind::ForcePush);
        assert_eq!(entries[4].reference, "deadbeef");
        assert_eq!(entries[5].kind, ConversationKind::Renamed);
        assert_eq!(
            (entries[5].detail.as_str(), entries[5].reference.as_str()),
            ("Old title", "New title")
        );
        assert_eq!(
            entries[6].kind,
            ConversationKind::Other,
            "an event GitHub adds later still renders with its actor and time"
        );
        assert_eq!(entries[6].actor, "somebody");
    }

    #[test]
    fn rejects_records_that_do_not_match_the_query_shape() {
        assert!(parse_conversation(b"comment\tonly\ttwo\n").is_err());
    }

    #[test]
    fn queries_are_paginated_and_scoped_to_the_pull_request() {
        let request = super::super::tests::pull_request(
            super::super::tests::repository(
                "acme/widget",
                "https://github.com/acme/widget",
                &["origin"],
            ),
            42,
        );

        let timeline: Vec<String> = timeline_args(&request)
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let comments: Vec<String> = review_comment_args(&request)
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            &timeline[..3],
            &[
                "api",
                "--paginate",
                "repos/acme/widget/issues/42/timeline?per_page=100"
            ]
        );
        assert_eq!(
            &comments[..3],
            &[
                "api",
                "--paginate",
                "repos/acme/widget/pulls/42/comments?per_page=100"
            ]
        );
        assert!(timeline[4].contains("head_ref_force_pushed"));
        assert!(timeline[4].contains("line-commented"));
        assert!(comments[4].contains("diff_hunk"));
    }
}
