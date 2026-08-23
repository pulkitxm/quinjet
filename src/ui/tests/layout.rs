use super::*;

#[test]
fn header_opens_the_project_menu_from_the_name_and_links_the_branch() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.branch.head = "feature/link".to_owned();
    app.local_github_repository = Some(GitHubRepository {
        name_with_owner: "acme/repo".to_owned(),
        url: "https://github.com/acme/repo".to_owned(),
        remotes: vec!["origin".to_owned()],
    });
    let backend = TestBackend::new(160, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(app.geometry.link_hits.iter().any(|hit| matches!(
        &hit.target,
        OpenTarget::Browser(url)
            if url == "https://github.com/acme/repo/tree/feature/link"
    )));
    assert!(!app.geometry.link_hits.iter().any(|hit| matches!(
        &hit.target,
        OpenTarget::Browser(url) if url == "https://github.com/acme/repo"
    )));

    let name_hit = app
        .geometry
        .project_hits
        .iter()
        .copied()
        .min_by_key(|area| area.y)
        .expect("repository name opens the project menu");
    let buffer = terminal.backend().buffer();
    let mut name = String::new();
    for x in name_hit.x..name_hit.right() {
        let cell = &buffer[(x, name_hit.y)];
        name.push_str(cell.symbol());
        assert!(cell.modifier.contains(Modifier::UNDERLINED));
    }
    assert_eq!(name, "repo");

    let rendered = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(!rendered.contains("/tmp/repo"));
    assert!(!rendered.contains("\x1b]8;;"));

    app.configure_mouse_capture(false);
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("\x1b]8;;https://github.com/acme/repo/tree/feature/link\x1b\\"));

    app.view = View::History;
    app.history = vec![crate::git::history::Commit {
        id: "abc123".to_owned(),
        short_id: "abc123".to_owned(),
        parent_ids: Vec::new(),
        author: String::new(),
        author_email: String::new(),
        authored_at: String::new(),
        committer: String::new(),
        committer_email: String::new(),
        committed_at: String::new(),
        relative_date: "now".to_owned(),
        subject: "Linked commit".to_owned(),
        decorations: Vec::new(),
    }];
    terminal.clear().unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("\x1b]8;;https://github.com/acme/repo/commit/abc123\x1b\\"));
}

#[test]
fn history_rows_give_the_subject_hash_and_live_age_their_own_space() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::History;
    app.history = vec![crate::git::history::Commit {
        id: "abc123456789".to_owned(),
        short_id: "abc1234".to_owned(),
        parent_ids: Vec::new(),
        author: String::new(),
        author_email: String::new(),
        authored_at: String::new(),
        committer: String::new(),
        committer_email: String::new(),
        committed_at: "2000-01-01T00:00:00Z".to_owned(),
        relative_date: String::new(),
        subject: "feat: use the full sidebar line".to_owned(),
        decorations: Vec::new(),
    }];
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let hit = app
        .geometry
        .sidebar_hits
        .iter()
        .find(|hit| matches!(hit.target, SidebarHit::Commit(0)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let row_text = |y| {
        (hit.area.x..hit.area.right())
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    };
    let subject = row_text(hit.area.y);
    let metadata = row_text(hit.area.y.saturating_add(1));

    assert_eq!(hit.area.height, 2);
    assert!(subject.contains("feat: use the full sidebar line"));
    assert!(metadata.contains("abc1234"));
    assert!(metadata.contains("years ago"));
}

#[test]
fn footer_underlines_only_the_worktree_count() {
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.branch.head = "main".to_owned();
    let tree = |path: &str, current: bool| crate::git::Worktree {
        path: PathBuf::from(path),
        head: "abcdef0123456789".to_owned(),
        updated_at: Some("2026-08-22T18:00:00Z".to_owned()),
        updated_unix: Some(1_776_964_800),
        branch: Some("main".to_owned()),
        current,
        bare: false,
        detached: false,
        locked: None,
        prunable: None,
    };
    app.worktrees = vec![
        tree("/tmp/repo", true),
        tree("/tmp/repo-a", false),
        tree("/tmp/repo-b", false),
    ];
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let hit = app
        .geometry
        .project_hits
        .iter()
        .copied()
        .max_by_key(|area| area.y)
        .expect("footer worktree hit");
    let buffer = terminal.backend().buffer();
    let mut label = String::new();
    for x in hit.x..hit.right() {
        let cell = &buffer[(x, hit.y)];
        label.push_str(cell.symbol());
        assert!(
            cell.modifier.contains(Modifier::UNDERLINED),
            "worktree label should be underlined"
        );
    }
    assert_eq!(label, "3 worktrees");
    let before = &buffer[(hit.x.saturating_sub(1), hit.y)];
    assert_eq!(before.symbol(), " ");
    assert!(!before.modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn preview_selection_highlights_only_the_pane_where_it_started() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    let selection = crate::app::TextSelection {
        pane: Rect::new(10, 0, 10, 4),
        anchor: (12, 1),
        head: (18, 2),
    };

    terminal
        .draw(|frame| draw_text_selection(frame, selection, &theme))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(12, 1)].bg, theme.selected);
    assert_eq!(buffer[(18, 2)].bg, theme.selected);
    assert_ne!(buffer[(9, 1)].bg, theme.selected);
    assert_ne!(buffer[(9, 2)].bg, theme.selected);
}

#[test]
fn changes_view_exposes_vscode_style_file_group_and_toolbar_actions() {
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.changes = vec![
        Change {
            path: PathBuf::from("src/main.rs"),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Modified,
        },
        Change {
            path: PathBuf::from("README.md"),
            original_path: None,
            area: ChangeArea::Staged,
            status: ChangeStatus::Modified,
        },
        Change {
            path: PathBuf::from("notes.txt"),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Untracked,
        },
    ];
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();

    assert!(rendered.contains("[+]"));
    assert!(rendered.contains("[−]"));
    assert!(rendered.contains("Commit"));
    assert!(rendered.contains('▶'));
    assert!(rendered.contains("[ ]"));
    assert!(!rendered.contains("[c] Commit"));
    assert!(!rendered.contains("[S] Stashes"));
    assert!(!rendered.contains("[d] Compare Branch"));
    assert!(!rendered.contains("UNTRACKED CHANGES"));
    assert!(rendered.contains("CHANGES"));
    assert!(rendered.contains("notes.txt"));
    assert!(rendered.contains("\u{e7a8} main.rs"));
    assert!(rendered.contains("\u{eeab} README.md"));
    assert!(!rendered.contains('›'));
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::Stage(0)))
    );
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::Unstage(1)))
    );
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::ToggleCheck(_)))
    );
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::Primary))
    );
}

#[test]
fn middle_truncation_respects_display_width() {
    let result = truncate_middle("src/a-very-long-file-name.rs", 14);
    assert!(result.width() <= 14);
    assert!(result.contains('…'));
    assert!(result.ends_with("me.rs"));
}

#[test]
fn diff_rows_are_cached_between_draws_and_rebuilt_on_document_change() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.set_document(DiffDocument {
        title: "Changes".to_owned(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_file_header("src/main.rs", 1, 0),
            test_line(DiffLineKind::Context, "same"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    });
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        !app.unified_diff_rows.is_empty(),
        "the first draw builds the unified rows"
    );
    let key = app.diff_rows_key;
    let pointer = app.unified_diff_rows.as_ptr();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(
        app.diff_rows_key, key,
        "an unchanged document keeps its key"
    );
    assert_eq!(
        app.unified_diff_rows.as_ptr(),
        pointer,
        "an unchanged document reuses its rows"
    );

    app.set_document(DiffDocument::empty("Changes", "Working tree clean"));
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_ne!(
        app.diff_rows_key, key,
        "replacing the document rebuilds the rows"
    );
    assert_eq!(
        app.unified_diff_rows.len(),
        app.document.lines.len(),
        "the rebuilt rows describe the new document"
    );
}

#[test]
fn side_by_side_pairs_replacements() {
    let document = DiffDocument {
        title: String::new(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_line(DiffLineKind::Removed, "old one"),
            test_line(DiffLineKind::Removed, "old two"),
            test_line(DiffLineKind::Added, "new one"),
            test_line(DiffLineKind::Context, "same"),
        ],
    };
    let app = App::new("/tmp/repo", "repo");
    let rows = side_by_side_rows(&document, &app);
    assert_eq!(rows.len(), 3);
    let line_text = |index: usize| document.lines.get(index).map(DiffLine::text).unwrap();
    let SideBySideRow::Split(old, new) = &rows[0] else {
        panic!("expected a split diff row");
    };
    assert_eq!(line_text(old.unwrap()), "old one");
    assert_eq!(line_text(new.unwrap()), "new one");
    let SideBySideRow::Split(_, new) = &rows[1] else {
        panic!("expected a split diff row");
    };
    assert!(new.is_none());
    let SideBySideRow::Split(old, _) = &rows[2] else {
        panic!("expected a split diff row");
    };
    assert_eq!(line_text(old.unwrap()), "same");
}
