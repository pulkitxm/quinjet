use super::*;

#[test]
fn text_buffer_edits_unicode_on_character_boundaries() {
    let mut buffer = TextBuffer::new("a🚀b");
    buffer.move_left();
    buffer.backspace();
    assert_eq!(buffer.value, "ab");
    buffer.insert('é');
    assert_eq!(buffer.value, "aéb");
}

#[test]
fn text_buffer_supports_word_and_line_deletion() {
    let mut buffer = TextBuffer::new("first second\nthird word");
    buffer.delete_word_backward();
    assert_eq!(buffer.value, "first second\nthird ");
    buffer.delete_to_line_start();
    assert_eq!(buffer.value, "first second\n");
    buffer.document_start();
    buffer.delete_word_forward();
    assert_eq!(buffer.value, " second\n");
    buffer.document_end();
    buffer.delete_to_line_start();
    assert_eq!(buffer.value, " second\n");
}

#[test]
fn pane_resize_is_clamped_to_usable_bounds() {
    let mut app = App::new("/tmp/repo", "repo");
    app.geometry.main = Rect::new(5, 3, 120, 30);
    app.geometry.content = Rect::new(48, 3, 77, 30);

    app.resize_sidebar(120);
    assert_eq!(app.sidebar_width, 88);
    app.resize_sidebar(6);
    assert_eq!(app.sidebar_width, 22);
    app.resize_diff(49);
    assert_eq!(app.diff_split_percent, 20);
    app.resize_diff(124);
    assert_eq!(app.diff_split_percent, 80);
}

#[test]
fn double_tapping_each_divider_restores_its_default_size() {
    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    let mut app = App::new("/tmp/repo", "repo");
    app.geometry.main = Rect::new(5, 3, 120, 30);
    app.geometry.sidebar_divider = Rect::new(82, 3, 1, 30);
    app.geometry.content = Rect::new(48, 3, 77, 30);
    app.geometry.diff_divider = Some(Rect::new(109, 3, 1, 30));
    app.sidebar_width = 77;
    app.diff_split_percent = 80;
    let now = Instant::now();

    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 82, 10), now);
    app.handle_mouse(
        mouse(MouseEventKind::Up(MouseButton::Left), 82, 10),
        now + Duration::from_millis(20),
    );
    app.handle_mouse(
        mouse(MouseEventKind::Down(MouseButton::Left), 82, 10),
        now + Duration::from_millis(120),
    );
    assert_eq!(app.sidebar_width, DEFAULT_SIDEBAR_WIDTH);
    assert_eq!(app.resize_target, None);

    app.handle_mouse(
        mouse(MouseEventKind::Down(MouseButton::Left), 109, 10),
        now + Duration::from_millis(600),
    );
    app.handle_mouse(
        mouse(MouseEventKind::Up(MouseButton::Left), 109, 10),
        now + Duration::from_millis(620),
    );
    app.handle_mouse(
        mouse(MouseEventKind::Down(MouseButton::Left), 109, 10),
        now + Duration::from_millis(720),
    );
    assert_eq!(app.diff_split_percent, DEFAULT_DIFF_SPLIT_PERCENT);
    assert_eq!(app.resize_target, None);
}

#[test]
fn filters_changes_without_losing_underlying_index() {
    let mut app = app_with_changes();
    app.filter = "read".to_owned();
    assert_eq!(app.visible_change_indices(), vec![1]);
    assert_eq!(
        app.selected_change().unwrap().path,
        PathBuf::from("README.md")
    );
}

#[test]
fn discard_on_a_conflict_opens_resolution_instead() {
    let mut app = app_with_changes();
    app.status.changes[0].area = ChangeArea::Conflict;
    app.status.changes[0].status = ChangeStatus::Conflicted;

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert!(matches!(app.modal, Some(Modal::Conflict { .. })));
}

#[test]
fn a_section_checkbox_checks_and_clears_its_whole_group() {
    let mut app = app_with_changes();
    let mut effects = Vec::new();

    app.handle_scm_action(
        ScmAction::ToggleCheckSection(ChangeSection::Unstaged),
        &mut effects,
        Instant::now(),
    );
    assert_eq!(app.checked_change_count(), 1);
    assert_eq!(app.section_check_label(ChangeSection::Unstaged), "[x]");
    assert_eq!(app.section_check_label(ChangeSection::Staged), "[ ]");

    app.handle_scm_action(
        ScmAction::ToggleCheckSection(ChangeSection::Staged),
        &mut effects,
        Instant::now(),
    );
    app.status.changes.push(Change {
        path: PathBuf::from("docs/notes.md"),
        original_path: None,
        area: ChangeArea::Staged,
        status: ChangeStatus::Modified,
    });
    assert_eq!(app.section_check_label(ChangeSection::Staged), "[-]");

    app.handle_scm_action(
        ScmAction::ToggleCheckSection(ChangeSection::Staged),
        &mut effects,
        Instant::now(),
    );
    assert_eq!(app.section_check_label(ChangeSection::Staged), "[x]");
    assert_eq!(app.checked_change_count(), 3);

    app.handle_scm_action(
        ScmAction::ToggleCheckSection(ChangeSection::Staged),
        &mut effects,
        Instant::now(),
    );
    assert_eq!(app.section_check_label(ChangeSection::Staged), "[ ]");
    assert_eq!(app.checked_change_count(), 1);
    assert!(effects.is_empty());
}

#[test]
fn the_revert_button_asks_about_the_checked_files() {
    let mut app = app_with_changes();
    let mut effects = Vec::new();
    app.handle_scm_action(
        ScmAction::ToggleCheckSection(ChangeSection::Unstaged),
        &mut effects,
        Instant::now(),
    );

    app.handle_scm_action(ScmAction::RevertChecked, &mut effects, Instant::now());

    let Some(Modal::Confirm { title, action, .. }) = app.modal else {
        panic!("the revert button must ask first");
    };
    assert_eq!(title, "Revert Checked Files?");
    let ConfirmAction::Operate(GitOperation::Discard(changes)) = action else {
        panic!("the confirmation must carry a discard operation");
    };
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, PathBuf::from("src/main.rs"));
    assert!(effects.is_empty());
}

#[test]
fn shift_x_asks_to_remove_the_selected_file() {
    let mut app = app_with_changes();

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
        Instant::now(),
    );

    assert!(effects.is_empty());
    let Some(Modal::Confirm { title, action, .. }) = app.modal else {
        panic!("removing the selected file must ask first");
    };
    assert_eq!(title, "Remove File?");
    let ConfirmAction::Operate(GitOperation::Remove(paths)) = action else {
        panic!("the confirmation must carry a remove operation");
    };
    assert_eq!(paths, vec![PathBuf::from("src/main.rs")]);
}

#[test]
fn checked_files_drive_revert_and_remove() {
    let mut app = app_with_changes();
    app.checked_change_paths
        .extend([PathBuf::from("src/main.rs"), PathBuf::from("README.md")]);

    let items = app.scm_menu_items();
    assert!(items.contains(&ScmMenuItem::RemoveChecked));
    assert!(items.contains(&ScmMenuItem::DiscardChecked));
    assert!(!items.contains(&ScmMenuItem::RemoveSelected));
    assert_eq!(
        app.scm_menu_label(ScmMenuItem::RemoveChecked),
        "Remove Checked Files (2)"
    );

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        Instant::now(),
    );

    assert!(effects.is_empty());
    let Some(Modal::Confirm { title, action, .. }) = app.modal else {
        panic!("reverting the checked files must ask first");
    };
    assert_eq!(title, "Revert Checked Files?");
    let ConfirmAction::Operate(GitOperation::Discard(changes)) = action else {
        panic!("the confirmation must carry a discard operation");
    };
    assert_eq!(changes.len(), 2);
}

#[test]
fn the_changes_menu_offers_reverting_the_working_tree() {
    let mut app = app_with_changes();

    let items = app.scm_menu_items();
    assert!(items.contains(&ScmMenuItem::DiscardUnstaged));
    assert!(items.contains(&ScmMenuItem::DiscardAll));
    assert!(items.contains(&ScmMenuItem::RemoveSelected));

    let position = items
        .iter()
        .position(|item| *item == ScmMenuItem::DiscardAll)
        .expect("the menu must offer reverting every change");
    app.scm_menu_open = true;
    app.scm_menu_selected = position;
    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert!(!app.scm_menu_open);
    let Some(Modal::Confirm { title, action, .. }) = app.modal else {
        panic!("reverting every change must ask first");
    };
    assert_eq!(title, "Revert All Changes?");
    let ConfirmAction::Operate(GitOperation::Discard(changes)) = action else {
        panic!("the confirmation must carry a discard operation");
    };
    assert_eq!(changes.len(), 2);
}

#[test]
fn enter_toggles_focus_between_sidebar_and_content() {
    let mut app = app_with_changes();
    let now = Instant::now();

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert_eq!(app.focus, Focus::Content);
    assert!(app.mouse_capture);
    assert!(effects.is_empty());

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert_eq!(app.focus, Focus::Sidebar);
    assert!(app.mouse_capture);
    assert!(effects.is_empty());
}

#[test]
fn enter_moves_focus_without_folding_files_or_check_steps() {
    let now = Instant::now();
    let mut diff = App::new("/tmp/repo", "repo");
    diff.document = indexed_document(&["src/main.rs", "src/lib.rs"]);
    diff.selected_preview_file = Some(PathBuf::from("src/main.rs"));
    diff.focus = Focus::Content;

    diff.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert_eq!(diff.focus, Focus::Sidebar);
    assert!(!diff.preview_file_collapsed("src/main.rs"));

    let mut log = App::new("/tmp/repo", "repo");
    log.view = View::PullRequests;
    log.pull_request_section = PullRequestSection::Overview;
    log.pull_request_check_cursor = Some(0);
    log.pull_request_step_cursor = 4;
    let _ = log.expanded_check_steps.insert(4);
    log.focus = Focus::Content;

    log.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert_eq!(log.focus, Focus::Sidebar);
    assert!(log.check_step_expanded(4));
}

#[test]
fn preview_focus_preserves_an_explicit_no_mouse_setting() {
    let mut app = app_with_changes();
    let now = Instant::now();
    app.configure_mouse_capture(false);

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert_eq!(app.focus, Focus::Content);
    assert!(!app.mouse_capture);
    assert!(effects.is_empty());

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert_eq!(app.focus, Focus::Sidebar);
    assert!(!app.mouse_capture);
    assert!(effects.is_empty());
}

#[test]
fn expanded_preview_files_are_selectable_from_the_content_pane() {
    let mut app = App::new("/tmp/repo", "repo");
    app.document = indexed_document(&["src/main.rs", "src/lib.rs"]);
    app.selected_preview_file = Some(PathBuf::from("src/main.rs"));
    app.focus = Focus::Content;

    app.handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        Instant::now(),
    );

    assert_eq!(app.selected_preview_file, Some(PathBuf::from("src/lib.rs")));
    assert_eq!(app.preview_file_cursor, 1);
    assert_eq!(app.content_file_anchor, Some(PathBuf::from("src/lib.rs")));
}

#[test]
fn reading_a_pull_request_never_pushes_your_own_branch() {
    let mut app = app_with_changes();
    let now = Instant::now();
    app.view = View::PullRequests;

    for character in ['p', 'f'] {
        let effects = app.handle_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            now,
        );
        assert!(
            effects.is_empty(),
            "{character} queued work from inside the pull-request view"
        );
        assert!(app.busy.is_none(), "{character} started a git operation");
    }

    app.view = View::Changes;
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), now);
    assert!(app.busy.is_some(), "fetch still runs from the changes view");
}

#[test]
fn shift_o_opens_branches_commits_pull_requests_and_checks() {
    let mut app = app_with_changes();
    let now = Instant::now();
    app.local_github_repository = Some(GitHubRepository {
        name_with_owner: "o/r".to_owned(),
        url: "https://github.com/o/r".to_owned(),
        remotes: vec!["origin".to_owned()],
    });
    app.status.branch.head = "feature/right-pane".to_owned();

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/o/r/tree/feature/right-pane"
    ));

    app.view = View::History;
    app.history.push(Commit {
        id: "abc123".to_owned(),
        short_id: "abc123".to_owned(),
        parent_ids: Vec::new(),
        author: String::new(),
        author_email: String::new(),
        authored_at: String::new(),
        committer: String::new(),
        committer_email: String::new(),
        committed_at: String::new(),
        relative_date: String::new(),
        subject: "Selectable commit".to_owned(),
        decorations: Vec::new(),
    });
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/o/r/commit/abc123"
    ));

    app.view = View::PullRequests;

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(effects.is_empty(), "nothing is open, so nothing to open");

    app.pull_request = Some(PullRequest {
        url: "https://github.com/o/r/pull/8".to_owned(),
        ..PullRequest::default()
    });
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Open(OpenTarget::Browser(url))]
            if url == "https://github.com/o/r/pull/8"
    ));

    app.pull_request_checks = vec![PullRequestCheck {
        name: "build".to_owned(),
        workflow: "CI".to_owned(),
        state: "SUCCESS".to_owned(),
        status: PullRequestCheckStatus::Passed,
        description: String::new(),
        link: "https://github.com/o/r/actions/runs/1/job/2".to_owned(),
        started_at: String::new(),
        completed_at: String::new(),
    }];
    app.set_check_cursor(Some(0));
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Open(OpenTarget::Browser(url))]
                if url.contains("/actions/runs/1/job/2")
        ),
        "a selected check opens the run it names, not the pull request"
    );
}
