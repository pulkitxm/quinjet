use super::*;
use crate::git::github::PullRequestCheckStatus;
use crate::git::status::{BranchState, ChangeStatus};

fn check(name: &str, status: PullRequestCheckStatus) -> PullRequestCheck {
    PullRequestCheck {
        name: name.to_owned(),
        workflow: "CI".to_owned(),
        state: format!("{status:?}").to_uppercase(),
        status,
        description: String::new(),
        link: "https://github.com/acme/widget/actions/runs/1/job/2".to_owned(),
        started_at: "2026-08-14T18:00:00Z".to_owned(),
        completed_at: String::new(),
    }
}

fn pull_request(number: u64, title: &str, repository: &str) -> PullRequest {
    PullRequest {
        number,
        title: title.to_owned(),
        description: "A detailed pull-request description".to_owned(),
        author: "octocat".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: format!("2026-07-{number:02}T00:00:00Z"),
        updated_at: format!("2026-08-{number:02}T00:00:00Z"),
        url: format!("https://github.com/{repository}/pull/{number}"),
        base_ref: "main".to_owned(),
        base_oid: format!("base-{number}"),
        head_ref: format!("feature/{number}"),
        head_oid: format!("head-{number}"),
        base_repository: GitHubRepository {
            name_with_owner: repository.to_owned(),
            url: format!("https://github.com/{repository}"),
            remotes: vec!["upstream".to_owned()],
        },
        head_repository: Some("octocat/fork".to_owned()),
        head_remotes: vec!["origin".to_owned()],
        is_cross_repository: true,
        additions: usize::try_from(number).unwrap_or(usize::MAX),
        deletions: 1,
        changed_files: 2,
    }
}

#[test]
fn pull_request_cta_actions_follow_state_and_remembered_merge_method() {
    let mut app = App::new("/tmp/repo", "repo");
    assert_eq!(app.pr_primary_action(), None);
    assert_eq!(app.pr_menu_items(), Vec::new());

    app.pull_request = Some(pull_request(12, "Ship it", "acme/widget"));
    assert_eq!(
        app.pr_primary_action(),
        Some(PrPrimaryAction::Merge(PullRequestMergeMethod::Squash))
    );
    assert_eq!(
        app.pr_menu_items(),
        vec![
            PrMenuItem::Merge(PullRequestMergeMethod::Merge),
            PrMenuItem::Merge(PullRequestMergeMethod::Rebase),
            PrMenuItem::Close,
            PrMenuItem::OpenInBrowser,
        ]
    );

    app.preferred_merge_method = PullRequestMergeMethod::Rebase;
    assert_eq!(
        app.pr_primary_action(),
        Some(PrPrimaryAction::Merge(PullRequestMergeMethod::Rebase))
    );
    assert_eq!(
        app.pr_menu_items(),
        vec![
            PrMenuItem::Merge(PullRequestMergeMethod::Merge),
            PrMenuItem::Merge(PullRequestMergeMethod::Squash),
            PrMenuItem::Close,
            PrMenuItem::OpenInBrowser,
        ]
    );

    if let Some(pull_request) = app.pull_request.as_mut() {
        pull_request.state = "CLOSED".to_owned();
    }
    assert_eq!(app.pr_primary_action(), Some(PrPrimaryAction::Reopen));
    assert_eq!(app.pr_menu_items(), vec![PrMenuItem::OpenInBrowser]);

    if let Some(pull_request) = app.pull_request.as_mut() {
        pull_request.state = "MERGED".to_owned();
    }
    assert_eq!(
        app.pr_primary_action(),
        Some(PrPrimaryAction::OpenInBrowser)
    );
    assert_eq!(app.pr_menu_items(), Vec::new());
}

fn app_with_changes() -> App {
    let mut app = App::new("/tmp/repo", "repo");
    app.status = RepoStatus {
        branch: BranchState::default(),
        changes: vec![
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
        ],
    };
    app.selected_change_section = None;
    app
}

fn indexed_document(paths: &[&str]) -> DiffDocument {
    DiffIndex {
        title: "Diff".to_owned(),
        files: paths
            .iter()
            .map(|path| crate::git::diff::DiffFileIndexEntry {
                path: PathBuf::from(path),
                old_path: None,
                status: "modified".to_owned(),
                counts: None,
            })
            .collect(),
        truncated: false,
        commit_details: None,
    }
    .document(&HashMap::new())
}

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
    );
    assert_eq!(app.checked_change_count(), 1);
    assert_eq!(app.section_check_label(ChangeSection::Unstaged), "[x]");
    assert_eq!(app.section_check_label(ChangeSection::Staged), "[ ]");

    app.handle_scm_action(
        ScmAction::ToggleCheckSection(ChangeSection::Staged),
        &mut effects,
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
    );
    assert_eq!(app.section_check_label(ChangeSection::Staged), "[x]");
    assert_eq!(app.checked_change_count(), 3);

    app.handle_scm_action(
        ScmAction::ToggleCheckSection(ChangeSection::Staged),
        &mut effects,
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
    );

    app.handle_scm_action(ScmAction::RevertChecked, &mut effects);

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
    assert!(app.content_scroll > 0);
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

#[test]
fn single_file_views_cannot_be_collapsed() {
    let mut app = App::new("/tmp/repo", "repo");
    app.document = indexed_document(&["src/main.rs"]);
    app.selected_preview_file = Some(PathBuf::from("src/main.rs"));
    let now = Instant::now();

    assert!(!app.preview_files_collapsible());
    assert!(!app.preview_file_collapsed("src/main.rs"));
    assert!(!app.preview_files_all_collapsed());

    app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now);
    app.toggle_preview_file(PathBuf::from("src/main.rs"), &mut Vec::new());

    assert!(!app.files_collapsed);
    assert!(app.collapsed_preview_files.is_empty());
    assert!(app.expanded_preview_files.is_empty());
    assert!(!app.preview_file_collapsed("src/main.rs"));
}

#[test]
fn preserves_selected_change_across_status_refresh_order() {
    let mut app = app_with_changes();
    app.change_cursor = 1;
    let selected = app.selected_change().cloned();
    app.status.changes.swap(0, 1);
    app.restore_change_selection(selected.as_ref());
    assert_eq!(
        app.selected_change().unwrap().path,
        PathBuf::from("README.md")
    );
}

#[test]
fn startup_does_not_fetch_any_pull_request_data() {
    let mut app = App::new("/tmp/repo", "repo");

    let effects = app.initial_effects();

    assert!(matches!(
        effects.as_slice(),
        [
            AppEffect::Git(refresh),
            AppEffect::Git(worktrees),
            AppEffect::Git(history),
            AppEffect::Git(branches),
            AppEffect::Git(repository),
        ] if matches!(refresh.as_ref(), WorkerCommand::Refresh { .. })
            && matches!(worktrees.as_ref(), WorkerCommand::LoadWorktrees { .. })
            && matches!(history.as_ref(), WorkerCommand::LoadHistory { .. })
            && matches!(branches.as_ref(), WorkerCommand::LoadHistoryBranches { .. })
            && matches!(repository.as_ref(), WorkerCommand::LoadLocalGitHubRepository)
    ));
    assert!(!app.pull_request_loading);
    assert!(app.pull_request.is_none());
    assert_eq!(app.pull_request_generation, 0);
}

#[test]
fn opening_pr_tab_only_focuses_the_number_field() {
    let mut app = App::new("/tmp/repo", "repo");

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert_eq!(app.view, View::PullRequests);
    assert!(app.pull_request_lookup_active);
    assert_eq!(app.document.title, "Open Pull Request");
}

#[test]
fn recent_pull_requests_open_with_space_while_enter_moves_focus() {
    let mut app = App::new("/tmp/repo", "repo");
    app.recent_pull_requests = vec![
        RecentPullRequest::from(&pull_request(39, "First", "acme/widget")),
        RecentPullRequest::from(&pull_request(42, "Second", "acme/widget")),
    ];
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE), now);
    assert!(!app.pull_request_lookup_active);
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert!(effects.is_empty());
    assert_eq!(app.focus, Focus::Content);
    assert!(app.pull_request.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);

    assert_eq!(app.recent_pull_request_cursor, 1);
    assert_eq!(app.pull_request_lookup.value, "42");
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest {
                number: 42,
                repository: Some(repository),
                ..
            } if repository.name_with_owner == "acme/widget"
        )
    ));
}

#[test]
fn repository_picker_is_also_discovered_only_on_explicit_request() {
    let mut app = App::new("/tmp/repo", "repo");
    app.switch_view(View::PullRequests, &mut Vec::new());

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadGitHubRepositories {
                generation: 1,
                refresh: false,
            }
        )
    ));
    assert!(matches!(
        app.modal,
        Some(Modal::PullRequestRepositories { loading: true, .. })
    ));
}

#[test]
fn numeric_lookup_discovers_the_repository_on_demand() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.switch_view(View::PullRequests, &mut Vec::new());
    for character in ['4', '2'] {
        app.handle_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            now,
        );
    }

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest {
                number: 42,
                repository: None,
                ..
            }
        )
    ));
    assert_eq!(
        app.pull_request_progress,
        Some(PullRequestProgress::LoadingMetadata)
    );
    assert!(app.pull_request_loading);
}

#[test]
fn opening_a_pull_request_prefetches_diffs_checks_and_conversation_from_overview() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_generation = 3;
    app.pull_request_loading = true;
    let request = pull_request(8, "Cross-fork update", "acme/widget");
    let repository = request.base_repository.clone();

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: 3,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: vec![repository.clone()],
                selected_repository: Some(repository),
                pull_request: request,
                warnings: Vec::new(),
                exact_number: Some(8),
                from_cache: false,
            }),
        },
        Instant::now(),
    );

    assert_eq!(effects.len(), 3);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestConversation { pull_request, .. }
                if pull_request.number == 8
        )
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::PreparePullRequest { pull_request, .. }
                if pull_request.number == 8
        )
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestChecks { pull_request, .. }
                if pull_request.number == 8
        )
    )));
    assert_eq!(
        app.pull_request_progress,
        Some(PullRequestProgress::PreparingRepository)
    );
    assert_eq!(
        app.pull_request_section,
        PullRequestSection::Overview,
        "an opened pull request lands on itself, not on its files"
    );
    let workspace_generation = app.diff_generation;
    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestIndex {
            generation: workspace_generation,
            result: Ok(PullRequestDiffIndex {
                files: ["src/first.rs", "src/second.rs"]
                    .into_iter()
                    .map(|path| PullRequestFile {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: PullRequestFileStatus::Modified,
                        counts: None,
                    })
                    .collect(),
                total_files: 2,
                truncated: false,
            }),
        },
        Instant::now(),
    );
    assert_eq!(app.pull_request_section, PullRequestSection::Overview);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation: generation,
                paths,
            } if *generation == workspace_generation
                && paths == &[PathBuf::from("src/first.rs"), PathBuf::from("src/second.rs")]
        )
    ));
    assert_eq!(
        app.pull_request.as_ref().unwrap().description,
        "A detailed pull-request description"
    );
}

#[test]
fn local_diff_index_prefetches_one_file_then_loads_only_an_expanded_path() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 5;
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 5,
            result: Ok(DiffIndex {
                title: "Branch comparison".to_owned(),
                files: ["src/first.rs", "src/second.rs"]
                    .into_iter()
                    .map(|path| crate::git::diff::DiffFileIndexEntry {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: "modified".to_owned(),
                        counts: None,
                    })
                    .collect(),
                truncated: false,
                commit_details: None,
            }),
        },
        Instant::now(),
    );

    assert_eq!(app.document.file_count(), 2);
    assert!(app.preview_files_all_collapsed());
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile {
                workspace_generation: 5,
                path,
                ..
            } if path == Path::new("src/first.rs")
        )
    ));

    let first_generation = app.diff_generation;
    app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: first_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/first.rs"),
            result: Ok(DiffDocument::empty("first", "loaded")),
        },
        Instant::now(),
    );
    let mut effects = Vec::new();
    app.toggle_preview_file(PathBuf::from("src/second.rs"), &mut effects);

    assert_eq!(app.document.file_count(), 2);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile {
                workspace_generation: 5,
                path,
                ..
            } if path == Path::new("src/second.rs")
        )
    ));
}

#[test]
fn expanding_all_local_files_loads_every_visible_diff_in_order() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 5;
    let now = Instant::now();
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 5,
            result: Ok(DiffIndex {
                title: "Commit details".to_owned(),
                files: ["src/first.rs", "src/second.rs", "src/third.rs"]
                    .into_iter()
                    .map(|path| crate::git::diff::DiffFileIndexEntry {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: "modified".to_owned(),
                        counts: None,
                    })
                    .collect(),
                truncated: false,
                commit_details: None,
            }),
        },
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == Path::new("src/first.rs")
        )
    ));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: app.diff_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/first.rs"),
            result: Ok(indexed_document(&["src/first.rs"])),
        },
        now,
    );
    assert!(effects.is_empty());

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == Path::new("src/second.rs")
        )
    ));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: app.diff_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/second.rs"),
            result: Ok(indexed_document(&["src/second.rs"])),
        },
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. }
                if path == Path::new("src/third.rs")
        )
    ));

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: app.diff_generation,
            workspace_generation: 5,
            path: PathBuf::from("src/third.rs"),
            result: Ok(indexed_document(&["src/third.rs"])),
        },
        now,
    );
    assert!(effects.is_empty());
    assert_eq!(app.local_diff_documents.len(), 3);
    assert!(app.local_diff_pending_paths.is_empty());
}

#[test]
fn an_existing_expand_all_preference_loads_every_file_in_a_new_index() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 9;
    app.files_collapsed = false;
    app.collapse_preference_set = true;

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 9,
            result: Ok(DiffIndex {
                title: "Next commit".to_owned(),
                files: ["one.rs", "two.rs", "three.rs"]
                    .into_iter()
                    .map(|path| crate::git::diff::DiffFileIndexEntry {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: "modified".to_owned(),
                        counts: None,
                    })
                    .collect(),
                truncated: false,
                commit_details: None,
            }),
        },
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadLocalDiffFile { path, .. } if path == Path::new("one.rs")
        )
    ));
    assert_eq!(
        app.local_diff_pending_paths,
        VecDeque::from([PathBuf::from("two.rs"), PathBuf::from("three.rs")])
    );
    assert!(
        app.preview_file_paths()
            .iter()
            .all(|path| !app.preview_file_collapsed(&path.to_string_lossy()))
    );
}

#[test]
fn local_diff_file_replies_must_match_the_prepared_workspace() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 12;
    app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 12,
            result: Ok(DiffIndex {
                title: "Current".to_owned(),
                files: vec![crate::git::diff::DiffFileIndexEntry {
                    path: PathBuf::from("same.rs"),
                    old_path: None,
                    status: "modified".to_owned(),
                    counts: None,
                }],
                truncated: false,
                commit_details: None,
            }),
        },
        Instant::now(),
    );

    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: 12,
            workspace_generation: 11,
            path: PathBuf::from("same.rs"),
            result: Ok(DiffDocument::empty("Old commit", "stale")),
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert_eq!(app.local_diff_loading_path, Some(PathBuf::from("same.rs")));
    assert!(!app.local_diff_single_loaded);
    assert_ne!(app.document.title, "Old commit");
}

#[test]
fn local_diff_failure_replaces_the_loading_placeholder() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 12;
    app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: 12,
            result: Ok(DiffIndex {
                title: "Stash".to_owned(),
                files: vec![crate::git::diff::DiffFileIndexEntry {
                    path: PathBuf::from("index2.ts"),
                    old_path: None,
                    status: "added".to_owned(),
                    counts: None,
                }],
                truncated: false,
                commit_details: None,
            }),
        },
        Instant::now(),
    );

    app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: 12,
            workspace_generation: 12,
            path: PathBuf::from("index2.ts"),
            result: Err("missing stash parent".to_owned()),
        },
        Instant::now(),
    );

    assert!(app.local_diff_single_loaded);
    assert!(
        app.document
            .lines
            .iter()
            .any(|line| line.text() == "Unable to load diff: missing stash parent")
    );
}

#[test]
fn pull_request_folders_are_selectable_and_collapsible() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_section = PullRequestSection::Files;
    app.focus = Focus::Sidebar;
    app.pull_request_files = ["src/app.rs", "src/git/diff.rs", "tests/ui.rs"]
        .into_iter()
        .map(|path| PullRequestFile {
            path: PathBuf::from(path),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    app.sync_pull_request_tree_cursor_to_file();

    let cursor = app.pull_request_tree_cursor;
    let entries = app.pull_request_tree_entries();
    assert!(matches!(
        entries.get(cursor),
        Some(PullRequestTreeEntry::File { index: 0, .. })
    ));

    app.handle_key(
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        Instant::now(),
    );
    let cursor = app.pull_request_tree_cursor;
    assert!(matches!(
        app.pull_request_tree_entries().get(cursor),
        Some(PullRequestTreeEntry::Directory { path, .. }) if path == Path::new("src")
    ));

    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    assert_eq!(app.focus, Focus::Content);
    assert!(!app.pull_request_directory_collapsed(Path::new("src")));

    app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(app.pull_request_directory_collapsed(Path::new("src")));
    assert_eq!(
        app.pull_request_tree_entries()
            .iter()
            .filter(|entry| matches!(entry, PullRequestTreeEntry::File { .. }))
            .count(),
        1
    );

    app.handle_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(app.pull_request_directory_collapsed(Path::new("src")));

    app.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(!app.pull_request_directory_collapsed(Path::new("src")));
    assert_eq!(
        app.pull_request_tree_entries()
            .iter()
            .filter(|entry| matches!(entry, PullRequestTreeEntry::File { .. }))
            .count(),
        3
    );

    app.geometry.sidebar = Rect::new(0, 0, 40, 10);
    app.geometry.sidebar_hits = vec![SidebarHitArea {
        area: Rect::new(0, 2, 40, 1),
        target: SidebarHit::PullRequestDirectory(PathBuf::from("src/git")),
    }];
    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::SHIFT,
        },
        Instant::now(),
    );
    assert!(!app.pull_request_directory_collapsed(Path::new("src/git")));

    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert!(app.pull_request_directory_collapsed(Path::new("src/git")));
    assert!(
        !app.pull_request_tree_entries()
            .iter()
            .any(|entry| { matches!(entry, PullRequestTreeEntry::File { index: 1, .. }) })
    );
}

#[test]
fn pull_request_tree_compacts_single_child_directory_chains() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_files = [
        "apps/web/app/one.txt",
        "apps/web/modules/marketing/landing/two.txt",
    ]
    .into_iter()
    .map(|path| PullRequestFile {
        path: PathBuf::from(path),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: None,
    })
    .collect();

    let entries = app.pull_request_tree_entries();

    assert!(matches!(
        entries.first(),
        Some(PullRequestTreeEntry::Directory { path, label, depth: 0 })
            if path == Path::new("apps/web") && label == "apps/web"
    ));
    assert!(entries.iter().any(|entry| matches!(
        entry,
        PullRequestTreeEntry::Directory { path, label, depth: 1 }
            if path == Path::new("apps/web/modules/marketing/landing")
                && label == "modules/marketing/landing"
    )));

    app.toggle_pull_request_directory(PathBuf::from("apps/web"));
    assert_eq!(app.pull_request_tree_entries().len(), 1);
}

#[test]
fn pull_request_defaults_to_all_files_then_files_tab_restores_it_from_single_file() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_section = PullRequestSection::Files;
    app.focus = Focus::Sidebar;
    app.pull_request = Some(pull_request(8, "Large change", "acme/widget"));
    app.diff_generation = 10;
    let files = ["src/first.rs", "src/second.rs"]
        .into_iter()
        .map(|path| PullRequestFile {
            path: PathBuf::from(path),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    let now = Instant::now();

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestIndex {
            generation: 10,
            result: Ok(PullRequestDiffIndex {
                files,
                total_files: 2,
                truncated: false,
            }),
        },
        now,
    );

    assert_eq!(app.pull_request_file_view, PullRequestFileView::AllFiles);
    assert_eq!(app.document.file_count(), 2);
    assert!(app.preview_files_all_collapsed());
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch {
                    workspace_generation: 10,
                    paths,
                } if paths == &[PathBuf::from("src/first.rs"), PathBuf::from("src/second.rs")]
            )
        ),
        "the whole index is fetched in one batch rather than a file at a time"
    );

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Ok(vec![
                (
                    PathBuf::from("src/first.rs"),
                    indexed_document(&["src/first.rs"]),
                ),
                (
                    PathBuf::from("src/second.rs"),
                    indexed_document(&["src/second.rs"]),
                ),
            ]),
        },
        now,
    );
    assert!(effects.is_empty(), "no file is left to fetch");
    assert_eq!(app.document.file_count(), 2);

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_file_cursor, 1);
    assert_eq!(app.pull_request_file_view, PullRequestFileView::SingleFile);
    let (effects, _) = app.tick(now + PREVIEW_DEBOUNCE);
    assert!(
        effects.is_empty(),
        "a prefetched file opens without another Git round trip"
    );
    assert_eq!(app.document.file_count(), 1);
    assert!(!app.preview_files_collapsible());

    app.geometry.sidebar = Rect::new(0, 0, 20, 10);
    app.geometry.sidebar_hits = vec![SidebarHitArea {
        area: Rect::new(1, 1, 8, 1),
        target: SidebarHit::PullRequestFiles,
    }];
    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
        now,
    );

    assert!(effects.is_empty(), "the prefetched first file is cached");
    assert_eq!(app.pull_request_file_view, PullRequestFileView::AllFiles);
    assert_eq!(app.document.file_count(), 2);
    assert!(app.preview_files_all_collapsed());
}

#[test]
fn sidebar_wheel_scroll_pans_without_moving_the_selection() {
    let mut app = App::new("/tmp/repo", "repo");
    app.geometry.sidebar = Rect::new(0, 0, 30, 10);
    app.sidebar_viewport(0, 5, 40);
    let cursor_before = app.history_cursor;

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );
    assert!(effects.is_empty(), "panning requests no preview");
    assert_eq!(app.history_cursor, cursor_before, "the selection stays put");
    assert_eq!(app.sidebar_offset, 2, "the viewport pans");
    assert!(app.sidebar_free_scroll, "the window detaches");

    app.sidebar_viewport(0, 5, 40);
    assert_eq!(
        app.sidebar_offset, 2,
        "an unmoved selection does not snap the window back"
    );

    app.sidebar_viewport(1, 5, 40);
    assert!(!app.sidebar_free_scroll, "a selection change reattaches");
    assert_eq!(app.sidebar_offset, 1, "the window follows the selection");
}

#[test]
fn pull_request_prefetch_batches_by_estimated_patch_size() {
    let counts = |additions: usize| {
        Some(DiffLineCounts {
            additions,
            deletions: 0,
            binary: false,
        })
    };
    let file = |path: &str, additions: usize| PullRequestFile {
        path: PathBuf::from(path),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: counts(additions),
    };
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_workspace_generation = Some(10);
    app.pull_request_files = vec![
        file("src/huge.rs", 200_000),
        file("src/small.rs", 10),
        file("src/tiny.rs", 5),
    ];

    let mut effects = Vec::new();
    app.request_pull_request_prefetch(&mut effects);
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths }
                    if paths == &[PathBuf::from("src/huge.rs")]
            )
        ),
        "a file estimated past the byte budget travels alone"
    );

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Ok(vec![(
                PathBuf::from("src/huge.rs"),
                indexed_document(&["src/huge.rs"]),
            )]),
        },
        Instant::now(),
    );
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths }
                    if paths == &[PathBuf::from("src/small.rs"), PathBuf::from("src/tiny.rs")]
            )
        ),
        "small files share the next batch"
    );
}

#[test]
fn prefetch_starts_at_the_files_viewport_and_wraps_around() {
    let counts = |additions: usize| {
        Some(DiffLineCounts {
            additions,
            deletions: 0,
            binary: false,
        })
    };
    let file = |path: &str| PullRequestFile {
        path: PathBuf::from(path),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: counts(5),
    };
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_workspace_generation = Some(10);
    app.view = View::PullRequests;
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_files = vec![file("a.rs"), file("b.rs"), file("c.rs"), file("d.rs")];
    let _ = app.pull_request_tree_entries();
    app.sidebar_offset = 2;

    let mut effects = Vec::new();
    app.request_pull_request_prefetch(&mut effects);
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths }
                    if paths == &[
                        PathBuf::from("c.rs"),
                        PathBuf::from("d.rs"),
                        PathBuf::from("a.rs"),
                        PathBuf::from("b.rs"),
                    ]
            )
        ),
        "fill starts at the visible file and wraps around the index"
    );
}

#[test]
fn pull_request_prefetch_retries_once_after_a_failure() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_workspace_generation = Some(10);
    app.pull_request_files = vec![PullRequestFile {
        path: PathBuf::from("src/first.rs"),
        old_path: None,
        status: PullRequestFileStatus::Modified,
        counts: None,
    }];
    app.pull_request_prefetching = true;

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Err("transient failure".to_owned()),
        },
        Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation: 10,
                paths,
            } if paths == &[PathBuf::from("src/first.rs")]
        )
    ));
    assert!(app.pull_request_prefetch_retrying);

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Err("persistent failure".to_owned()),
        },
        Instant::now(),
    );
    assert!(effects.is_empty());
    assert!(!app.pull_request_prefetching);
    assert!(!app.pull_request_prefetch_retrying);
}

#[test]
fn a_live_poll_refreshes_a_pull_request_without_disturbing_the_reader() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
    app.pull_request_exact_number = Some(8);
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    app.pull_request_check_cursor = Some(0);
    app.content_scroll = 40;

    let mut effects = Vec::new();
    app.refresh_pull_request_live(now, false, &mut effects);

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command) if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest { number: 8, refresh: true, .. }
        )
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { .. })
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestConversation { .. })
    )));
    assert!(
        !effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::LoadCheckRunLog { .. })
        )),
        "a finished run's log never changes, so a poll does not re-read it"
    );
    assert!(
        app.pull_request.is_some(),
        "the loaded pull request stays on screen while the poll runs"
    );
    assert_eq!(app.pull_request_section, PullRequestSection::Files);
    assert_eq!(app.content_scroll, 40);
    assert!(app.pull_request_progress.is_none());
}

#[test]
fn conversation_refresh_replays_when_the_previous_read_finishes() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request = Some(pull_request(8, "Conversation", "acme/widget"));
    let mut first = Vec::new();
    app.request_pull_request_conversation(true, &mut first);
    let generation = app.pull_request_conversation_generation;

    let mut coalesced = Vec::new();
    app.request_pull_request_conversation(true, &mut coalesced);
    assert!(coalesced.is_empty());
    assert!(app.pull_request_conversation_refresh_again);

    let replayed = app.handle_worker_event(
        WorkerEvent::PullRequestConversation {
            generation,
            result: Ok(PullRequestConversation::default()),
        },
        Instant::now(),
    );
    assert!(matches!(
        replayed.as_slice(),
        [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestConversation { .. })
    ));
    assert!(app.pull_request_conversation_loading);
}

#[test]
fn parsed_pull_request_documents_evict_oldest_entries_by_size() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut document = indexed_document(&["src/file.rs"]);
    document.title = "x".repeat(1_024);
    let document_size = diff_document_size(&document);

    for name in ["first.rs", "second.rs", "third.rs"] {
        app.cache_pull_request_document(PathBuf::from(name), document.clone());
    }
    app.prune_pull_request_documents(document_size.saturating_mul(2));

    assert_eq!(app.pull_request_documents.len(), 2);
    assert!(
        !app.pull_request_documents
            .contains_key(Path::new("first.rs"))
    );
    assert!(
        app.pull_request_documents
            .contains_key(Path::new("second.rs"))
    );
    assert!(
        app.pull_request_documents
            .contains_key(Path::new("third.rs"))
    );
}

#[test]
fn a_fast_tick_only_speeds_up_the_reads_that_change_that_fast() {
    let mut app = App::new("/tmp/repo", "repo");
    let start = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
    app.pull_request_exact_number = Some(8);
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
    app.pull_request_check_cursor = Some(0);

    let mut first = Vec::new();
    app.refresh_pull_request_live(start, false, &mut first);
    let command_count = |effects: &[AppEffect], name: &str| {
        effects
            .iter()
            .filter(|effect| match effect {
                AppEffect::Git(command) => match name {
                    "checks" => {
                        matches!(
                            command.as_ref(),
                            WorkerCommand::LoadPullRequestChecks { .. }
                        )
                    }
                    "conversation" => matches!(
                        command.as_ref(),
                        WorkerCommand::LoadPullRequestConversation { .. }
                    ),
                    _ => matches!(command.as_ref(), WorkerCommand::LoadCheckRunLog { .. }),
                },
                AppEffect::Copy(_)
                | AppEffect::SetMouseCapture(_)
                | AppEffect::Open(_)
                | AppEffect::OpenRepository(_)
                | AppEffect::Quit => false,
            })
            .count()
    };
    assert_eq!(command_count(&first, "checks"), 1);
    assert_eq!(command_count(&first, "conversation"), 1);
    assert_eq!(command_count(&first, "log"), 1);

    app.pull_request_checks_loading = false;
    app.pull_request_conversation_loading = false;
    app.pull_request_check_log_loading = false;
    app.pull_request_loading = false;
    let mut second = Vec::new();
    app.refresh_pull_request_live(start + PULL_REQUEST_ACTIVE_POLL, false, &mut second);

    assert_eq!(command_count(&second, "checks"), 1);
    assert_eq!(
        command_count(&second, "conversation"),
        0,
        "the conversation holds its own floor rather than following the tick"
    );
    assert_eq!(command_count(&second, "log"), 0);

    let forced = app.webhook_delivered(start + PULL_REQUEST_ACTIVE_POLL);
    assert_eq!(command_count(&forced, "conversation"), 1);
    assert_eq!(command_count(&forced, "log"), 1);
}

#[test]
fn steps_are_selected_with_the_same_keys_as_every_other_list() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.focus = Focus::Content;
    app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    app.pull_request_check_cursor = Some(0);
    let step = |number: usize| crate::git::github::CheckStep {
        number,
        name: format!("step {number}"),
        status: PullRequestCheckStatus::Passed,
        conclusion: "success".to_owned(),
        started_at: String::new(),
        completed_at: String::new(),
        lines: Vec::new(),
    };
    app.pull_request_check_log = Some(CheckRunLog {
        steps: vec![step(1), step(2), step(3)],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    app.pull_request_step_cursor = 1;

    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 2);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 3);
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 3, "the last step is the end");
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 2);
    assert_eq!(
        app.content_scroll, 0,
        "selecting a step never scrolls the pane behind the reader's back"
    );

    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);
    assert!(app.check_step_expanded(2));

    app.geometry.content = Rect::new(0, 0, 80, 20);
    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 2);
    assert!(app.content_scroll > 0);

    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 3);
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_step_cursor, 1);
}

#[test]
fn walking_the_check_list_with_keys_drops_the_log_it_walked_away_from() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.focus = Focus::Sidebar;
    app.pull_request = Some(pull_request(8, "Two checks", "acme/widget"));
    app.pull_request_checks = vec![
        check("Build every workspace", PullRequestCheckStatus::Passed),
        check("No-comment policy", PullRequestCheckStatus::Passed),
    ];
    app.pull_request_check_cursor = Some(0);
    app.pull_request_check_log_target = Some(app.pull_request_checks[0].identity());
    app.pull_request_check_log = Some(CheckRunLog {
        steps: vec![crate::git::github::CheckStep {
            number: 1,
            name: "Build every workspace".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
            lines: Vec::new(),
        }],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    let stale = app.pull_request_check_log_generation;

    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);

    assert_eq!(app.pull_request_check_cursor, Some(1));
    assert!(
        app.pull_request_check_log.is_none(),
        "the header moved to another run, so its body cannot still be the old one"
    );
    assert_ne!(
        app.pull_request_check_log_generation, stale,
        "a reply already in flight for the previous run is invalidated"
    );
}

#[test]
fn check_status_sections_group_fold_and_navigate() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.focus = Focus::Sidebar;
    app.pull_request = Some(pull_request(8, "Grouped checks", "acme/widget"));
    app.pull_request_checks = vec![
        check("pending", PullRequestCheckStatus::Pending),
        check("broken", PullRequestCheckStatus::Failed),
        check("green", PullRequestCheckStatus::Passed),
        check("skipped", PullRequestCheckStatus::Skipped),
    ];

    let rows = app.check_list_rows();
    assert!(matches!(rows.first(), Some(CheckListRow::Conversation)));
    assert!(rows.iter().any(|row| matches!(row, CheckListRow::Heading)));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::Failed,
            count: 1,
            collapsed: false,
        }
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::InProgress,
            count: 1,
            collapsed: false,
        }
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::Successful,
            count: 1,
            collapsed: false,
        }
    )));
    assert!(rows.iter().any(|row| matches!(
        row,
        CheckListRow::Section {
            section: CheckStatusSection::Skipped,
            count: 1,
            collapsed: false,
        }
    )));

    app.navigate(1, now);
    assert_eq!(app.selected_check_section, Some(CheckStatusSection::Failed));
    assert!(app.pull_request_check_cursor.is_none());

    assert!(app.toggle_selected_check_section());
    assert!(
        app.collapsed_check_sections
            .contains(&CheckStatusSection::Failed)
    );
    assert!(
        !app.check_list_rows()
            .iter()
            .any(|row| matches!(row, CheckListRow::Check { index: 1 }))
    );

    app.navigate(1, now);
    assert_eq!(
        app.selected_check_section,
        Some(CheckStatusSection::InProgress)
    );
    app.navigate(1, now);
    assert_eq!(app.pull_request_check_cursor, Some(0));
    assert!(app.selected_check_section.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), now);
    assert_eq!(
        app.selected_check_section,
        Some(CheckStatusSection::InProgress)
    );
    assert!(app.pull_request_check_cursor.is_none());

    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), now);
    assert_eq!(app.pull_request_check_cursor, Some(0));

    app.go_to_edge(true, now);
    assert_eq!(app.pull_request_check_cursor, Some(3));
    app.go_to_edge(false, now);
    assert!(app.pull_request_check_cursor.is_none());
    assert!(app.selected_check_section.is_none());
}

#[test]
fn opening_the_repository_picker_does_not_cancel_a_pull_request_lookup() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;

    let mut effects = Vec::new();
    app.request_pull_request_lookup(42, false, false, &mut effects);
    let lookup = app.pull_request_generation;
    assert!(app.pull_request_loading);

    app.open_pull_request_repositories(&mut Vec::new());
    assert_eq!(
        app.pull_request_generation, lookup,
        "repository discovery answers on its own counter"
    );

    app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: lookup,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: Vec::new(),
                selected_repository: None,
                pull_request: pull_request(42, "Still wanted", "acme/widget"),
                warnings: Vec::new(),
                exact_number: Some(42),
                from_cache: false,
            }),
        },
        now,
    );

    assert!(
        !app.pull_request_loading,
        "the lookup reply is still accepted, so polling is never blocked"
    );
    assert_eq!(
        app.pull_request.as_ref().map(|request| request.number),
        Some(42)
    );
}

#[test]
fn moving_to_another_check_never_shows_the_previous_run_under_its_name() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Two checks", "acme/widget"));
    app.pull_request_checks = vec![
        check("Build every workspace", PullRequestCheckStatus::Passed),
        check("No-comment policy", PullRequestCheckStatus::Passed),
    ];

    let mut effects = Vec::new();
    app.select_pull_request_check(Some(0), &mut effects);
    assert_eq!(effects.len(), 1);
    let slow = app.pull_request_check_log_generation;
    assert!(app.pull_request_check_log_loading);

    let mut effects = Vec::new();
    app.select_pull_request_check(Some(1), &mut effects);
    assert!(
        matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadCheckRunLog { check, .. }
                    if check.name == "No-comment policy"
            )
        ),
        "the newly selected run is requested rather than waited for"
    );
    assert_ne!(
        app.pull_request_check_log_generation, slow,
        "the in-flight read is invalidated by the move"
    );

    let effects = app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: slow,
            result: Ok(CheckRunLog {
                steps: vec![crate::git::github::CheckStep {
                    number: 1,
                    name: "Build every workspace".to_owned(),
                    status: PullRequestCheckStatus::Passed,
                    conclusion: "success".to_owned(),
                    started_at: String::new(),
                    completed_at: String::new(),
                    lines: Vec::new(),
                }],
                loose_lines: Vec::new(),
                truncated: false,
                unavailable: None,
                log_pending: false,
            }),
        },
        now,
    );

    assert!(effects.is_empty());
    assert!(
        app.pull_request_check_log.is_none(),
        "a reply for the previous check is discarded, not shown under the new one"
    );
    assert!(
        app.pull_request_check_log_loading,
        "the new read is still pending"
    );
}

#[test]
fn a_running_log_follows_its_own_tail_unless_the_reader_scrolled_up() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
    app.pull_request_check_cursor = Some(0);
    app.pull_request_check_log_generation = 3;
    let log = || CheckRunLog {
        steps: Vec::new(),
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    };

    app.content_at_bottom = true;
    app.content_scroll = 120;
    app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: 3,
            result: Ok(log()),
        },
        now,
    );
    assert_eq!(
        app.content_scroll,
        usize::MAX,
        "the draw clamps this to the new end"
    );

    app.content_at_bottom = false;
    app.content_scroll = 40;
    app.pull_request_check_log_generation = 4;
    app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: 4,
            result: Ok(log()),
        },
        now,
    );
    assert_eq!(app.content_scroll, 40);

    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    app.content_at_bottom = true;
    app.content_scroll = 40;
    app.pull_request_check_log_generation = 5;
    app.handle_worker_event(
        WorkerEvent::CheckRunLog {
            generation: 5,
            result: Ok(log()),
        },
        now,
    );
    assert_eq!(app.content_scroll, 40);
}

#[test]
fn a_settled_pull_request_polls_less_often_than_a_running_one() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
    assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_IDLE_POLL);

    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
    assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_ACTIVE_POLL);

    app.view = View::Changes;
    assert_eq!(
        app.pull_request_poll_interval(),
        PULL_REQUEST_BACKGROUND_POLL,
        "a pull request nobody is looking at still stays fresh, just cheaply"
    );
}

#[test]
fn a_moved_head_reindexes_the_diff_and_keeps_everything_else() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request_generation = 4;
    app.pull_request_loading = true;
    app.pull_request = Some(pull_request(8, "Force pushed", "acme/widget"));
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_workspace_generation = Some(2);
    app.pull_request_documents
        .insert(PathBuf::from("src/one.rs"), DiffDocument::default());
    app.pull_request_check_cursor = Some(0);
    app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];

    let mut moved = pull_request(8, "Force pushed", "acme/widget");
    moved.head_oid = "rewritten".to_owned();
    let repository = moved.base_repository.clone();
    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: 4,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: vec![repository.clone()],
                selected_repository: Some(repository),
                pull_request: moved,
                warnings: Vec::new(),
                exact_number: Some(8),
                from_cache: false,
            }),
        },
        now,
    );

    assert!(app.pull_request_workspace_generation.is_none());
    assert!(app.pull_request_documents.is_empty());
    assert_eq!(
        app.pull_request_section,
        PullRequestSection::Files,
        "a force push replaces the diff, not the reader's place in the view"
    );
    assert_eq!(app.pull_request_check_cursor, Some(0));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PreparePullRequest { .. })
    )));
}

#[test]
fn a_forwarded_webhook_refreshes_immediately_instead_of_waiting_for_the_poll() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
    app.pull_request_exact_number = Some(8);
    app.pull_request_poll_due = Some(now + Duration::from_secs(3_600));

    let effects = app.webhook_delivered(now);

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { .. })
    )));
    assert!(
        app.pull_request_poll_due
            .is_some_and(|due| due <= now + PULL_REQUEST_IDLE_POLL),
        "the delivery also restarts the poll clock"
    );
}

#[test]
fn history_reset_requested_during_a_load_runs_after_the_in_flight_page() {
    let mut app = App::new("/tmp/repo", "repo");
    app.history_generation = 4;
    app.history_loading = true;
    let mut effects = Vec::new();

    app.request_history(true, &mut effects);
    assert!(effects.is_empty());
    assert!(app.history_refresh_again);

    let effects = app.handle_worker_event(
        WorkerEvent::History {
            generation: 4,
            skip: 0,
            result: Ok(Vec::new()),
        },
        Instant::now(),
    );

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadHistory {
                generation: 5,
                skip: 0,
                limit: HISTORY_PAGE_SIZE,
                ..
            }
        )
    ));
    assert!(app.history_loading);
    assert!(!app.history_refresh_again);
}

#[test]
fn scheduling_a_new_selection_immediately_invalidates_an_in_flight_preview() {
    let mut app = App::new("/tmp/repo", "repo");
    app.diff_generation = 7;
    app.document = DiffDocument::empty("Current", "keep me");
    app.local_diff_workspace_generation = Some(7);
    app.local_diff_index = Some(DiffIndex {
        title: "Previous commit".to_owned(),
        files: vec![crate::git::diff::DiffFileIndexEntry {
            path: PathBuf::from("stale.rs"),
            old_path: None,
            status: "modified".to_owned(),
            counts: None,
        }],
        truncated: false,
        commit_details: None,
    });

    let now = Instant::now();
    app.schedule_preview(now);
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), now);
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffFile {
            generation: 7,
            workspace_generation: 7,
            path: PathBuf::from("stale.rs"),
            result: Ok(DiffDocument::empty("Stale", "replace me")),
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert_eq!(app.diff_generation, 8);
    assert!(app.local_diff_index.is_none());
    assert!(app.local_diff_workspace_generation.is_none());
    assert_eq!(app.document.title, "Working Tree");
    assert_eq!(app.document.lines[0].text(), "Loading selected changes…");
}

#[test]
fn switching_views_replaces_stale_preview_before_async_work_completes() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.pull_request = Some(pull_request(6, "Slow preview", "acme/widget"));
    app.view = View::PullRequests;
    app.document = DiffDocument::empty("PR #6", "stale PR contents");
    app.history.push(Commit {
        id: "a".repeat(40),
        short_id: "aaaaaaa".to_owned(),
        parent_ids: Vec::new(),
        author: String::new(),
        author_email: String::new(),
        authored_at: String::new(),
        committer: String::new(),
        committer_email: String::new(),
        committed_at: String::new(),
        relative_date: String::new(),
        subject: "Selected history commit".to_owned(),
        decorations: Vec::new(),
    });

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);

    assert!(effects.is_empty());
    assert_eq!(app.view, View::History);
    assert_eq!(app.document.title, "aaaaaaa — Selected history commit");
    assert_eq!(app.document.lines[0].text(), "Loading commit preview…");
    assert!(app.preview_due.is_some());
}

#[test]
fn switching_views_invalidates_an_in_flight_pull_request_preview() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.view = View::PullRequests;
    app.pull_request = Some(pull_request(6, "Slow preview", "acme/widget"));
    app.diff_generation = 9;

    app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);
    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestDiff {
            generation: 9,
            result: Ok(DiffDocument::empty("PR #6", "stale PR contents")),
        },
        now,
    );

    assert!(effects.is_empty());
    assert_eq!(app.view, View::History);
    assert_eq!(app.document.title, "Commit History");
    assert_eq!(
        app.document.lines[0].text(),
        "No commits in this repository"
    );
    assert_ne!(app.document.lines[0].text(), "stale PR contents");
}

#[test]
fn stale_pull_request_metadata_does_not_replace_the_active_lookup() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_generation = 2;
    app.pull_request_loading = true;

    let effects = app.handle_worker_event(
        WorkerEvent::PullRequestLookup {
            generation: 1,
            result: Ok(crate::git::github::PullRequestSnapshot {
                repositories: Vec::new(),
                selected_repository: None,
                pull_request: pull_request(1, "Stale", "acme/widget"),
                warnings: Vec::new(),
                exact_number: Some(1),
                from_cache: false,
            }),
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    assert!(app.pull_request_loading);
    assert!(app.pull_request.is_none());
}

#[test]
fn exact_pull_request_lookup_accepts_only_digits_and_keeps_repository_scope() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    let repository = pull_request(1, "One", "acme/widget").base_repository;
    app.github_repositories = vec![repository.clone()];
    app.pull_request_repository = Some(repository.clone());
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
    for character in ['1', '2', 'x'] {
        app.handle_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
            now,
        );
    }
    app.handle_paste("abc3def");
    assert_eq!(app.pull_request_lookup.value, "123");

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LookupPullRequest {
                number: 123,
                repository: Some(selected),
                ..
            } if selected.url == repository.url
        )
    ));
    assert!(!app.pull_request_lookup_active);
}

#[test]
fn history_branch_picker_changes_only_the_viewed_revision() {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::History;
    app.status.branch.head = "main".to_owned();
    let now = Instant::now();

    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadHistoryBranches { generation: 1 }
        )
    ));
    app.handle_worker_event(
        WorkerEvent::HistoryBranches {
            generation: 1,
            result: Ok(vec![
                HistoryBranch {
                    name: "main".to_owned(),
                    reference: "refs/heads/main".to_owned(),
                    current: true,
                    remote: false,
                    relative_date: "now".to_owned(),
                    short_id: "aaaaaaa".to_owned(),
                },
                HistoryBranch {
                    name: "topic".to_owned(),
                    reference: "refs/heads/topic".to_owned(),
                    current: false,
                    remote: false,
                    relative_date: "now".to_owned(),
                    short_id: "bbbbbbb".to_owned(),
                },
            ]),
        },
        now,
    );
    let Some(Modal::HistoryBranches { selected, .. }) = app.modal.as_mut() else {
        panic!("expected history branch picker");
    };
    *selected = 1;

    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert_eq!(app.status.branch.head, "main");
    assert_eq!(app.history_branch_label(), "topic");
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadHistory { revision, skip: 0, .. }
                if revision == "refs/heads/topic"
        )
    ));
}

#[test]
fn collapse_preference_survives_documents_selections_and_views() {
    let mut app = app_with_changes();
    app.document = indexed_document(&["src/main.rs", "README.md"]);
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now);
    assert!(app.files_collapsed);
    app.toggle_preview_file(PathBuf::from("src/main.rs"), &mut Vec::new());
    assert!(
        app.files_collapsed,
        "a one-file override must not reset the preference"
    );
    assert!(!app.preview_file_collapsed("src/main.rs"));

    app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);
    assert!(app.files_collapsed);
    assert!(app.expanded_preview_files.is_empty());

    app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE), now);
    app.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE), now);
    assert!(app.files_collapsed);
}

#[test]
fn branch_dialog_renames_the_selected_local_branch() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::Branches {
        items: vec![Branch {
            name: "topic".to_owned(),
            current: false,
            upstream: Some("origin/topic".to_owned()),
            relative_date: "now".to_owned(),
            short_id: "abc1234".to_owned(),
        }],
        selected: 0,
        query: TextBuffer::default(),
        loading: false,
    });

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(effects.is_empty());
    let Some(Modal::Prompt { input, kind, .. }) = app.modal.as_mut() else {
        panic!("expected rename prompt");
    };
    assert_eq!(input.value, "topic");
    assert!(matches!(kind, PromptKind::RenameBranch { old } if old == "topic"));
    *input = TextBuffer::new("feature/topic");

    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::Operate {
                operation: GitOperation::RenameBranch { old, new }, ..
            } if old == "topic" && new == "feature/topic")
    ));
    assert_eq!(app.busy.as_deref(), Some("Renaming branch"));
}

#[test]
fn long_running_git_operations_animate_until_completion() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut effects = Vec::new();
    app.queue_operation(GitOperation::Pull, &mut effects);
    let initial = app.operation_spinner();

    let (_, changed) = app.tick(Instant::now());

    assert!(changed);
    assert_ne!(app.operation_spinner(), initial);
    assert_eq!(app.busy.as_deref(), Some("Pulling changes"));
}

#[expect(
    clippy::ref_patterns,
    reason = "matches! can only borrow the value under test"
)]
#[test]
fn compare_branch_picker_queues_a_head_diff_without_checkout() {
    let mut app = app_with_changes();
    app.history_branches_loaded = true;
    app.history_branches = vec![
        HistoryBranch {
            name: "main".to_owned(),
            reference: "refs/heads/main".to_owned(),
            current: true,
            remote: false,
            relative_date: "now".to_owned(),
            short_id: "aaaaaaa".to_owned(),
        },
        HistoryBranch {
            name: "topic".to_owned(),
            reference: "refs/heads/topic".to_owned(),
            current: false,
            remote: false,
            relative_date: "now".to_owned(),
            short_id: "bbbbbbb".to_owned(),
        },
    ];
    let now = Instant::now();

    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), now)
            .is_empty()
    );
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::PrepareLocalDiff { request, .. }
                if matches!(request.as_ref(), LocalDiffRequest::Branch { branch, .. } if branch.name == "topic")
        )
    ));
    assert!(matches!(
        app.auxiliary_preview,
        Some(AuxiliaryPreview::Branch(ref branch)) if branch.name == "topic"
    ));
    assert_eq!(app.focus, Focus::Content);
}

#[expect(
    clippy::ref_patterns,
    reason = "matches! can only borrow the value under test"
)]
#[test]
fn background_status_and_collapse_do_not_restart_a_branch_comparison() {
    let mut app = app_with_changes();
    app.history_branches_loaded = true;
    app.history_branches = vec![HistoryBranch {
        name: "topic".to_owned(),
        reference: "refs/heads/topic".to_owned(),
        current: false,
        remote: false,
        relative_date: "now".to_owned(),
        short_id: "bbbbbbb".to_owned(),
    }];
    let now = Instant::now();

    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), now);
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
    ));
    let index_generation = app.diff_generation;
    let effects = app.handle_worker_event(
        WorkerEvent::LocalDiffIndex {
            generation: index_generation,
            result: Ok(DiffIndex {
                title: "topic → HEAD — branch comparison".to_owned(),
                files: ["src/main.rs", "src/lib.rs"]
                    .into_iter()
                    .map(|path| crate::git::diff::DiffFileIndexEntry {
                        path: PathBuf::from(path),
                        old_path: None,
                        status: "modified".to_owned(),
                        counts: None,
                    })
                    .collect(),
                truncated: false,
                commit_details: None,
            }),
        },
        now,
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile { .. })
    ));
    let stable_generation = app.diff_generation;

    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now)
            .is_empty()
    );
    assert!(
        !app.files_collapsed,
        "first press expands the initial index"
    );
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now)
            .is_empty()
    );
    assert!(app.files_collapsed, "second press collapses every file");
    app.focus = Focus::Sidebar;
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now)
            .is_empty()
    );
    assert_eq!(app.diff_generation, stable_generation);
    assert!(matches!(
        app.auxiliary_preview,
        Some(AuxiliaryPreview::Branch(ref branch)) if branch.name == "topic"
    ));

    app.status_generation = 4;
    let effects = app.handle_worker_event(
        WorkerEvent::Status {
            generation: 4,
            result: Ok(app.status.clone()),
        },
        now + Duration::from_secs(10),
    );

    assert!(effects.iter().all(|effect| !matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
    )));
    assert_eq!(app.diff_generation, stable_generation);
    assert!(matches!(
        app.auxiliary_preview,
        Some(AuxiliaryPreview::Branch(ref branch)) if branch.name == "topic"
    ));
    assert_eq!(app.document.file_count(), 2);
}

#[test]
fn stash_manager_creates_a_named_stash_flow() {
    let mut app = app_with_changes();
    let now = Instant::now();
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::LoadStashes { .. })
    ));

    app.handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
        now,
    );
    let Some(Modal::Prompt { input, .. }) = app.modal.as_mut() else {
        panic!("expected stash message prompt");
    };
    *input = TextBuffer::new("checkpoint");
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::Operate {
                operation: GitOperation::StashPush {
                    message,
                    include_untracked: false,
                    staged: false,
                    paths,
                },
                ..
            } if message == "checkpoint" && paths.is_empty()
        )
    ));
}

#[test]
fn command_palette_can_rename_the_current_branch_but_not_detached_head() {
    let mut app = App::new("/tmp/repo", "repo");
    app.status.branch.head = "main".to_owned();
    let now = Instant::now();

    app.execute_palette(PaletteCommand::RenameCurrentBranch, &mut Vec::new(), now);
    assert!(matches!(
        app.modal,
        Some(Modal::Prompt {
            kind: PromptKind::RenameBranch { old },
            ..
        }) if old == "main"
    ));

    app.modal = None;
    app.status.branch.detached = true;
    app.execute_palette(PaletteCommand::RenameCurrentBranch, &mut Vec::new(), now);
    assert!(app.modal.is_none());
    assert_eq!(app.toast.as_ref().unwrap().level, ToastLevel::Error);
}

#[test]
fn command_palette_navigation_wraps() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.modal = Some(Modal::CommandPalette {
        query: TextBuffer::default(),
        selected: PaletteCommand::ALL.len().saturating_sub(1),
    });

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
    assert!(matches!(
        app.modal,
        Some(Modal::CommandPalette { selected: 0, .. })
    ));

    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
    assert!(matches!(
        app.modal,
        Some(Modal::CommandPalette { selected, .. })
            if selected == PaletteCommand::ALL.len().saturating_sub(1)
    ));
}

#[test]
fn list_navigation_wraps_and_handles_empty_lists() {
    assert_eq!(previous_list_index(0, 3), 2);
    assert_eq!(next_list_index(2, 3), 0);
    assert_eq!(previous_list_index(0, 0), 0);
    assert_eq!(next_list_index(0, 0), 0);
}

#[test]
fn command_palette_switches_theme_and_appearance_in_place() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.set_theme_selection(ThemeName::Catppuccin, AppearanceChoice::Dark);
    let original_background = app.theme.background;

    assert_eq!(
        app.palette_commands("change theme"),
        vec![PaletteCommand::ChangeTheme]
    );
    app.execute_palette(PaletteCommand::ChangeTheme, &mut Vec::new(), now);
    assert!(matches!(
        app.modal,
        Some(Modal::Themes {
            selected: 1,
            original: ThemeName::Catppuccin,
        })
    ));
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);

    assert_eq!(app.theme_name, ThemeName::Github);
    assert_ne!(app.theme.background, original_background);
    assert!(matches!(
        app.modal,
        Some(Modal::Themes { selected: 12, .. })
    ));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);

    assert_eq!(app.theme_name, ThemeName::Catppuccin);
    assert_eq!(app.theme.background, original_background);
    assert!(app.modal.is_none());

    app.execute_palette(PaletteCommand::ChangeTheme, &mut Vec::new(), now);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert_eq!(app.theme_name, ThemeName::Dracula);
    assert_eq!(app.appearance_choice, AppearanceChoice::Dark);
    assert_eq!(app.appearance, Appearance::Dark);
    assert_ne!(app.theme.background, original_background);
    assert!(app.modal.is_none());

    app.execute_palette(PaletteCommand::ChangeAppearance, &mut Vec::new(), now);
    assert!(matches!(
        app.modal,
        Some(Modal::Appearances {
            selected: 2,
            original_choice: AppearanceChoice::Dark,
            original_appearance: Appearance::Dark,
        })
    ));
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
    assert_eq!(app.appearance_choice, AppearanceChoice::Light);
    assert_eq!(app.appearance, Appearance::Light);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);

    assert_eq!(app.appearance_choice, AppearanceChoice::Dark);
    assert_eq!(app.appearance, Appearance::Dark);
    assert!(app.modal.is_none());

    app.execute_palette(PaletteCommand::ChangeAppearance, &mut Vec::new(), now);
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

    assert_eq!(app.theme_name, ThemeName::Dracula);
    assert_eq!(app.appearance_choice, AppearanceChoice::Light);
    assert_eq!(app.appearance, Appearance::Light);
    assert!(app.modal.is_none());
}

fn sample_worktree(path: &str, branch: &str, current: bool) -> Worktree {
    Worktree {
        path: PathBuf::from(path),
        head: "abcdef0123456789".to_owned(),
        branch: Some(branch.to_owned()),
        current,
        bare: false,
        detached: false,
        locked: None,
        prunable: None,
    }
}

#[test]
fn recent_projects_nest_worktrees_and_open_another_tree() {
    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    let groups = vec![
        ProjectGroup {
            name: "repo".to_owned(),
            common_dir: PathBuf::from("/tmp/repo/.git"),
            worktrees: vec![
                sample_worktree("/tmp/repo", "main", true),
                sample_worktree("/tmp/repo-topic", "topic", false),
            ],
        },
        ProjectGroup {
            name: "helix".to_owned(),
            common_dir: PathBuf::from("/src/helix/.git"),
            worktrees: vec![sample_worktree("/src/helix", "master", false)],
        },
    ];
    assert_eq!(
        App::filtered_project_rows(&groups, ""),
        vec![(0, 0), (0, 1), (1, 0)]
    );
    assert_eq!(App::filtered_project_rows(&groups, "helix"), vec![(1, 0)]);
    assert_eq!(App::filtered_project_rows(&groups, "topic"), vec![(0, 1)]);

    app.project_groups.clone_from(&groups);
    app.modal = Some(Modal::Projects {
        groups,
        selected: 1,
        query: TextBuffer::default(),
        loading: false,
    });
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::OpenRepository(path)] if path == Path::new("/tmp/repo-topic")
    ));
    assert!(app.modal.is_none());
}

#[test]
fn header_path_and_w_open_the_projects_picker() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    let effects = app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), now);
    assert!(matches!(app.modal, Some(Modal::Projects { .. })));
    assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::LoadRecentProjects { .. })
        )));

    app.modal = None;
    app.geometry.project_hits = vec![Rect::new(10, 0, 20, 1)];
    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
        now,
    );
    assert!(matches!(app.modal, Some(Modal::Projects { .. })));
    assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command) if matches!(command.as_ref(), WorkerCommand::LoadRecentProjects { .. })
        )));
}
