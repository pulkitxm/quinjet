use super::*;
use crate::git::Repository;
use crate::git::github::{
    PullRequestReviewComment, PullRequestReviewSide, PullRequestReviewThreadSubject,
};

fn comment(id: &str, body: &str) -> PullRequestReviewComment {
    PullRequestReviewComment {
        id: id.to_owned(),
        author: "hubot".to_owned(),
        body: body.to_owned(),
        created_at: String::new(),
        updated_at: String::new(),
        url: "https://github.com/acme/project/pull/42#c1".to_owned(),
        state: "SUBMITTED".to_owned(),
        viewer_did_author: false,
        viewer_can_update: false,
        viewer_can_delete: false,
    }
}

fn thread(
    id: &str,
    path: &str,
    line: Option<usize>,
    start_line: Option<usize>,
    comments: Vec<PullRequestReviewComment>,
) -> PullRequestReviewThread {
    PullRequestReviewThread {
        id: id.to_owned(),
        path: PathBuf::from(path),
        side: PullRequestReviewSide::Right,
        line,
        original_line: None,
        start_side: None,
        start_line,
        original_start_line: None,
        subject: PullRequestReviewThreadSubject::Line,
        is_resolved: false,
        is_outdated: false,
        resolved_by: None,
        viewer_can_reply: true,
        viewer_can_resolve: true,
        viewer_can_unresolve: false,
        comments,
        comments_truncated: false,
    }
}

fn review(threads: Vec<PullRequestReviewThread>) -> PullRequestReviewSnapshot {
    PullRequestReviewSnapshot {
        threads,
        ..PullRequestReviewSnapshot::default()
    }
}

#[test]
fn a_suggestion_body_wraps_the_replacement_in_the_fence_github_renders() {
    assert_eq!(
        suggestion_body("let value = 1;", "").expect("a plain replacement"),
        "```suggestion\nlet value = 1;\n```\n"
    );
    assert_eq!(
        suggestion_body("one\ntwo\n", "  Use a slice  ").expect("a note is kept above"),
        "Use a slice\n\n```suggestion\none\ntwo\n```\n"
    );
}

#[test]
fn a_replacement_that_would_break_out_of_the_fence_is_refused() {
    drop(suggestion_body("```\nescape\n```", "").expect_err("a fence is refused"));
    drop(suggestion_body("  ```rust", "").expect_err("an indented fence is refused"));
    drop(suggestion_body("let x = \"``\";", "").expect("a short backtick run is fine"));
}

#[test]
fn a_suggestion_is_read_out_of_a_comment_with_its_line_range() {
    let suggestions = collect_suggestions(&review(vec![thread(
        "T1",
        "src/lib.rs",
        Some(14),
        Some(12),
        vec![comment(
            "C1",
            "Use a slice here\n\n```suggestion\nlet value = &[1];\n```\n",
        )],
    )]));

    assert_eq!(suggestions.len(), 1);
    let suggestion = &suggestions[0];
    assert_eq!(suggestion.id, "C1");
    assert_eq!(suggestion.thread_id, "T1");
    assert_eq!(suggestion.path, PathBuf::from("src/lib.rs"));
    assert_eq!(suggestion.start_line, 12);
    assert_eq!(suggestion.end_line, 14);
    assert_eq!(suggestion.replacement, "let value = &[1];");
    assert_eq!(suggestion.comment, "Use a slice here");
    assert_eq!(suggestion.location(), "src/lib.rs:12-14");
    assert_eq!(suggestion.counts(), (3, 1));
    assert!(suggestion.is_applicable());
}

#[test]
fn a_single_line_thread_replaces_exactly_its_own_line() {
    let suggestions = collect_suggestions(&review(vec![thread(
        "T1",
        "src/lib.rs",
        Some(7),
        None,
        vec![comment("C1", "```suggestion\nfixed\n```")],
    )]));

    assert_eq!(suggestions[0].start_line, 7);
    assert_eq!(suggestions[0].end_line, 7);
    assert_eq!(suggestions[0].location(), "src/lib.rs:7");
    assert_eq!(suggestions[0].counts(), (1, 1));
}

#[test]
fn a_comment_carrying_two_suggestions_becomes_two_rows() {
    let suggestions = collect_suggestions(&review(vec![thread(
        "T1",
        "src/lib.rs",
        Some(7),
        None,
        vec![comment(
            "C1",
            "First\n```suggestion\none\n```\nSecond\n```suggestion\ntwo\n```",
        )],
    )]));

    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0].id, "C1");
    assert_eq!(suggestions[0].replacement, "one");
    assert_eq!(suggestions[0].comment, "First");
    assert_eq!(suggestions[1].id, "C1#1");
    assert_eq!(suggestions[1].replacement, "two");
    assert_eq!(suggestions[1].comment, "Second");
}

#[test]
fn a_comment_without_a_suggestion_fence_produces_nothing() {
    let suggestions = collect_suggestions(&review(vec![thread(
        "T1",
        "src/lib.rs",
        Some(7),
        None,
        vec![comment(
            "C1",
            "Please rename this\n```rust\nlet x = 1;\n```",
        )],
    )]));

    assert_eq!(suggestions, Vec::new());
}

#[test]
fn an_empty_suggestion_deletes_its_lines() {
    let suggestions = collect_suggestions(&review(vec![thread(
        "T1",
        "src/lib.rs",
        Some(9),
        Some(8),
        vec![comment("C1", "Drop this\n```suggestion\n```")],
    )]));

    assert_eq!(suggestions[0].replacement, "");
    assert_eq!(suggestions[0].counts(), (2, 0));
}

#[test]
fn a_resolved_or_outdated_thread_blocks_its_suggestions() {
    let mut resolved = thread(
        "T1",
        "src/lib.rs",
        Some(7),
        None,
        vec![comment("C1", "```suggestion\nfixed\n```")],
    );
    resolved.is_resolved = true;
    let mut outdated = thread(
        "T2",
        "src/main.rs",
        Some(7),
        None,
        vec![comment("C2", "```suggestion\nfixed\n```")],
    );
    outdated.is_outdated = true;

    let suggestions = collect_suggestions(&review(vec![resolved, outdated]));

    assert_eq!(suggestions[0].blocker, Some(SuggestionBlocker::Resolved));
    assert_eq!(suggestions[1].blocker, Some(SuggestionBlocker::Outdated));
    assert!(!suggestions[0].is_applicable());
    assert_eq!(
        suggestions[0]
            .blocker
            .as_ref()
            .map(SuggestionBlocker::message),
        Some("its thread is resolved".to_owned())
    );
}

#[test]
fn a_thread_without_a_line_range_blocks_its_suggestions() {
    let suggestions = collect_suggestions(&review(vec![thread(
        "T1",
        "src/lib.rs",
        None,
        None,
        vec![comment("C1", "```suggestion\nfixed\n```")],
    )]));

    assert_eq!(suggestions[0].blocker, Some(SuggestionBlocker::NoLineRange));
}

#[test]
fn a_listing_counts_what_can_be_applied_and_selects_by_id_or_prefix() {
    let mut listing = PullRequestSuggestions {
        number: 42,
        head_oid: "a".repeat(40),
        suggestions: collect_suggestions(&review(vec![
            thread(
                "T1",
                "src/lib.rs",
                Some(7),
                None,
                vec![comment("C1", "```suggestion\none\n```")],
            ),
            thread(
                "T2",
                "src/main.rs",
                None,
                None,
                vec![comment("C2", "```suggestion\ntwo\n```")],
            ),
        ])),
        ..PullRequestSuggestions::default()
    };
    listing.finish();

    assert_eq!(listing.applicable, 1);
    assert_eq!(listing.blocked, 1);
    assert_eq!(listing.applicable_suggestions().len(), 1);
    assert_eq!(listing.select("C1").expect("an exact id").id, "C1");
    drop(listing.select("C").expect_err("an ambiguous prefix"));
    drop(listing.select("nothing").expect_err("an unknown id"));
    assert_eq!(
        listing.schema_version,
        PullRequestSuggestions::SCHEMA_VERSION
    );
}

struct Worktree(tempfile::TempDir);

impl Worktree {
    fn new(files: &[(&str, &str)]) -> Self {
        let directory = tempfile::tempdir().expect("a scratch worktree");
        for (name, contents) in files {
            std::fs::write(directory.path().join(name), contents).expect("a file");
        }
        Self(directory)
    }

    fn repository(&self) -> Repository {
        Repository {
            root: self.0.path().to_path_buf(),
            github_cli: None,
        }
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.path().join(name)).expect("the file")
    }
}

fn suggestion(path: &str, start: usize, end: usize, replacement: &str) -> Suggestion {
    Suggestion {
        id: format!("C{start}"),
        thread_id: "T1".to_owned(),
        author: "hubot".to_owned(),
        path: PathBuf::from(path),
        start_line: start,
        end_line: end,
        replacement: replacement.to_owned(),
        comment: String::new(),
        url: String::new(),
        outdated: false,
        resolved: false,
        blocker: None,
    }
}

#[test]
fn a_plan_replaces_exactly_the_lines_a_suggestion_names() {
    let worktree = Worktree::new(&[("a.rs", "one\ntwo\nthree\nfour\n")]);
    let replacement = suggestion("a.rs", 2, 3, "TWO\nTHREE\nEXTRA");

    let plan = worktree.repository().plan_suggestions(&[&replacement]);

    assert_eq!(plan.applied, vec!["C2".to_owned()]);
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].contents, "one\nTWO\nTHREE\nEXTRA\nfour\n");
    assert_eq!(plan.files[0].removed, 2);
    assert_eq!(plan.files[0].added, 3);
    assert_eq!(plan.summary(), "1 suggestion(s) across 1 file(s), +3 -2");
}

#[test]
fn two_suggestions_in_one_file_are_applied_together_in_line_order() {
    let worktree = Worktree::new(&[("a.rs", "one\ntwo\nthree\nfour\n")]);
    let later = suggestion("a.rs", 4, 4, "FOUR");
    let earlier = suggestion("a.rs", 1, 1, "ONE");

    let plan = worktree.repository().plan_suggestions(&[&later, &earlier]);

    assert_eq!(plan.files[0].contents, "ONE\ntwo\nthree\nFOUR\n");
    assert_eq!(plan.applied, vec!["C1".to_owned(), "C4".to_owned()]);
}

#[test]
fn overlapping_suggestions_are_refused_rather_than_applied_in_some_order() {
    let worktree = Worktree::new(&[("a.rs", "one\ntwo\nthree\n")]);
    let first = suggestion("a.rs", 1, 2, "X");
    let second = suggestion("a.rs", 2, 3, "Y");

    let plan = worktree.repository().plan_suggestions(&[&first, &second]);

    assert!(plan.is_empty());
    assert_eq!(plan.skipped.len(), 2);
    assert!(
        plan.skipped[0].reason.contains("overlap"),
        "{}",
        plan.skipped[0].reason
    );
    assert_eq!(worktree.read("a.rs"), "one\ntwo\nthree\n");
}

#[test]
fn a_suggestion_past_the_end_of_the_file_is_skipped_with_its_reason() {
    let worktree = Worktree::new(&[("a.rs", "one\n")]);
    let past = suggestion("a.rs", 4, 4, "X");

    let plan = worktree.repository().plan_suggestions(&[&past]);

    assert!(plan.is_empty());
    assert!(
        plan.skipped[0].reason.contains("has only 1 lines"),
        "{}",
        plan.skipped[0].reason
    );
    assert_eq!(plan.skipped[0].location, "a.rs:4");
}

#[test]
fn a_missing_file_is_skipped_with_its_reason() {
    let worktree = Worktree::new(&[]);
    let missing = suggestion("gone.rs", 1, 1, "X");

    let plan = worktree.repository().plan_suggestions(&[&missing]);

    assert!(plan.is_empty());
    assert!(
        plan.skipped[0].reason.contains("not readable"),
        "{}",
        plan.skipped[0].reason
    );
}

#[test]
fn a_blocked_suggestion_never_reaches_the_file_plan() {
    let worktree = Worktree::new(&[("a.rs", "one\n")]);
    let mut blocked = suggestion("a.rs", 1, 1, "X");
    blocked.blocker = Some(SuggestionBlocker::Outdated);

    let plan = worktree.repository().plan_suggestions(&[&blocked]);

    assert!(plan.is_empty());
    assert_eq!(
        plan.skipped[0].reason,
        "a later commit moved the code it was written against"
    );
    assert_eq!(plan.summary(), "Nothing to apply");
}

#[test]
fn a_file_without_a_trailing_newline_keeps_not_having_one() {
    let worktree = Worktree::new(&[("a.rs", "one\ntwo")]);
    let replacement = suggestion("a.rs", 2, 2, "TWO");

    let plan = worktree.repository().plan_suggestions(&[&replacement]);

    assert_eq!(plan.files[0].contents, "one\nTWO");
}

#[test]
fn writing_a_plan_changes_only_the_files_it_names() {
    let worktree = Worktree::new(&[("a.rs", "one\ntwo\n"), ("b.rs", "kept\n")]);
    let replacement = suggestion("a.rs", 1, 1, "ONE");
    let repository = worktree.repository();
    let plan = repository.plan_suggestions(&[&replacement]);

    repository
        .write_suggestion_plan(&plan)
        .expect("the plan writes");

    assert_eq!(worktree.read("a.rs"), "ONE\ntwo\n");
    assert_eq!(worktree.read("b.rs"), "kept\n");
}
