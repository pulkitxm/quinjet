use super::*;

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
        mode: ProjectOpenMode::CurrentTab,
    });
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SwitchRepository(path)] if path == Path::new("/tmp/repo-topic")
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

#[test]
fn project_picker_is_the_machine_switching_entry_point() {
    let mut app = App::new("/tmp/repo", "repo");
    app.ssh_context = Some(SshContext {
        current: "current-host".to_owned(),
        machines: vec![
            SshMachine {
                target: "busy-host".to_owned(),
                folder: PathBuf::from("/work/busy"),
                accessible: true,
                uses: 12,
            },
            SshMachine {
                target: "current-host".to_owned(),
                folder: PathBuf::from("/work/current"),
                accessible: true,
                uses: 3,
            },
        ],
    });
    app.modal = Some(Modal::Projects {
        groups: Vec::new(),
        selected: 0,
        query: TextBuffer::default(),
        loading: false,
        mode: ProjectOpenMode::NewTab,
    });
    let now = Instant::now();
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now)
            .is_empty()
    );
    assert!(matches!(
        app.modal,
        Some(Modal::SshMachines { selected: 1, .. })
    ));
    drop(app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now));
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::SwitchSshMachine(0)]
    ));
}

#[test]
fn unavailable_machine_cannot_be_selected() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::SshMachines {
        items: vec![SshMachine {
            target: "offline".to_owned(),
            folder: PathBuf::from("/work/offline"),
            accessible: false,
            uses: 4,
        }],
        selected: 0,
        current: "current".to_owned(),
        parent: Box::new(Modal::Projects {
            groups: Vec::new(),
            selected: 0,
            query: TextBuffer::default(),
            loading: false,
            mode: ProjectOpenMode::NewTab,
        }),
    });
    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        Instant::now(),
    );
    assert!(effects.is_empty());
    assert!(matches!(app.modal, Some(Modal::SshMachines { .. })));
}
