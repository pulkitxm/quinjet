use super::*;

#[test]
fn pull_request_folders_render_as_clickable_collapse_controls() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_files = ["src/app.rs", "src/git/diff.rs"]
        .into_iter()
        .map(|path| PullRequestFile {
            path: std::path::PathBuf::from(path),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut hits = Vec::new();

    terminal
        .draw(|frame| {
            hits = draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
        })
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("⌄ src/"));
    assert!(rendered.contains("\u{e7a8} app.rs"));
    assert!(rendered.contains("app.rs"));
    assert!(hits.iter().any(|hit| {
        matches!(
            &hit.target,
            SidebarHit::PullRequestDirectory(path) if path == Path::new("src")
        )
    }));

    app.collapsed_pull_request_directories
        .insert(std::path::PathBuf::from("src"));
    app.pull_request_tree.clear();
    terminal.clear().unwrap();
    terminal
        .draw(|frame| {
            draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
        })
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("› src/"));
    assert!(!rendered.contains("app.rs"));
}

#[test]
fn pull_request_file_tree_virtualizes_a_thousand_files() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_files = (0..1_000)
        .map(|index| PullRequestFile {
            path: std::path::PathBuf::from(format!(
                "packages/package-{index:04}/src/file-{index:04}.rs"
            )),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    app.pull_request_total_files = app.pull_request_files.len();
    app.pull_request_file_cursor = 999;
    let rows = app.pull_request_tree_entries();
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row, PullRequestTreeEntry::File { .. }))
            .count(),
        1_000
    );
    app.pull_request_tree_cursor = rows
        .iter()
        .position(|row| matches!(row, PullRequestTreeEntry::File { index: 999, .. }))
        .unwrap();

    let backend = TestBackend::new(48, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
        })
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();

    assert!(app.sidebar_offset > 0);
    assert!(rendered.contains("file-0999.rs"));
}

#[test]
fn hides_raw_hunk_coordinates_in_both_diff_layouts() {
    let document = DiffDocument {
        title: String::new(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_file_header("src/main.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -10,2 +10,3 @@ fn main()"),
            test_line(DiffLineKind::Context, "same"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };

    let app = App::new("/tmp/repo", "repo");
    assert_eq!(unified_row_indices(&document, &app), vec![0, 2, 3]);
    assert!(side_by_side_rows(&document, &app).iter().all(|row| {
        !matches!(
            row,
            SideBySideRow::Full { index, .. }
                if document
                    .lines
                    .get(*index)
                    .is_some_and(|line| line.kind == DiffLineKind::HunkHeader)
        )
    }));
}

#[test]
fn commit_preview_renders_details_once_and_names_each_file_pane() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::History;
    app.focus = Focus::Content;
    app.status.branch.head = "main".to_owned();
    app.history_branch = Some(HistoryBranch {
        name: "origin/topic".to_owned(),
        reference: "refs/remotes/origin/topic".to_owned(),
        current: false,
        remote: true,
        relative_date: "now".to_owned(),
        short_id: "abc1234".to_owned(),
    });
    app.document = DiffDocument {
        title: "abc1234 — Improve history".to_owned(),
        truncated: false,
        commit_details: Some(CommitDetails {
            id: "abc123456789".to_owned(),
            subject: "Improve history".to_owned(),
            author: "Ada".to_owned(),
            author_email: "ada@example.com".to_owned(),
            authored_at: "2026-01-02T03:04:05Z".to_owned(),
            committer: "Grace".to_owned(),
            committer_email: "grace@example.com".to_owned(),
            committed_at: "2026-01-02T04:05:06Z".to_owned(),
        }),
        pull_request_details: None,
        lines: vec![
            test_file_header("src/main.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -1,0 +1 @@"),
            test_line(DiffLineKind::Added, "fn main() {}"),
            test_line(DiffLineKind::FileFooter, ""),
            test_file_header("README.md", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -1,0 +1 @@"),
            test_line(DiffLineKind::Added, "# Quinjet"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };
    let backend = TestBackend::new(140, 32);
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

    assert_eq!(rendered.matches("Commit details").count(), 1);
    assert!(rendered.contains("origin/topic"));
    assert!(rendered.contains("[b branch]"));
    assert!(rendered.contains("src/main.rs"));
    assert!(rendered.contains("README.md"));
    assert!(!rendered.contains("@@"));
    assert!(!rendered.contains('◆'));
    assert!(!rendered.contains('░'));
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
#[test]
fn pull_request_preview_renders_cross_remote_metadata_and_diff() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.focus = Focus::Content;
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_exact_number = Some(42);
    app.pull_request_lookup = crate::app::TextBuffer::new("42");
    app.pull_request = Some(crate::git::github::PullRequest {
        number: 42,
        title: "Ship the rocket".to_owned(),
        description:
            "## Summary\n- Launch **safely** after all checks pass\n- Keep raw `gh` output bounded"
                .to_owned(),
        author: "octocat".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: String::new(),
        updated_at: String::new(),
        url: "https://github.com/acme/widget/pull/42".to_owned(),
        base_ref: "main".to_owned(),
        base_oid: String::new(),
        head_ref: "feature/rocket".to_owned(),
        head_oid: String::new(),
        base_repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["upstream".to_owned()],
        },
        head_repository: Some("octocat/widget".to_owned()),
        head_remotes: vec!["origin".to_owned(), "publish".to_owned()],
        is_cross_repository: true,
        additions: 101,
        deletions: 20,
        changed_files: 1,
    });
    app.pull_request_repository = Some(GitHubRepository {
        name_with_owner: "acme/widget".to_owned(),
        url: "https://github.com/acme/widget".to_owned(),
        remotes: vec!["upstream".to_owned()],
    });
    app.pull_request_files = vec![PullRequestFile {
        path: std::path::PathBuf::from("src/rocket.rs"),
        old_path: None,
        status: PullRequestFileStatus::Added,
        counts: None,
    }];
    app.pull_request_total_files = 1;
    app.document = DiffDocument {
        title: "PR #42 — Ship the rocket".to_owned(),
        truncated: false,
        commit_details: None,
        pull_request_details: Some(PullRequestDetails {
            number: 42,
            title: "Ship the rocket".to_owned(),
            description: "Launch safely after all checks pass".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            updated_at: "2026-08-13T12:00:00Z".to_owned(),
            url: "https://github.com/acme/widget/pull/42".to_owned(),
            base_repository: "acme/widget".to_owned(),
            base_ref: "main".to_owned(),
            base_remotes: vec!["upstream".to_owned()],
            head_repository: Some("octocat/widget".to_owned()),
            head_ref: "feature/rocket".to_owned(),
            head_remotes: vec!["origin".to_owned(), "publish".to_owned()],
            is_cross_repository: true,
            changed_files: 1,
            additions: 101,
            deletions: 20,
            selected_file: Some("src/rocket.rs".to_owned()),
            selected_file_additions: 1,
            selected_file_deletions: 0,
        }),
        lines: vec![
            test_file_header("src/rocket.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -0,0 +1 @@"),
            test_line(DiffLineKind::Added, "launch();"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };
    let backend = TestBackend::new(160, 34);
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

    assert!(rendered.contains("Pull Requests"));
    assert!(rendered.contains("Files 1"));
    assert!(rendered.contains("PR"));
    assert!(rendered.contains("rocket.rs"));
    assert!(rendered.contains("launch();"));
    assert!(!rendered.contains("Page"));
    assert!(!rendered.contains("files on page"));
    assert!(!rendered.contains("@@"));
    for expected in [
        "https://github.com/acme/widget",
        "https://github.com/acme/widget/pull/42",
        "https://github.com/octocat",
        "https://github.com/octocat/widget/tree/feature/rocket",
        "https://github.com/acme/widget/tree/main",
    ] {
        assert!(
            app.geometry
                .link_hits
                .iter()
                .any(|hit| { matches!(&hit.target, OpenTarget::Browser(url) if url == expected) }),
            "missing link target {expected}"
        );
    }

    app.pull_request_section = PullRequestSection::Overview;
    app.pull_request_checks = vec![PullRequestCheck {
        name: "CI / ubuntu".to_owned(),
        workflow: "CI".to_owned(),
        state: "SUCCESS".to_owned(),
        status: PullRequestCheckStatus::Passed,
        description: "All jobs passed".to_owned(),
        link: "https://github.com/acme/widget/actions/1".to_owned(),
        started_at: "2026-08-13T12:00:00Z".to_owned(),
        completed_at: "2026-08-13T12:01:00Z".to_owned(),
    }];
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
    assert!(rendered.contains("Conversation"));
    assert!(!rendered.contains(['⟳', '↻', '↺']));
    assert!(rendered.contains("CI / ubuntu"));
    assert!(rendered.contains("Ship the rocket"));
    assert!(rendered.contains("octocat/widget:feature/rocket"));
    assert!(rendered.contains("acme/widget:main"));
    assert!(rendered.contains("+101"));
    assert!(rendered.contains("-20"));
    assert!(
        rendered.contains("Launch"),
        "the pull-request body is part of the default view"
    );

    app.pull_request_conversation_loading = true;
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
    assert!(rendered.contains("loading"));
    assert!(!rendered.contains(['⟳', '↻', '↺']));
}
