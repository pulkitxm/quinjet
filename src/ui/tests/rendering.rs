use super::*;

#[test]
fn file_header_right_aligns_colored_line_counts() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let header = test_file_header("src/main.rs", 12, 3);
    let theme = Theme::default();
    let backend = TestBackend::new(40, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new("/tmp/repo", "repo");
    app.document.lines = vec![header.clone()];
    terminal
        .draw(|frame| draw_file_header(frame, frame.area(), &header, &app, &theme))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rendered: String = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    let addition_column = rendered
        .chars()
        .position(|character| character == '+')
        .unwrap();
    let deletion_column = rendered
        .chars()
        .position(|character| character == '-')
        .unwrap();

    assert!(rendered.ends_with("+12 -3 ─"), "{rendered:?}");
    assert!(rendered.contains('─'));
    assert!(rendered.contains("\u{e7a8} src/main.rs"));
    assert!(!rendered.contains(['┌', '┐', '└', '┘', '│']));
    assert!(!rendered.contains('⌄'));
    assert!(!rendered.contains('›'));
    assert_eq!(buffer[(cells(addition_column), 0)].fg, theme.added);
    assert_eq!(buffer[(cells(deletion_column), 0)].fg, theme.removed);
}

#[test]
fn file_footer_draws_one_horizontal_separator_without_vertical_edges() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(12, 1)).unwrap();
    terminal
        .draw(|frame| draw_file_footer(frame, frame.area(), &theme))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert!(
        buffer
            .content()
            .iter()
            .all(|cell| cell.symbol() == "─" && cell.fg == theme.border)
    );
}

#[test]
fn skips_intraline_work_for_very_long_rows() {
    let old = test_line(
        DiffLineKind::Removed,
        &"a".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
    );
    let new = test_line(
        DiffLineKind::Added,
        &"b".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
    );

    assert_eq!(
        paired_intraline_emphasis(Some(&old), Some(&new)),
        (None, None)
    );
}

#[test]
fn visible_intraline_emphasis_matches_block_pairing() {
    let lines = vec![
        test_line(DiffLineKind::Context, "same"),
        test_line(DiffLineKind::Removed, "value = 1"),
        test_line(DiffLineKind::Removed, "gone"),
        test_line(DiffLineKind::Added, "value = 2"),
        test_line(DiffLineKind::Context, "same"),
        test_line(DiffLineKind::Added, "standalone"),
    ];

    let emphasis = visible_intraline_emphasis(&lines, 0..lines.len());
    assert_eq!(emphasis.get(&1), Some(&(8..9)), "removed side of the pair");
    assert_eq!(emphasis.get(&3), Some(&(8..9)), "added side of the pair");
    assert!(!emphasis.contains_key(&2), "unpaired removed line");
    assert!(
        !emphasis.contains_key(&5),
        "added run without a removed partner"
    );

    let only_added = visible_intraline_emphasis(&lines, [3_usize].into_iter());
    assert_eq!(
        only_added.get(&3),
        Some(&(8..9)),
        "partner is found outside the viewport"
    );
}

#[test]
fn document_details_participate_in_vertical_scroll() {
    assert_eq!(commit_details_row_count(30), 7);
    assert_eq!(commit_details_row_count(8), 5);
    assert_eq!(commit_details_row_count(3), 0);
    assert_eq!(pull_request_details_row_count(30), 12);
    assert_eq!(pull_request_details_row_count(9), 6);
    assert_eq!(pull_request_details_row_count(3), 0);
}

#[test]
fn pull_request_description_preview_is_bounded_and_marks_truncation() {
    let lines =
        description_preview_lines("one two three four five six seven eight nine ten", 10, 3);
    assert_eq!(lines.len(), 3);
    assert!(lines.last().unwrap().ends_with('…'));
    assert!(lines.iter().all(|line| line.width() <= 10));
}

#[test]
fn pull_request_description_preview_cleans_common_markdown() {
    let lines = description_preview_lines(
        "## Summary\n- Add a **Pull Requests** tab\n- Cache raw `gh` metadata",
        24,
        3,
    );
    let rendered = lines.join(" ");

    assert!(rendered.contains("Add a Pull Requests tab"));
    assert!(rendered.contains("•"));
    assert!(!rendered.contains("Summary"));
    assert!(!rendered.contains('#'));
    assert!(!rendered.contains('*'));
    assert!(!rendered.contains('`'));
    assert!(lines.iter().all(|line| line.width() <= 24));
}

#[test]
fn formats_zero_one_and_multiple_remote_aliases() {
    assert_eq!(remote_suffix(&[]), "");
    assert_eq!(remote_suffix(&["origin".to_owned()]), "  ·  remote origin");
    assert_eq!(
        remote_suffix(&["origin".to_owned(), "upstream".to_owned()]),
        "  ·  remotes origin, upstream"
    );
}

#[test]
fn collapse_all_keeps_only_selectable_file_headers() {
    let document = DiffDocument {
        title: String::new(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_file_header("one.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -0,0 +1 @@"),
            test_line(DiffLineKind::Added, "one"),
            test_line(DiffLineKind::FileFooter, ""),
            test_file_header("two.rs", 1, 0),
            test_line(DiffLineKind::Added, "two"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };

    let mut app = App::new("/tmp/repo", "repo");
    app.document = document.clone();
    app.files_collapsed = true;
    assert_eq!(unified_row_indices(&document, &app), vec![0, 4]);
    assert_eq!(side_by_side_rows(&document, &app).len(), 2);
}

#[test]
fn computes_vscode_style_intraline_changed_ranges() {
    assert_eq!(
        changed_ranges("const oldValue = 1;", "const newValue = 2;"),
        (Some(6..18), Some(6..18))
    );
    assert_eq!(changed_ranges("same", "same"), (None, None));
    assert_eq!(
        changed_ranges("prefix old suffix", "prefix new suffix"),
        (Some(7..10), Some(7..10))
    );
}
