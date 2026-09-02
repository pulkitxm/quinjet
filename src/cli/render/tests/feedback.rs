use std::path::PathBuf;

use super::*;
use crate::git::github::{
    FeedbackCounts, FeedbackKind, FeedbackOwner, Suggestion, SuggestionBlocker,
};

fn item(
    kind: FeedbackKind,
    owner: FeedbackOwner,
    path: Option<&str>,
    line: Option<usize>,
    summary: &str,
) -> FeedbackItem {
    FeedbackItem {
        kind,
        id: "T1".to_owned(),
        path: path.map(PathBuf::from),
        line,
        author: "hubot".to_owned(),
        summary: summary.to_owned(),
        body: "First line\nsecond line".to_owned(),
        url: String::new(),
        owner,
        mine: false,
        action: "reply to it".to_owned(),
    }
}

fn queue(items: Vec<FeedbackItem>) -> PullRequestFeedback {
    let mut queue = PullRequestFeedback {
        number: 42,
        head_oid: "a".repeat(40),
        viewer: "octocat".to_owned(),
        items,
        counts: FeedbackCounts::default(),
        truncated: false,
        warnings: Vec::new(),
        schema_version: PullRequestFeedback::SCHEMA_VERSION,
    };
    queue.finish();
    queue
}

#[test]
fn a_queue_row_leads_with_its_kind_and_who_it_waits_on() {
    let text = feedback(
        &queue(vec![
            item(
                FeedbackKind::Thread,
                FeedbackOwner::You,
                Some("src/lib.rs"),
                Some(12),
                "Rename this",
            ),
            item(
                FeedbackKind::Advisory,
                FeedbackOwner::Nobody,
                Some("README.md"),
                Some(2),
                "Spelling",
            ),
        ]),
        false,
    );
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(
        lines[0],
        "thread    you     src/lib.rs:12                    Rename this"
    );
    assert!(
        lines[1].starts_with("advisory  -       README.md:2"),
        "{}",
        lines[1]
    );
    assert!(
        text.contains("1 blocking, 1 advisory · 1 on you, 0 on others"),
        "{text}"
    );
    assert!(text.contains("next  thread src/lib.rs:12"), "{text}");
}

#[test]
fn a_row_without_a_path_falls_back_to_its_author() {
    let text = feedback(
        &queue(vec![item(
            FeedbackKind::ChangesRequested,
            FeedbackOwner::You,
            None,
            None,
            "1 reviewer requested changes",
        )]),
        false,
    );

    assert!(text.contains("changes   you     @hubot"), "{text}");
    assert!(text.contains("next  changes @hubot"), "{text}");
}

#[test]
fn an_empty_queue_says_so() {
    assert_eq!(feedback(&queue(Vec::new()), false), "Nothing outstanding\n");
}

#[test]
fn the_full_face_prints_the_body_and_the_action_under_each_row() {
    let text = feedback(
        &queue(vec![item(
            FeedbackKind::Thread,
            FeedbackOwner::You,
            Some("src/lib.rs"),
            Some(12),
            "Rename this",
        )]),
        true,
    );

    assert!(text.contains("      First line"), "{text}");
    assert!(text.contains("      second line"), "{text}");
    assert!(text.contains("      -> reply to it"), "{text}");
}

#[test]
fn a_truncated_queue_and_its_warnings_are_reported() {
    let mut listing = queue(vec![item(
        FeedbackKind::Thread,
        FeedbackOwner::You,
        Some("src/lib.rs"),
        Some(12),
        "Rename this",
    )]);
    listing.truncated = true;
    listing.warnings = vec!["one check run could not be read".to_owned()];

    let text = feedback(&listing, false);

    assert!(text.contains("reached Quinjet's size cap"), "{text}");
    assert!(
        text.contains("note  one check run could not be read"),
        "{text}"
    );
}

fn suggestion(id: &str, blocker: Option<SuggestionBlocker>) -> Suggestion {
    Suggestion {
        id: id.to_owned(),
        thread_id: "T1".to_owned(),
        author: "hubot".to_owned(),
        path: PathBuf::from("src/lib.rs"),
        start_line: 12,
        end_line: 14,
        replacement: "one\ntwo".to_owned(),
        comment: String::new(),
        url: String::new(),
        outdated: false,
        resolved: false,
        blocker,
    }
}

#[test]
fn a_suggestion_listing_shows_its_range_its_counts_and_whether_it_is_ready() {
    let mut listing = PullRequestSuggestions {
        number: 42,
        head_oid: "a".repeat(40),
        suggestions: vec![
            suggestion("C1", None),
            suggestion("C2", Some(SuggestionBlocker::Outdated)),
        ],
        ..PullRequestSuggestions::default()
    };
    listing.finish();

    let text = suggestions(&listing);

    assert!(text.contains("C1"), "{text}");
    assert!(text.contains("src/lib.rs:12-14"), "{text}");
    assert!(text.contains("+2 -3"), "{text}");
    assert!(text.contains("@hubot"), "{text}");
    assert!(text.contains("ready"), "{text}");
    assert!(
        text.contains("a later commit moved the code it was written against"),
        "{text}"
    );
    assert!(text.contains("1 ready to apply, 1 blocked"), "{text}");
}

#[test]
fn an_empty_suggestion_listing_says_so() {
    assert_eq!(
        suggestions(&PullRequestSuggestions::default()),
        "No suggested changes reported\n"
    );
}
