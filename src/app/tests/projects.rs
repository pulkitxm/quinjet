use super::*;
use crate::ssh::SshMachine;

fn sample_worktree(path: &str, branch: &str, current: bool) -> Worktree {
    Worktree {
        path: PathBuf::from(path),
        head: "abcdef0123456789".to_owned(),
        updated_at: Some("2026-08-22T18:00:00Z".to_owned()),
        updated_unix: Some(1_776_964_800),
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
        App::filtered_project_rows(&groups, "", &HashSet::new()),
        vec![
            ProjectRow::Group(0),
            ProjectRow::Worktree {
                group_index: 0,
                tree_index: 0,
            },
            ProjectRow::Worktree {
                group_index: 0,
                tree_index: 1,
            },
            ProjectRow::Group(1),
            ProjectRow::Worktree {
                group_index: 1,
                tree_index: 0,
            },
        ]
    );
    assert_eq!(
        App::filtered_project_rows(&groups, "helix", &HashSet::new()),
        vec![
            ProjectRow::Group(1),
            ProjectRow::Worktree {
                group_index: 1,
                tree_index: 0,
            },
        ]
    );
    assert_eq!(
        App::filtered_project_rows(&groups, "topic", &HashSet::new()),
        vec![
            ProjectRow::Group(0),
            ProjectRow::Worktree {
                group_index: 0,
                tree_index: 1,
            },
        ]
    );

    app.project_groups.clone_from(&groups);
    app.modal = Some(Modal::Projects {
        groups,
        selected: 2,
        query: TextBuffer::default(),
        collapsed: HashSet::new(),
        loading: false,
        opening: None,
        mode: ProjectOpenMode::CurrentTab,
    });
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SwitchRepository(path)] if path == Path::new("/tmp/repo-topic")
    ));
    assert!(matches!(
        app.modal.as_ref(),
        Some(Modal::Projects {
            opening: Some(path),
            ..
        }) if path == Path::new("/tmp/repo-topic")
    ));
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

#[test]
fn machine_handoff_reopens_projects_in_new_tab_mode() {
    let mut app = App::new("/tmp/repo", "repo");
    let effects = app.open_projects_on_launch(ProjectOpenMode::NewTab);

    assert!(matches!(
        app.modal,
        Some(Modal::Projects {
            mode: ProjectOpenMode::NewTab,
            ..
        })
    ));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadRecentProjects { .. })
    )));
}

#[test]
fn control_e_expands_mixed_projects_then_collapses_all() {
    let mut app = App::new("/tmp/repo", "repo");
    let groups = vec![
        ProjectGroup {
            name: "repo".to_owned(),
            common_dir: PathBuf::from("/tmp/repo/.git"),
            worktrees: vec![sample_worktree("/tmp/repo", "main", true)],
        },
        ProjectGroup {
            name: "helix".to_owned(),
            common_dir: PathBuf::from("/src/helix/.git"),
            worktrees: vec![sample_worktree("/src/helix-topic", "topic", false)],
        },
    ];
    app.modal = Some(Modal::Projects {
        groups,
        selected: 0,
        query: TextBuffer::default(),
        collapsed: HashSet::from([PathBuf::from("/tmp/repo/.git")]),
        loading: false,
        opening: None,
        mode: ProjectOpenMode::CurrentTab,
    });

    drop(app.handle_key(
        KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
        Instant::now(),
    ));

    let Some(Modal::Projects {
        groups,
        query,
        collapsed,
        ..
    }) = app.modal.as_mut()
    else {
        panic!("project picker closed");
    };
    assert!(collapsed.is_empty());
    assert_eq!(App::filtered_project_rows(groups, "", collapsed).len(), 4);
    query.insert_str("topic");
    assert_eq!(
        App::filtered_project_rows(groups, &query.value, collapsed),
        vec![
            ProjectRow::Group(1),
            ProjectRow::Worktree {
                group_index: 1,
                tree_index: 0,
            },
        ]
    );
    query.value.clear();

    drop(app.handle_key(
        KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
        Instant::now(),
    ));
    let Some(Modal::Projects { collapsed, .. }) = app.modal.as_ref() else {
        panic!("project picker closed");
    };
    assert_eq!(collapsed.len(), 2);
}

#[test]
fn clicking_a_project_button_only_toggles_that_project() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    let mut app = App::new("/tmp/repo", "repo");
    let common_dir = PathBuf::from("/tmp/repo/.git");
    app.modal = Some(Modal::Projects {
        groups: vec![ProjectGroup {
            name: "repo".to_owned(),
            common_dir: common_dir.clone(),
            worktrees: vec![sample_worktree("/tmp/repo", "main", true)],
        }],
        selected: 0,
        query: TextBuffer::default(),
        collapsed: HashSet::new(),
        loading: false,
        opening: None,
        mode: ProjectOpenMode::CurrentTab,
    });
    app.geometry.project_collapse_hits = vec![(Rect::new(10, 5, 3, 1), common_dir.clone())];

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 11,
            row: 5,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    );

    assert!(effects.is_empty());
    let Some(Modal::Projects { collapsed, .. }) = &app.modal else {
        panic!("project picker closed");
    };
    assert_eq!(collapsed, &HashSet::from([common_dir]));
}

#[test]
fn project_headers_are_keyboard_rows_and_remember_their_fold() {
    let mut app = App::new("/tmp/repo", "repo");
    let common_dir = PathBuf::from("/tmp/repo/.git");
    app.modal = Some(Modal::Projects {
        groups: vec![ProjectGroup {
            name: "repo".to_owned(),
            common_dir: common_dir.clone(),
            worktrees: vec![sample_worktree("/tmp/repo", "main", true)],
        }],
        selected: 1,
        query: TextBuffer::default(),
        collapsed: HashSet::new(),
        loading: false,
        opening: None,
        mode: ProjectOpenMode::CurrentTab,
    });
    let now = Instant::now();

    drop(app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now));
    drop(app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now));

    let Some(Modal::Projects {
        selected,
        collapsed,
        ..
    }) = &app.modal
    else {
        panic!("project picker closed");
    };
    assert_eq!(*selected, 0);
    assert_eq!(collapsed, &HashSet::from([common_dir.clone()]));
    assert_eq!(app.collapsed_project_groups, HashSet::from([common_dir]));
}

#[test]
fn project_picker_omits_prunable_worktrees() {
    let mut stale = sample_worktree("/tmp/repo-stale", "stale", false);
    stale.prunable = Some("gitdir file points to a missing path".to_owned());
    let groups = vec![ProjectGroup {
        name: "repo".to_owned(),
        common_dir: PathBuf::from("/tmp/repo/.git"),
        worktrees: vec![sample_worktree("/tmp/repo", "main", true), stale],
    }];

    assert_eq!(
        App::filtered_project_rows(&groups, "", &HashSet::new()),
        vec![
            ProjectRow::Group(0),
            ProjectRow::Worktree {
                group_index: 0,
                tree_index: 0,
            },
        ]
    );
}

#[test]
fn project_picker_is_the_machine_switching_entry_point() {
    let mut app = App::new("/tmp/repo", "repo");
    app.ssh_context = Some(SshContext {
        current: "current-host".to_owned(),
        machines: vec![
            SshMachine {
                target: "Pulkits-MacBook-Pro.local".to_owned(),
                folder: PathBuf::from("/Users/pulkit"),
                accessible: true,
                uses: 0,
                local: true,
            },
            SshMachine {
                target: "current-host".to_owned(),
                folder: PathBuf::from("/work/current"),
                accessible: true,
                uses: 3,
                local: false,
            },
            SshMachine {
                target: "next-host".to_owned(),
                folder: PathBuf::from("/work/next"),
                accessible: true,
                uses: 2,
                local: false,
            },
        ],
    });
    app.modal = Some(Modal::Projects {
        groups: Vec::new(),
        selected: 0,
        query: TextBuffer::default(),
        collapsed: HashSet::new(),
        loading: false,
        opening: None,
        mode: ProjectOpenMode::NewTab,
    });
    let now = Instant::now();
    let effects = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now);
    assert!(matches!(app.modal, Some(Modal::Projects { .. })));
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SwitchSshMachine(crate::ssh::SshSwitch {
            index: 2,
            mode: crate::ssh::SshProjectOpenMode::NewTab,
        })]
    ));

    let effects = app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SwitchSshMachine(crate::ssh::SshSwitch {
            index: 0,
            mode: crate::ssh::SshProjectOpenMode::NewTab,
        })]
    ));
}

#[test]
fn machine_shortcut_skips_unavailable_machines() {
    let mut app = App::new("/tmp/repo", "repo");
    app.ssh_context = Some(SshContext {
        current: "current".to_owned(),
        machines: vec![
            SshMachine {
                target: "current".to_owned(),
                folder: PathBuf::from("/work/current"),
                accessible: true,
                uses: 5,
                local: true,
            },
            SshMachine {
                target: "offline".to_owned(),
                folder: PathBuf::from("/work/offline"),
                accessible: false,
                uses: 4,
                local: false,
            },
        ],
    });
    app.modal = Some(Modal::Projects {
        groups: Vec::new(),
        selected: 0,
        query: TextBuffer::default(),
        collapsed: HashSet::new(),
        loading: false,
        opening: None,
        mode: ProjectOpenMode::NewTab,
    });
    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(effects.is_empty());
    assert!(matches!(app.modal, Some(Modal::Projects { .. })));
}
