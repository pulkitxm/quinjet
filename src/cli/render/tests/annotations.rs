use std::path::PathBuf;

use super::*;
use crate::git::github::{
    AnnotationPlacement, AnnotationSeverity, CheckAnnotation, PullRequestAnnotations,
};

fn annotation(
    check: &str,
    path: &str,
    start: usize,
    end: usize,
    severity: AnnotationSeverity,
    placement: AnnotationPlacement,
) -> CheckAnnotation {
    CheckAnnotation {
        check: check.to_owned(),
        check_run_id: 7,
        check_url: "https://example.test/run".to_owned(),
        path: PathBuf::from(path),
        start_line: start,
        end_line: end,
        start_column: None,
        end_column: None,
        severity,
        title: format!("{check} says something"),
        message: "First line\n\nSecond line".to_owned(),
        raw_details: "extra detail".to_owned(),
        url: String::new(),
        placement,
    }
}

fn sample() -> PullRequestAnnotations {
    let mut listing = PullRequestAnnotations {
        head_oid: "a".repeat(40),
        annotations: vec![
            annotation(
                "Clippy",
                "src/lib.rs",
                12,
                12,
                AnnotationSeverity::Failure,
                AnnotationPlacement::InDiff,
            ),
            annotation(
                "Clippy",
                "src/lib.rs",
                90,
                92,
                AnnotationSeverity::Warning,
                AnnotationPlacement::OutsideHunks,
            ),
            annotation(
                "Spell check",
                "README.md",
                2,
                2,
                AnnotationSeverity::Notice,
                AnnotationPlacement::OutsideDiff,
            ),
        ],
        ..PullRequestAnnotations::default()
    };
    let filter = crate::git::github::AnnotationFilter::default();
    listing = filter.apply(listing);
    listing
}

#[test]
fn grouping_by_file_leads_with_the_path_and_omits_it_from_the_rows() {
    let text = annotations(&sample(), AnnotationGrouping::File, false);
    let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();

    assert_eq!(lines[0], "README.md");
    assert!(lines[1].starts_with("  i  notice   2 "), "{}", lines[1]);
    assert!(lines[1].contains("[outside diff]"), "{}", lines[1]);
    assert!(lines[1].ends_with("(Spell check)"), "{}", lines[1]);
    assert_eq!(lines[2], "src/lib.rs");
    assert!(lines[3].starts_with("  x  failure  12 "), "{}", lines[3]);
    assert!(!lines[3].contains("src/lib.rs"), "{}", lines[3]);
    assert!(lines[4].contains("90-92"), "{}", lines[4]);
}

#[test]
fn grouping_by_check_drops_the_check_suffix_and_shows_the_whole_location() {
    let text = annotations(&sample(), AnnotationGrouping::Check, false);

    assert!(text.contains("\nClippy\n"), "{text}");
    assert!(text.contains("\nSpell check\n"), "{text}");
    assert!(!text.contains("(Clippy)"), "{text}");
    assert!(text.contains("src/lib.rs:12"), "{text}");
    assert!(text.contains("src/lib.rs:90-92"), "{text}");
}

#[test]
fn grouping_by_severity_drops_the_severity_word_from_the_rows() {
    let text = annotations(&sample(), AnnotationGrouping::Severity, false);
    let headings: Vec<&str> = text
        .lines()
        .filter(|line| ["failure", "warning", "notice"].contains(line))
        .collect();

    assert_eq!(headings, vec!["failure", "notice", "warning"]);
    assert!(text.contains("  x  src/lib.rs:12"), "{text}");
}

#[test]
fn the_summary_counts_severity_and_where_the_annotations_landed() {
    assert!(
        annotations(&sample(), AnnotationGrouping::File, false)
            .contains("1 failure, 1 warning, 1 notice · 1 on changed lines, 2 elsewhere"),
        "{}",
        annotations(&sample(), AnnotationGrouping::File, false)
    );
}

#[test]
fn the_full_face_adds_the_message_and_the_raw_details_without_blank_lines() {
    let text = annotations(&sample(), AnnotationGrouping::File, true);

    assert!(text.contains("      First line"), "{text}");
    assert!(text.contains("      Second line"), "{text}");
    assert!(text.contains("      extra detail"), "{text}");
    assert!(!text.contains("\n      \n"), "{text}");
}

#[test]
fn an_empty_listing_says_so_and_still_reports_its_warnings() {
    let mut listing = PullRequestAnnotations::default();
    listing
        .warnings
        .push("one check run could not be read".to_owned());

    let text = annotations(&listing, AnnotationGrouping::File, false);

    assert_eq!(
        text,
        "No annotations reported\nnote  one check run could not be read\n"
    );
}

#[test]
fn a_truncated_listing_says_it_is_incomplete() {
    let mut listing = sample();
    listing.truncated = true;

    assert!(
        annotations(&listing, AnnotationGrouping::File, false)
            .contains("[the annotation list reached Quinjet's size cap]"),
        "{}",
        annotations(&listing, AnnotationGrouping::File, false)
    );
}

#[test]
fn a_file_level_annotation_prints_file_instead_of_a_line_number() {
    let mut listing = PullRequestAnnotations {
        annotations: vec![annotation(
            "Clippy",
            "src/lib.rs",
            0,
            0,
            AnnotationSeverity::Notice,
            AnnotationPlacement::InDiff,
        )],
        ..PullRequestAnnotations::default()
    };
    listing = crate::git::github::AnnotationFilter::default().apply(listing);

    assert!(
        annotations(&listing, AnnotationGrouping::File, false).contains("  i  notice   file "),
        "{}",
        annotations(&listing, AnnotationGrouping::File, false)
    );
}
