use super::*;

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
fn command_palette_exposes_new_project_tabs() {
    let mut app = App::new("/tmp/repo", "repo");
    let commands = app.palette_commands("new tab");

    assert_eq!(commands, vec![PaletteCommand::OpenProjectNewTab]);
    let mut effects = Vec::new();
    app.execute_palette(
        PaletteCommand::OpenProjectNewTab,
        &mut effects,
        Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::OpenRepositoryTabPicker]
    ));
    assert!(app.modal.is_none());
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
