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
