use super::coverage::visible_lines;
use super::*;
use crate::git::diff::{DiffDocument, DiffLine, DiffLineKind};
use crate::git::github::{PullRequestFile, PullRequestFileStatus};

fn annotation(
    check: &str,
    path: &str,
    start: usize,
    end: usize,
    severity: AnnotationSeverity,
) -> CheckAnnotation {
    CheckAnnotation {
        check: check.to_owned(),
        check_run_id: 1,
        check_url: "https://example.test/run".to_owned(),
        path: PathBuf::from(path),
        start_line: start,
        end_line: end,
        start_column: None,
        end_column: None,
        severity,
        title: format!("{check} finding"),
        message: "Something to fix".to_owned(),
        raw_details: String::new(),
        url: String::new(),
        placement: AnnotationPlacement::Unknown,
    }
}

fn listing(annotations: Vec<CheckAnnotation>) -> PullRequestAnnotations {
    let mut listing = PullRequestAnnotations {
        head_oid: "a".repeat(40),
        annotations,
        ..PullRequestAnnotations::default()
    };
    listing.finish();
    listing
}

fn index(paths: &[&str]) -> PullRequestDiffIndex {
    PullRequestDiffIndex {
        total_files: paths.len(),
        files: paths
            .iter()
            .map(|path| PullRequestFile {
                path: PathBuf::from(path),
                old_path: None,
                status: PullRequestFileStatus::Modified,
                counts: None,
            })
            .collect(),
        truncated: false,
    }
}

fn document(new_lines: &[Option<usize>]) -> DiffDocument {
    DiffDocument {
        title: "patch".to_owned(),
        lines: new_lines
            .iter()
            .map(|new_line| DiffLine {
                kind: DiffLineKind::Added,
                old_line: None,
                new_line: *new_line,
                spans: Vec::new(),
            })
            .collect(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
    }
}

#[test]
fn a_severity_is_read_from_githubs_word_and_anything_else_is_a_notice() {
    assert_eq!(
        AnnotationSeverity::parse("failure"),
        AnnotationSeverity::Failure
    );
    assert_eq!(
        AnnotationSeverity::parse("WARNING"),
        AnnotationSeverity::Warning
    );
    assert_eq!(
        AnnotationSeverity::parse("notice"),
        AnnotationSeverity::Notice
    );
    assert_eq!(AnnotationSeverity::parse(""), AnnotationSeverity::Notice);
    assert_eq!(
        AnnotationSeverity::parse("something-new"),
        AnnotationSeverity::Notice
    );
}

#[test]
fn annotations_sort_by_severity_then_path_then_line() {
    let listing = listing(vec![
        annotation("lint", "src/z.rs", 10, 10, AnnotationSeverity::Notice),
        annotation("lint", "src/a.rs", 40, 40, AnnotationSeverity::Failure),
        annotation("lint", "src/a.rs", 5, 5, AnnotationSeverity::Failure),
        annotation("build", "src/a.rs", 5, 5, AnnotationSeverity::Warning),
    ]);

    let order: Vec<String> = listing
        .annotations
        .iter()
        .map(|annotation| format!("{} {}", annotation.severity.word(), annotation.location()))
        .collect();

    assert_eq!(
        order,
        vec![
            "failure src/a.rs:5".to_owned(),
            "failure src/a.rs:40".to_owned(),
            "warning src/a.rs:5".to_owned(),
            "notice src/z.rs:10".to_owned(),
        ]
    );
}

#[test]
fn counts_follow_the_rows_that_survived_filtering() {
    let listing = listing(vec![
        annotation("lint", "src/a.rs", 5, 5, AnnotationSeverity::Failure),
        annotation("lint", "src/a.rs", 6, 6, AnnotationSeverity::Warning),
        annotation("build", "src/b.rs", 7, 7, AnnotationSeverity::Notice),
    ]);
    assert_eq!(listing.counts.failure, 1);
    assert_eq!(listing.counts.warning, 1);
    assert_eq!(listing.counts.notice, 1);
    assert!(listing.has_failures());

    let filtered = AnnotationFilter {
        severity: Some(AnnotationSeverity::Warning),
        ..AnnotationFilter::default()
    }
    .apply(listing);

    assert_eq!(filtered.annotations.len(), 1);
    assert_eq!(filtered.counts.failure, 0);
    assert_eq!(filtered.counts.warning, 1);
    assert!(!filtered.has_failures());
}

#[test]
fn a_check_filter_matches_part_of_the_name_case_insensitively() {
    let listing = listing(vec![
        annotation(
            "Clippy (ubuntu)",
            "src/a.rs",
            5,
            5,
            AnnotationSeverity::Failure,
        ),
        annotation("Build", "src/b.rs", 7, 7, AnnotationSeverity::Failure),
    ]);

    let filtered = AnnotationFilter {
        check: Some("clippy".to_owned()),
        ..AnnotationFilter::default()
    }
    .apply(listing);

    assert_eq!(filtered.annotations.len(), 1);
    assert_eq!(filtered.annotations[0].check, "Clippy (ubuntu)");
}

#[test]
fn a_path_filter_accepts_a_directory_prefix() {
    let listing = listing(vec![
        annotation("lint", "src/deep/a.rs", 5, 5, AnnotationSeverity::Failure),
        annotation("lint", "docs/b.md", 7, 7, AnnotationSeverity::Failure),
    ]);

    let filtered = AnnotationFilter {
        path: Some(PathBuf::from("src")),
        ..AnnotationFilter::default()
    }
    .apply(listing);

    assert_eq!(filtered.annotations.len(), 1);
    assert_eq!(filtered.annotations[0].path, PathBuf::from("src/deep/a.rs"));
}

#[test]
fn placement_separates_a_changed_line_from_a_changed_file_from_an_untouched_one() {
    let mut listing = listing(vec![
        annotation("lint", "src/a.rs", 5, 5, AnnotationSeverity::Failure),
        annotation("lint", "src/a.rs", 90, 90, AnnotationSeverity::Failure),
        annotation("lint", "src/never.rs", 1, 1, AnnotationSeverity::Failure),
        annotation("lint", "src/b.rs", 3, 3, AnnotationSeverity::Failure),
    ]);
    let index = index(&["src/a.rs", "src/b.rs"]);
    let mut visible = HashMap::new();
    drop(visible.insert(
        PathBuf::from("src/a.rs"),
        visible_lines(&document(&[Some(4), Some(5), Some(6)])),
    ));

    mark_diff_coverage(&mut listing, &index, &visible);

    let placements: Vec<AnnotationPlacement> = listing
        .annotations
        .iter()
        .map(|annotation| annotation.placement)
        .collect();
    assert_eq!(
        placements,
        vec![
            AnnotationPlacement::InDiff,
            AnnotationPlacement::OutsideHunks,
            AnnotationPlacement::Unknown,
            AnnotationPlacement::OutsideDiff,
        ]
    );
    assert_eq!(listing.counts.in_diff, 1);
    assert_eq!(listing.counts.outside_diff, 3);
}

#[test]
fn a_multi_line_annotation_lands_in_the_diff_when_any_of_its_lines_does() {
    let mut listing = listing(vec![annotation(
        "lint",
        "src/a.rs",
        10,
        14,
        AnnotationSeverity::Warning,
    )]);
    let mut visible = HashMap::new();
    drop(visible.insert(
        PathBuf::from("src/a.rs"),
        visible_lines(&document(&[Some(14), Some(15)])),
    ));

    mark_diff_coverage(&mut listing, &index(&["src/a.rs"]), &visible);

    assert_eq!(
        listing.annotations[0].placement,
        AnnotationPlacement::InDiff
    );
}

#[test]
fn a_file_level_annotation_on_a_changed_file_is_in_the_diff() {
    let mut listing = listing(vec![annotation(
        "lint",
        "src/a.rs",
        0,
        0,
        AnnotationSeverity::Notice,
    )]);
    let mut visible = HashMap::new();
    drop(visible.insert(PathBuf::from("src/a.rs"), visible_lines(&document(&[]))));

    mark_diff_coverage(&mut listing, &index(&["src/a.rs"]), &visible);

    assert_eq!(
        listing.annotations[0].placement,
        AnnotationPlacement::InDiff
    );
    assert_eq!(listing.annotations[0].location(), "src/a.rs");
}

#[test]
fn only_annotated_paths_the_pull_request_changes_are_worth_a_patch() {
    let listing = listing(vec![
        annotation("lint", "src/a.rs", 5, 5, AnnotationSeverity::Failure),
        annotation("lint", "src/a.rs", 6, 6, AnnotationSeverity::Failure),
        annotation("lint", "src/never.rs", 1, 1, AnnotationSeverity::Failure),
    ]);

    assert_eq!(
        annotated_paths(&listing, &index(&["src/a.rs", "src/b.rs"])),
        vec![PathBuf::from("src/a.rs")]
    );
}

#[test]
fn an_in_diff_filter_drops_everything_the_patch_does_not_show() {
    let mut listing = listing(vec![
        annotation("lint", "src/a.rs", 5, 5, AnnotationSeverity::Failure),
        annotation("lint", "src/never.rs", 1, 1, AnnotationSeverity::Failure),
    ]);
    let mut visible = HashMap::new();
    drop(visible.insert(
        PathBuf::from("src/a.rs"),
        visible_lines(&document(&[Some(5)])),
    ));
    mark_diff_coverage(&mut listing, &index(&["src/a.rs"]), &visible);

    let filtered = AnnotationFilter {
        in_diff_only: true,
        ..AnnotationFilter::default()
    }
    .apply(listing);

    assert_eq!(filtered.annotations.len(), 1);
    assert_eq!(filtered.counts.outside_diff, 0);
}

#[test]
fn grouping_keeps_the_stable_order_inside_each_group() {
    let listing = listing(vec![
        annotation("lint", "src/b.rs", 2, 2, AnnotationSeverity::Warning),
        annotation("build", "src/a.rs", 9, 9, AnnotationSeverity::Notice),
        annotation("lint", "src/a.rs", 1, 1, AnnotationSeverity::Failure),
    ]);

    let by_file = listing.grouped(AnnotationGrouping::File);
    assert_eq!(
        by_file
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["src/a.rs", "src/b.rs"]
    );
    assert_eq!(by_file[0].1.len(), 2);
    assert_eq!(by_file[0].1[0].severity, AnnotationSeverity::Failure);

    let by_check = listing.grouped(AnnotationGrouping::Check);
    assert_eq!(
        by_check
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["build", "lint"]
    );

    let by_severity = listing.grouped(AnnotationGrouping::Severity);
    assert_eq!(
        by_severity
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["failure", "notice", "warning"]
    );
}

#[test]
fn a_headline_prefers_the_title_and_falls_back_to_the_first_message_line() {
    let mut annotation = annotation("lint", "src/a.rs", 1, 1, AnnotationSeverity::Failure);
    assert_eq!(annotation.headline(), "lint finding");

    annotation.title = "   ".to_owned();
    annotation.message = "\n\nUse a slice here\nand not a vector".to_owned();
    assert_eq!(annotation.headline(), "Use a slice here");
}

#[test]
fn a_location_names_a_line_a_range_or_the_whole_file() {
    assert_eq!(
        annotation("lint", "src/a.rs", 7, 7, AnnotationSeverity::Notice).location(),
        "src/a.rs:7"
    );
    assert_eq!(
        annotation("lint", "src/a.rs", 7, 9, AnnotationSeverity::Notice).location(),
        "src/a.rs:7-9"
    );
    assert_eq!(
        annotation("lint", "src/a.rs", 0, 0, AnnotationSeverity::Notice).location(),
        "src/a.rs"
    );
}

#[test]
fn the_listing_is_capped_and_says_so_rather_than_dropping_rows_quietly() {
    let many: Vec<CheckAnnotation> = (0..MAX_ANNOTATIONS + 10)
        .map(|index| {
            annotation(
                "lint",
                &format!("src/file-{index:04}.rs"),
                1,
                1,
                AnnotationSeverity::Warning,
            )
        })
        .collect();

    let listing = listing(many);

    assert_eq!(listing.annotations.len(), MAX_ANNOTATIONS);
    assert_eq!(listing.counts.warning, MAX_ANNOTATIONS);
    assert_eq!(
        listing.schema_version,
        PullRequestAnnotations::SCHEMA_VERSION
    );
}

#[test]
fn every_placement_and_severity_names_itself() {
    let placements = [
        (AnnotationPlacement::InDiff, "in diff", true),
        (AnnotationPlacement::OutsideHunks, "outside hunks", false),
        (AnnotationPlacement::OutsideDiff, "outside diff", false),
        (AnnotationPlacement::Unknown, "unplaced", false),
    ];
    for (placement, word, in_diff) in placements {
        assert_eq!(placement.word(), word);
        assert_eq!(placement.is_in_diff(), in_diff);
    }
    let severities = [
        (AnnotationSeverity::Failure, "failure", "x"),
        (AnnotationSeverity::Warning, "warning", "!"),
        (AnnotationSeverity::Notice, "notice", "i"),
    ];
    for (severity, word, glyph) in severities {
        assert_eq!(severity.word(), word);
        assert_eq!(severity.glyph(), glyph);
    }
}
