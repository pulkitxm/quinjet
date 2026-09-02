use super::*;

#[test]
fn the_mouse_can_be_released_so_the_terminal_can_select_text() {
    let mut app = app_with_changes();
    let now = Instant::now();
    assert!(app.mouse_capture);

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), now);
    assert!(!app.mouse_capture);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SetMouseCapture(false)]
    ));
    assert!(
        app.toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("select and copy")),
        "the reader is told what releasing the mouse just gave them"
    );

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), now);
    assert!(app.mouse_capture);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SetMouseCapture(true)]
    ));
}

#[test]
fn the_mouse_is_released_even_while_the_number_field_has_focus() {
    let mut app = app_with_changes();
    let now = Instant::now();
    app.pull_request_lookup_active = true;

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), now);
    assert!(!app.mouse_capture);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SetMouseCapture(false)]
    ));
    assert!(
        app.pull_request_lookup_active,
        "releasing the mouse does not close the field"
    );
}

#[test]
fn z_hides_and_restores_sidebar_without_replacing_hunk_shortcuts() {
    let mut app = app_with_changes();
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
    assert!(app.sidebar_hidden);
    assert_eq!(app.focus, Focus::Content);
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
    assert!(!app.sidebar_hidden);
    assert_eq!(app.focus, Focus::Sidebar);
}

#[test]
fn z_works_while_the_pull_request_number_input_has_focus() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.switch_view(View::PullRequests, &mut Vec::new());
    assert!(app.pull_request_lookup_active);

    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
    assert!(app.sidebar_hidden);
    assert_eq!(app.focus, Focus::Content);
    assert!(!app.pull_request_lookup_active);

    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
    assert!(!app.sidebar_hidden);
    assert_eq!(app.focus, Focus::Sidebar);
}

#[test]
fn change_sections_share_navigation_folding_and_membership() {
    let mut app = app_with_changes();
    let now = Instant::now();
    app.status.changes.push(Change {
        path: PathBuf::from("new.txt"),
        original_path: None,
        area: ChangeArea::Unstaged,
        status: ChangeStatus::Untracked,
    });
    app.selected_change_section = Some(ChangeSection::Staged);

    app.navigate(1, now);
    assert!(app.selected_change_section.is_none());
    assert_eq!(app.selected_change().unwrap().area, ChangeArea::Staged);
    app.navigate(1, now);
    assert_eq!(app.selected_change_section, Some(ChangeSection::Unstaged));
    assert_eq!(app.selected_section_changes().len(), 2);
    assert!(
        app.selected_section_changes()
            .iter()
            .any(|change| change.path == Path::new("new.txt")
                && change.status == ChangeStatus::Untracked)
    );

    assert!(app.toggle_selected_change_section());
    assert!(
        app.collapsed_change_sections
            .contains(&ChangeSection::Unstaged)
    );
    assert!(!app.change_rows().iter().any(|row| matches!(
        row,
        ChangeRow::Change {
            section: ChangeSection::Unstaged,
            ..
        }
    )));

    let rows = app.change_rows();
    let section_count = rows
        .iter()
        .filter(|row| matches!(row, ChangeRow::Section { .. }))
        .count();
    let spacer_count = rows
        .iter()
        .filter(|row| matches!(row, ChangeRow::Spacer))
        .count();
    assert_eq!(
        spacer_count,
        section_count.saturating_sub(1),
        "adjacent change sections should be separated by a blank spacer row"
    );
}

#[test]
fn clicking_a_file_stage_action_queues_only_that_path() {
    let mut app = app_with_changes();
    app.geometry.scm_action_hits = vec![ScmActionHit {
        area: Rect::new(30, 8, 4, 1),
        action: ScmAction::Stage(0),
    }];
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 31,
        row: 8,
        modifiers: KeyModifiers::NONE,
    };

    let effects = app.handle_mouse(click, Instant::now());

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::Operate {
                operation: GitOperation::Stage(paths),
                ..
            } if paths == &[PathBuf::from("src/main.rs")]
        )
    ));
}

#[test]
fn clicking_link_text_opens_its_target_before_the_containing_row() {
    let mut app = app_with_changes();
    app.geometry.sidebar = Rect::new(0, 0, 40, 20);
    app.geometry.sidebar_hits = vec![SidebarHitArea {
        area: Rect::new(0, 4, 40, 1),
        target: SidebarHit::Change(0),
    }];
    app.geometry.link_hits = vec![LinkHit {
        area: Rect::new(30, 4, 7, 1),
        target: OpenTarget::Browser("https://github.com/acme/widget/commit/abc".to_owned()),
    }];

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 32,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/acme/widget/commit/abc"
    ));
    assert_eq!(app.selected_change_section, None);
    assert_eq!(app.change_cursor, 0);
}

#[test]
fn a_remote_session_copies_a_clicked_link_instead_of_opening_it() {
    let mut app = app_with_changes();
    app.local_browser = false;
    app.geometry.link_hits = vec![LinkHit {
        area: Rect::new(30, 4, 7, 1),
        target: OpenTarget::Browser("https://github.com/acme/widget/commit/abc".to_owned()),
    }];

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 32,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Copy(url)] if url == "https://github.com/acme/widget/commit/abc"
    ));
    assert!(app.toast.as_ref().is_some_and(|toast| {
        toast
            .message
            .contains("https://github.com/acme/widget/commit/abc")
            && toast.message.contains("Cmd-click or Ctrl-click")
    }));
}

#[test]
fn a_local_session_opens_a_clicked_link_and_says_so() {
    let mut app = app_with_changes();
    app.geometry.link_hits = vec![LinkHit {
        area: Rect::new(30, 4, 7, 1),
        target: OpenTarget::Browser("https://github.com/acme/widget/commit/abc".to_owned()),
    }];

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 32,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );

    assert!(app.local_browser);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/acme/widget/commit/abc"
    ));
    assert!(
        app.toast.as_ref().is_some_and(
            |toast| toast.message == "Opening https://github.com/acme/widget/commit/abc"
        )
    );
}

#[test]
fn the_keyboard_open_action_follows_the_same_local_and_remote_split() {
    let now = Instant::now();
    let mut app = app_with_changes();
    app.status.branch.head = "feature/link".to_owned();
    app.local_github_repository = Some(GitHubRepository {
        name_with_owner: "acme/widget".to_owned(),
        url: "https://github.com/acme/widget".to_owned(),
        remotes: vec!["origin".to_owned()],
    });

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/acme/widget/tree/feature/link"
    ));
    assert!(
        app.toast
            .as_ref()
            .is_some_and(|toast| toast.message.starts_with("Opening "))
    );

    app.local_browser = false;
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Copy(url)] if url == "https://github.com/acme/widget/tree/feature/link"
    ));
    assert!(
        app.toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Cmd-click or Ctrl-click"))
    );
}

#[test]
fn command_hover_tracks_only_link_cells() {
    let mut app = app_with_changes();
    app.geometry.link_hits = vec![LinkHit {
        area: Rect::new(30, 4, 7, 1),
        target: OpenTarget::Browser("https://github.com/acme/widget".to_owned()),
    }];

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 32,
            row: 4,
            modifiers: KeyModifiers::SUPER,
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert_eq!(app.link_hover, Some((32, 4)));

    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 20,
            row: 4,
            modifiers: KeyModifiers::SUPER,
        },
        Instant::now(),
    );
    assert_eq!(app.link_hover, None);

    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 32,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert_eq!(app.link_hover, None);
}

#[test]
fn github_reference_targets_encode_branch_paths() {
    let mut app = App::new("/tmp/repo", "repo");
    app.local_github_repository = Some(GitHubRepository {
        name_with_owner: "acme/widget".to_owned(),
        url: "https://github.com/acme/widget".to_owned(),
        remotes: vec!["origin".to_owned()],
    });

    assert_eq!(
        app.branch_open_target("feature/fix #42"),
        Some(OpenTarget::Browser(
            "https://github.com/acme/widget/tree/feature/fix%20%2342".to_owned()
        ))
    );
    assert_eq!(
        app.pull_request_open_target(42),
        Some(OpenTarget::Browser(
            "https://github.com/acme/widget/pull/42".to_owned()
        ))
    );
}

#[test]
fn pull_request_accounts_and_both_branches_resolve_from_one_repository_root() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request = Some(pull_request(42, "Ship it", "acme/widget"));

    assert_eq!(
        app.account_open_target("octocat"),
        Some(OpenTarget::Browser("https://github.com/octocat".to_owned()))
    );
    assert_eq!(app.account_open_target("not a login"), None);
    assert_eq!(
        app.pull_request_base_branch_open_target(),
        Some(OpenTarget::Browser(
            "https://github.com/acme/widget/tree/main".to_owned()
        ))
    );
    assert_eq!(
        app.pull_request_head_branch_open_target(),
        Some(OpenTarget::Browser(
            "https://github.com/octocat/fork/tree/feature/42".to_owned()
        ))
    );
}

#[test]
fn recent_pull_requests_are_scoped_to_the_open_repository() {
    let widget = RecentPullRequest::from(&pull_request(39, "Widget", "acme/widget"));
    let unrelated = RecentPullRequest::from(&pull_request(42, "Other", "acme/other"));
    let repository = widget.repository.clone();

    let recent = recent_pull_requests_for(vec![unrelated, widget.clone()], &repository);

    assert_eq!(recent, vec![widget]);
}

#[test]
fn clicking_a_change_section_selects_and_collapses_only_that_section() {
    let mut app = app_with_changes();
    app.geometry.sidebar = Rect::new(0, 0, 40, 20);
    app.geometry.sidebar_hits = vec![SidebarHitArea {
        area: Rect::new(0, 3, 40, 1),
        target: SidebarHit::ChangeSection(ChangeSection::Unstaged),
    }];
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 8,
        row: 3,
        modifiers: KeyModifiers::NONE,
    };

    app.handle_mouse(click, Instant::now());

    assert_eq!(app.selected_change_section, Some(ChangeSection::Unstaged));
    assert!(
        app.collapsed_change_sections
            .contains(&ChangeSection::Unstaged)
    );
    assert_eq!(
        app.selected_section_changes()
            .iter()
            .map(|change| change.path.as_path())
            .collect::<Vec<_>>(),
        vec![Path::new("src/main.rs")]
    );
}

#[test]
fn clicking_a_file_header_toggles_only_that_file() {
    let mut app = App::new("/tmp/repo", "repo");
    app.document = indexed_document(&["src/main.rs", "src/lib.rs"]);
    app.geometry.content = Rect::new(20, 4, 80, 20);
    app.geometry.content_file_hits = vec![ContentFileHit {
        area: Rect::new(20, 8, 80, 1),
        path: PathBuf::from("src/main.rs"),
    }];
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 24,
        row: 8,
        modifiers: KeyModifiers::NONE,
    };

    app.handle_mouse(click, Instant::now());
    assert!(app.mouse_capture);
    assert!(
        app.collapsed_preview_files
            .contains(Path::new("src/main.rs"))
    );
    app.handle_mouse(click, Instant::now());
    assert!(
        !app.collapsed_preview_files
            .contains(Path::new("src/main.rs"))
    );
}

#[test]
fn dragging_plain_preview_content_selects_only_its_starting_pane() {
    let mut app = app_with_changes();
    app.geometry.content = Rect::new(20, 4, 80, 20);
    app.geometry.diff_divider = Some(Rect::new(59, 4, 1, 20));
    app.rendered_cells = vec![vec!['x'; 100]; 24];
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 70,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };

    let effects = app.handle_mouse(click, Instant::now());
    assert_eq!(app.focus, Focus::Content);
    assert!(app.mouse_capture);
    assert!(effects.is_empty());

    let drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 10,
        row: 12,
        modifiers: KeyModifiers::NONE,
    };
    let effects = app.handle_mouse(drag, Instant::now());
    assert!(effects.is_empty());
    assert!(app.text_selection.is_some_and(|selection| {
        selection.pane == Rect::new(60, 4, 40, 20) && selection.head == (60, 12)
    }));

    let drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 73,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(drag, Instant::now());
    let release = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 73,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    let effects = app.handle_mouse(release, Instant::now());
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Copy(text)] if text == "xxxx"
    ));
}

#[test]
fn horizontal_swipes_accept_native_and_shift_wheel_events() {
    let mut app = app_with_changes();
    let event = |kind, modifiers| MouseEvent {
        kind,
        column: 40,
        row: 12,
        modifiers,
    };
    app.text_selection = Some(TextSelection {
        pane: Rect::new(20, 4, 40, 20),
        anchor: (25, 8),
        head: (30, 8),
    });

    app.handle_mouse(
        event(MouseEventKind::ScrollRight, KeyModifiers::NONE),
        Instant::now(),
    );
    assert_eq!(app.horizontal_scroll, 3);
    assert!(app.text_selection.is_none());

    app.handle_mouse(
        event(MouseEventKind::ScrollDown, KeyModifiers::SHIFT),
        Instant::now(),
    );
    assert_eq!(app.horizontal_scroll, 6);

    app.handle_mouse(
        event(MouseEventKind::ScrollUp, KeyModifiers::SHIFT),
        Instant::now(),
    );
    assert_eq!(app.horizontal_scroll, 3);

    app.handle_mouse(
        event(MouseEventKind::ScrollLeft, KeyModifiers::NONE),
        Instant::now(),
    );
    assert_eq!(app.horizontal_scroll, 0);
}
