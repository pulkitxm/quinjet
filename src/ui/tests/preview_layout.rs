use super::*;

#[test]
fn cached_file_boundaries_follow_document_replacement_and_footers() {
    let mut app = App::new("/example/project", "example");
    app.set_document(DiffDocument {
        lines: vec![
            test_line(DiffLineKind::Meta, "intro"),
            test_file_header("first.rs", 1, 0),
            test_line(DiffLineKind::Context, "first body"),
            test_line(DiffLineKind::FileFooter, ""),
            test_line(DiffLineKind::Meta, "between files"),
            test_file_header("second.rs", 1, 0),
            test_line(DiffLineKind::Context, "second body"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
        ..DiffDocument::default()
    });
    assert_eq!(app.preview_header_indices(), &[1, 5]);
    let pointer = app.preview_header_indices().as_ptr();
    assert!(app.preview_header_at(0).is_none());
    assert_eq!(
        app.preview_header_at(2).and_then(file_header_path),
        Some("first.rs")
    );
    assert!(app.preview_header_at(3).is_none());
    assert!(app.preview_header_at(4).is_none());
    assert_eq!(
        app.preview_header_at(6).and_then(file_header_path),
        Some("second.rs")
    );
    assert!(app.preview_header_at(7).is_none());
    assert_eq!(app.preview_header_indices().as_ptr(), pointer);

    app.set_document(DiffDocument {
        lines: vec![test_file_header("replacement.rs", 1, 0)],
        ..DiffDocument::default()
    });
    assert_eq!(app.preview_header_indices(), &[0]);
    assert_eq!(
        app.preview_header_at(0).and_then(file_header_path),
        Some("replacement.rs")
    );
    assert!(!app.preview_files_collapsible());
}

#[test]
fn preserved_preview_uses_rendered_files_until_the_new_index_is_displayed() {
    let mut app = App::new("/example/project", "example");
    app.set_document(DiffDocument {
        lines: vec![
            test_file_header("first.rs", 1, 0),
            test_line(DiffLineKind::FileFooter, ""),
            test_file_header("second.rs", 1, 0),
            test_line(DiffLineKind::FileFooter, ""),
        ],
        ..DiffDocument::default()
    });
    app.files_collapsed = true;
    app.local_diff_preserving_document = true;
    app.local_diff_index = Some(crate::git::diff::DiffIndex {
        title: "replacement".to_owned(),
        files: Vec::new(),
        truncated: false,
        commit_details: None,
    });
    assert!(app.preview_files_collapsible());
    assert!(app.preview_files_all_collapsed());
    app.expanded_preview_files.insert("second.rs".into());
    assert!(!app.preview_files_all_collapsed());
    app.set_document(DiffDocument::empty("replacement", "no files"));
    assert!(!app.preview_files_collapsible());
    assert!(!app.preview_files_all_collapsed());
}
