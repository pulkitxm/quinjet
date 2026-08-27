use super::*;

#[test]
fn new_project_picker_creates_and_resolves_only_its_pending_tab() {
    let (_first_directory, first_repository) = test_repository("first");
    let (_second_directory, second_repository) = test_repository("second");
    let mut workspace = workspace(&first_repository);
    let now = Instant::now();
    let first = workspace.active_id().expect("first tab is active");
    let first_app = workspace.app_mut(first).expect("first app");
    first_app.view = View::History;
    first_app.content_scroll = 41;

    let picker = workspace
        .open_repository_tab_picker(first, now)
        .expect("pending project tab");
    let pending = picker.id;

    assert_ne!(pending, first);
    assert_eq!(workspace.active_id(), Some(pending));
    assert_eq!(workspace.tabs.len(), 2);
    assert!(workspace.tabs.is_pending(pending));
    assert_eq!(
        workspace
            .tabs
            .infos()
            .into_iter()
            .map(|tab| (tab.id, tab.title, tab.active))
            .collect::<Vec<_>>(),
        vec![
            (first, first_repository.name(), false),
            (pending, "New project".to_owned(), true),
        ]
    );
    assert_eq!(
        workspace.project_session().roots,
        vec![first_repository.root().to_path_buf()]
    );
    assert!(matches!(
        workspace
            .app_mut(pending)
            .and_then(|app| app.modal.as_ref()),
        Some(Modal::Projects {
            mode: ProjectOpenMode::NewTab,
            ..
        })
    ));
    assert!(picker.effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(command.as_ref(), WorkerCommand::LoadRecentProjects { .. })
    )));

    let resolved = workspace
        .open_repository_tab(pending, second_repository.root(), now)
        .expect("resolved project tab");

    assert_eq!(resolved.id, pending);
    assert_eq!(workspace.active_id(), Some(pending));
    assert_eq!(workspace.tabs.len(), 2);
    assert!(!workspace.tabs.is_pending(pending));
    assert!(
        workspace
            .app_mut(pending)
            .is_some_and(|app| same_path(&app.repository_root, second_repository.root()))
    );
    let first_app = workspace.app_mut(first).expect("first app remains open");
    assert_eq!(first_app.repository_root, first_repository.root());
    assert_eq!(first_app.view, View::History);
    assert_eq!(first_app.content_scroll, 41);
}

#[test]
fn pending_project_tab_moves_between_machines_without_hiding_existing_tabs() {
    let (_local_directory, local_repository) = test_repository("local");
    let (_remote_directory, remote_repository) = test_repository("remote");
    let (_selected_directory, selected_repository) = test_repository("selected");
    let context = SshContext {
        current: "macbook".to_owned(),
        machines: vec![
            SshMachine {
                target: "macbook".to_owned(),
                folder: local_repository.root().to_path_buf(),
                accessible: true,
                uses: 0,
                local: true,
            },
            SshMachine {
                target: "tof".to_owned(),
                folder: remote_repository.root().to_path_buf(),
                accessible: true,
                uses: 4,
                local: false,
            },
        ],
        tabs: crate::ssh::SshTabs::default(),
        probing: false,
    };
    let mut local = RepositoryWorkspace::new(
        &local_repository,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(Some(context), None),
    );
    let now = Instant::now();
    let local_id = local.active_id().expect("local tab");
    let pending = local
        .open_repository_tab_picker(local_id, now)
        .expect("pending tab")
        .id;
    let request = SshSwitch {
        index: 1,
        mode: SshProjectOpenMode::New,
    };

    local.prepare_ssh_switch(pending, request);

    let mut handoff = local.ssh_context().expect("SSH handoff context");
    assert_eq!(handoff.tabs.active_id(), Some(pending));
    assert!(handoff.tabs.is_pending(pending));
    assert_eq!(
        handoff.tabs.get(pending).map(|tab| tab.machine.as_str()),
        Some("tof")
    );
    assert!(handoff.tabs.get(local_id).is_some());
    handoff.current = "tof".to_owned();
    let session = ProjectSession {
        roots: vec![remote_repository.root().to_path_buf()],
        active: Some(remote_repository.root().to_path_buf()),
    };
    let mut remote = RepositoryWorkspace::restore(
        &session,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(Some(handoff), None),
    )
    .expect("remote workspace");

    assert_eq!(remote.active_id(), Some(pending));
    assert!(remote.tabs.is_pending(pending));
    let visible = remote
        .active_app_mut()
        .expect("pending app")
        .repository_tabs
        .clone();
    assert!(visible.iter().any(|tab| tab.id == local_id));
    assert!(
        visible
            .iter()
            .any(|tab| tab.id == pending && tab.active && tab.title == "New project")
    );

    let resolved = remote
        .open_repository_tab(pending, selected_repository.root(), now)
        .expect("selected remote project");

    assert_eq!(resolved.id, pending);
    assert!(!remote.tabs.is_pending(pending));
    let shared = remote.ssh_context().expect("updated SSH context");
    assert!(shared.tabs.get(local_id).is_some());
    let selected = shared.tabs.get(pending).expect("resolved shared tab");
    assert_eq!(selected.machine, "tof");
    assert!(same_path(&selected.root, selected_repository.root()));
    assert_eq!(selected.title, selected_repository.name());
}
