use std::ffi::OsString;

use anyhow::{Context, Result};
use serde::Serialize;

use super::{
    CacheLife, PullRequest, Repository, bounded_text, cache_read, cache_write, parse_tsv_record,
};

/// The renderer wraps every entry to the pane width on each redraw, so this cap
/// is what keeps that work bounded. It is far above any real thread; the entries
/// dropped are the oldest, and the view says so.
const MAX_CONVERSATION_ENTRIES: usize = 500;
const MAX_CONVERSATION_BODY_BYTES: usize = 64 * 1024;
const MAX_CONVERSATION_CONTEXT_BYTES: usize = 8 * 1024;
const CONVERSATION_FIELDS: usize = 8;
const CONVERSATION_PAGE_SIZE: usize = 100;

/// Events GitHub records but never renders in a pull-request conversation.
/// Dropping them in the query keeps the response small and the thread readable.
const HIDDEN_TIMELINE_EVENTS: &str = r#"["subscribed","unsubscribed","mentioned","referenced","milestoned","demilestoned","user_blocked","connected","disconnected","transferred","pinned","unpinned","locked","unlocked","marked_as_duplicate","unmarked_as_duplicate","comment_deleted","deployed","deployment_environment_changed","automatic_base_change_succeeded","automatic_base_change_failed"]"#;

const REVIEW_COMMENT_TSV_JQ: &str = r#".[] | ["review_comment", (.user.login // ""), (.created_at // ""), ((.path // "") + ":" + (((.line // .original_line) // 0)|tostring)), (.body // ""), (.html_url // ""), ((.pull_request_review_id // 0)|tostring), (.diff_hunk // "")] | @tsv"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ConversationKind {
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
    pub(crate) const fn has_body(self) -> bool {
        matches!(
            self,
            Self::Opened | Self::Comment | Self::Review | Self::ReviewComment | Self::Commit
        )
    }
}

struct ConversationRecords {
    entries: Vec<ConversationEntry>,
    truncated: bool,
    from_cache: bool,
}

/// How a stream reaches its newest entries. Review comments accept a
/// descending sort, so their newest page is page one. The timeline API only
/// serves oldest-first, so its newest page is the one `rel="last"` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConversationPaging {
    NewestFirst,
    LastPageFirst,
}

struct ConversationStream {
    cache_key: String,
    validator_key: String,
    endpoint: String,
    jq: String,
    paging: ConversationPaging,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationEntry {
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestConversation {
    pub entries: Vec<ConversationEntry>,
    pub truncated: bool,
    /// True when nothing had to be transferred: either the thread was already
    /// held for this update stamp, or GitHub confirmed it had not changed.
    pub from_cache: bool,
}

impl PullRequestConversation {
    pub(crate) fn comment_count(&self) -> usize {
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
        let stamp = format!(
            "{}\n{}\n{}",
            pull_request.base_repository.url.trim_end_matches('/'),
            pull_request.number,
            pull_request.updated_at
        );
        let identity = format!(
            "{}\n{}",
            pull_request.base_repository.url.trim_end_matches('/'),
            pull_request.number
        );
        let timeline = self.conversation_records(
            &ConversationStream {
                cache_key: format!("conversation-timeline-v2\n{stamp}"),
                validator_key: format!("conversation-timeline-validator-v2\n{identity}"),
                endpoint: timeline_endpoint(pull_request),
                jq: timeline_tsv_jq(),
                paging: ConversationPaging::LastPageFirst,
            },
            "unable to load the pull-request timeline",
        )?;
        let comments = self.conversation_records(
            &ConversationStream {
                cache_key: format!("conversation-comments-v2\n{stamp}"),
                validator_key: format!("conversation-comments-validator-v2\n{identity}"),
                endpoint: review_comment_endpoint(pull_request),
                jq: REVIEW_COMMENT_TSV_JQ.to_owned(),
                paging: ConversationPaging::NewestFirst,
            },
            "unable to load pull-request review comments",
        )?;

        let from_cache = timeline.from_cache && comments.from_cache;
        let mut entries = vec![opened_entry(pull_request)];
        entries.extend(timeline.entries);
        for comment in comments.entries {
            if entries
                .iter()
                .any(|entry| !entry.url.is_empty() && entry.url == comment.url)
            {
                continue;
            }
            entries.push(comment);
        }
        entries.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));

        let overflowing = entries.len() > MAX_CONVERSATION_ENTRIES;
        if overflowing {
            let opened = entries.remove(0);
            let dropped = entries.len() - (MAX_CONVERSATION_ENTRIES - 1);
            drop(entries.drain(..dropped));
            entries.insert(0, opened);
        }
        let truncated = timeline.truncated || comments.truncated || overflowing;
        Ok(PullRequestConversation {
            entries,
            truncated,
            from_cache,
        })
    }

    /// Read one stream page by page, newest pages first, stopping at the entry
    /// cap. A capped read keeps only the newest pages, so the omitted activity
    /// is genuinely the oldest; an oversized or failed validated first page
    /// degrades to the bounded page loop instead of failing the conversation.
    /// Only a pipe-truncated page prevents caching.
    fn conversation_records(
        &self,
        stream: &ConversationStream,
        error_context: &str,
    ) -> Result<ConversationRecords> {
        if let Some(entry) = cache_read(&stream.cache_key, CacheLife::Immutable) {
            let (complete, body) = split_conversation_cache(&entry);
            return Ok(ConversationRecords {
                entries: parse_conversation(body).context(error_context.to_owned())?,
                truncated: !complete,
                from_cache: true,
            });
        }
        let first = match self.validated_gh(
            &stream.validator_key,
            vec![
                OsString::from(&stream.endpoint),
                OsString::from("--jq"),
                OsString::from(&stream.jq),
            ],
        ) {
            Ok(read) if !read.truncated => Some(read),
            Ok(_) | Err(_) => None,
        };
        if let Some(read) = &first
            && read.complete
        {
            let entries = parse_conversation(&read.data).context(error_context.to_owned())?;
            cache_write(
                &stream.cache_key,
                &conversation_cache_entry(true, &read.data),
            );
            return Ok(ConversationRecords {
                entries,
                truncated: false,
                from_cache: read.unchanged,
            });
        }

        let mut collected: Vec<u8> = Vec::new();
        let mut lines = 0_usize;
        let mut pipe_truncated = false;
        let mut complete = true;
        let (first_data, first_last_page, has_more) = if let Some(read) = first {
            (read.data, read.last_page, true)
        } else {
            let read = self.api_page(&stream.endpoint, &stream.jq, 1, error_context)?;
            pipe_truncated |= read.truncated;
            (read.data, read.last_page, read.has_next)
        };
        match (stream.paging, first_last_page.filter(|last| *last >= 2)) {
            (ConversationPaging::LastPageFirst, Some(last)) => {
                for page in (2..=last).rev() {
                    if lines >= MAX_CONVERSATION_ENTRIES {
                        complete = false;
                        break;
                    }
                    let read = self.api_page(&stream.endpoint, &stream.jq, page, error_context)?;
                    pipe_truncated |= read.truncated;
                    append_records(&mut collected, &mut lines, &read.data);
                }
                if complete {
                    append_records(&mut collected, &mut lines, &first_data);
                }
            }
            (ConversationPaging::NewestFirst, _) | (ConversationPaging::LastPageFirst, None) => {
                append_records(&mut collected, &mut lines, &first_data);
                let mut next = has_more.then_some(2_usize);
                while let Some(page) = next {
                    if lines >= MAX_CONVERSATION_ENTRIES {
                        complete = false;
                        break;
                    }
                    let read = self.api_page(&stream.endpoint, &stream.jq, page, error_context)?;
                    pipe_truncated |= read.truncated;
                    append_records(&mut collected, &mut lines, &read.data);
                    next = read.has_next.then(|| page.saturating_add(1));
                }
            }
        }

        let entries = parse_conversation(&collected).context(error_context.to_owned())?;
        if !pipe_truncated {
            cache_write(
                &stream.cache_key,
                &conversation_cache_entry(complete, &collected),
            );
        }
        Ok(ConversationRecords {
            entries,
            truncated: pipe_truncated || !complete,
            from_cache: false,
        })
    }
}

fn append_records(collected: &mut Vec<u8>, lines: &mut usize, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let mut added = data.split(|byte| *byte == b'\n').count().saturating_sub(1);
    collected.extend_from_slice(data);
    if collected.last() != Some(&b'\n') {
        collected.push(b'\n');
        added = added.saturating_add(1);
    }
    *lines = lines.saturating_add(added);
}

fn conversation_cache_entry(complete: bool, data: &[u8]) -> Vec<u8> {
    let mut entry = Vec::with_capacity(data.len().saturating_add(12));
    entry.extend_from_slice(if complete { b"complete" } else { b"partial" });
    entry.push(b'\n');
    entry.extend_from_slice(data);
    entry
}

fn split_conversation_cache(entry: &[u8]) -> (bool, &[u8]) {
    entry
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or((true, entry), |index| {
            let (marker, rest) = entry.split_at(index);
            (marker == b"complete", rest.get(1..).unwrap_or_default())
        })
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
        let [
            kind,
            actor,
            timestamp,
            detail,
            body,
            url,
            reference,
            context,
        ] = parse_tsv_record::<CONVERSATION_FIELDS>(record)
            .with_context(|| format!("invalid conversation record {}", index + 1))?;
        entries.push(ConversationEntry {
            kind: ConversationKind::parse(&kind),
            actor,
            timestamp,
            detail,
            body: bounded_text(&body, MAX_CONVERSATION_BODY_BYTES),
            url,
            reference,
            context: bounded_text(&context, MAX_CONVERSATION_CONTEXT_BYTES),
        });
    }
    Ok(entries)
}

fn timeline_endpoint(pull_request: &PullRequest) -> String {
    format!(
        "repos/{}/issues/{}/timeline?per_page={CONVERSATION_PAGE_SIZE}",
        pull_request.base_repository.name_with_owner, pull_request.number
    )
}

fn review_comment_endpoint(pull_request: &PullRequest) -> String {
    format!(
        "repos/{}/pulls/{}/comments?per_page={CONVERSATION_PAGE_SIZE}&sort=created&direction=desc",
        pull_request.base_repository.name_with_owner, pull_request.number
    )
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
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
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
        parse_conversation(b"comment\tonly\ttwo\n").unwrap_err();
    }

    #[test]
    fn queries_are_page_bounded_and_scoped_to_the_pull_request() {
        let request = super::super::tests::pull_request(
            super::super::tests::repository(
                "acme/widget",
                "https://github.com/acme/widget",
                &["origin"],
            ),
            42,
        );

        assert_eq!(
            timeline_endpoint(&request),
            "repos/acme/widget/issues/42/timeline?per_page=100"
        );
        assert_eq!(
            review_comment_endpoint(&request),
            "repos/acme/widget/pulls/42/comments?per_page=100&sort=created&direction=desc"
        );
        assert!(timeline_tsv_jq().contains("head_ref_force_pushed"));
        assert!(timeline_tsv_jq().contains("line-commented"));
        assert!(REVIEW_COMMENT_TSV_JQ.contains("diff_hunk"));
    }

    #[test]
    fn cache_entries_remember_whether_the_read_was_complete() {
        let complete = conversation_cache_entry(true, b"comment\trow\n");
        let partial = conversation_cache_entry(false, b"comment\trow\n");

        assert_eq!(
            split_conversation_cache(&complete),
            (true, b"comment\trow\n".as_slice())
        );
        assert_eq!(
            split_conversation_cache(&partial),
            (false, b"comment\trow\n".as_slice())
        );
    }

    #[test]
    fn appended_records_always_end_on_a_record_boundary() {
        let mut collected = Vec::new();
        let mut lines = 0;
        append_records(&mut collected, &mut lines, b"a\tb\nc\td");
        append_records(&mut collected, &mut lines, b"e\tf\n");

        assert_eq!(lines, 3, "an unterminated tail still counts as one record");
        assert_eq!(collected, b"a\tb\nc\td\ne\tf\n");
    }
}
