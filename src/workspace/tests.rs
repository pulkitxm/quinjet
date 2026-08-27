use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use super::*;
use crate::app::{Modal, ProjectOpenMode, TextBuffer, View};
use crate::git::support::same_path;
use crate::git::tests::TestRepository;
use crate::integration::Client;
use crate::ssh::{SshMachine, SshProjectOpenMode};

fn test_repository(branch: &str) -> (TestRepository, Repository) {
    let fixture = TestRepository::with_branch(branch);
    let repository = fixture.repository();
    (fixture, repository)
}

fn workspace(repository: &Repository) -> RepositoryWorkspace {
    RepositoryWorkspace::new(
        repository,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(None, None),
    )
}

#[test]
fn machine_session_restores_open_tabs_in_order_and_reactivates_the_saved_project() {
    let (_first_directory, first_repository) = test_repository("first");
    let (_second_directory, second_repository) = test_repository("second");
    let session = ProjectSession {
        roots: vec![
            first_repository.root().to_path_buf(),
            PathBuf::from("/missing/project"),
            second_repository.root().to_path_buf(),
        ],
        active: Some(first_repository.root().to_path_buf()),
    };

    let mut restored = RepositoryWorkspace::restore(
        &session,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(None, None),
    )
    .expect("restored workspace");

    let tabs = restored.tabs.infos();
    assert_eq!(tabs.len(), 2);
    assert!(
        tabs.first()
            .is_some_and(|tab| { tab.active && same_path(&tab.root, first_repository.root()) })
    );
    assert!(
        tabs.get(1)
            .is_some_and(|tab| { !tab.active && same_path(&tab.root, second_repository.root()) })
    );
    let restored_session = restored.project_session();
    assert_eq!(restored_session.roots.len(), 2);
    assert!(
        restored_session
            .active
            .as_deref()
            .is_some_and(|active| same_path(active, first_repository.root()))
    );
    assert_eq!(restored.initial_effects().len(), 2);
}

#[test]
fn machine_picker_context_follows_replaced_and_appended_projects() {
    let (_first_directory, first_repository) = test_repository("first");
    let (_second_directory, second_repository) = test_repository("second");
    let context = SshContext {
        current: "local".to_owned(),
        machines: vec![SshMachine {
            target: "remote-host".to_owned(),
            folder: "/work/remote".into(),
            accessible: true,
            uses: 5,
            local: false,
        }],
        tabs: crate::ssh::SshTabs::default(),
        probing: false,
    };
    let mut workspace = RepositoryWorkspace::new(
        &first_repository,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(Some(context.clone()), None),
    );
    let now = Instant::now();
    let first = workspace.active_id().expect("first tab is active");
    let second = workspace
        .append_repository(first, &second_repository, now)
        .expect("append second repository")
        .id;

    assert_eq!(
        workspace
            .app_mut(first)
            .and_then(|app| app.ssh_context.as_ref())
            .map(|saved| (&saved.current, &saved.machines)),
        Some((&context.current, &context.machines))
    );
    assert_eq!(
        workspace
            .app_mut(second)
            .and_then(|app| app.ssh_context.as_ref())
            .map(|saved| (&saved.current, &saved.machines)),
        Some((&context.current, &context.machines))
    );
}

#[test]
fn reachability_results_update_apps_without_replacing_open_tabs() {
    let (_directory, repository) = test_repository("main");
    let context = SshContext {
        current: "local".to_owned(),
        machines: vec![
            SshMachine {
                target: "local".to_owned(),
                folder: repository.root().to_path_buf(),
                accessible: true,
                uses: 0,
                local: true,
            },
            SshMachine {
                target: "remote".to_owned(),
                folder: "/remote".into(),
                accessible: false,
                uses: 2,
                local: false,
            },
        ],
        tabs: crate::ssh::SshTabs::default(),
        probing: true,
    };
    let mut workspace = RepositoryWorkspace::new(
        &repository,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(Some(context), None),
    );
    let now = Instant::now();
    let active = workspace.active_id().expect("active tab");
    let pending = workspace
        .open_repository_tab_picker(active, now)
        .expect("pending tab")
        .id;
    let tabs = workspace.tabs.infos();

    workspace.apply_ssh_probe(
        &[("local".to_owned(), true), ("remote".to_owned(), true)],
        now,
    );

    assert_eq!(workspace.tabs.infos(), tabs);
    let saved = workspace.ssh_context().expect("SSH context");
    assert!(!saved.probing);
    assert!(saved.machines[1].accessible);
    assert_eq!(saved.tabs.active_id(), Some(pending));
    assert!(
        workspace
            .app_mut(pending)
            .and_then(|app| app.ssh_context.as_ref())
            .is_some_and(|context| !context.probing && context.machines[1].accessible)
    );
}

#[test]
fn mixed_machine_tabs_share_one_order_and_remote_tabs_handoff_directly() {
    let (_first_directory, first_repository) = test_repository("first");
    let (_second_directory, second_repository) = test_repository("second");
    let mut tabs = crate::ssh::SshTabs::default();
    let first = tabs.append(
        "macbook",
        first_repository.name(),
        first_repository.root().to_path_buf(),
    );
    let remote = tabs.append("tof", "remote-repo", "/work/remote-repo");
    let second = tabs.append(
        "macbook",
        second_repository.name(),
        second_repository.root().to_path_buf(),
    );
    let _stale = tabs.append("macbook", "stale", "/missing/project");
    drop(tabs.activate(first));
    let context = SshContext {
        current: "macbook".to_owned(),
        machines: vec![
            SshMachine {
                target: "macbook".to_owned(),
                folder: first_repository.root().to_path_buf(),
                accessible: true,
                uses: 0,
                local: true,
            },
            SshMachine {
                target: "tof".to_owned(),
                folder: "/work".into(),
                accessible: true,
                uses: 4,
                local: false,
            },
        ],
        tabs,
        probing: false,
    };
    let session = ProjectSession::default();
    let mut workspace = RepositoryWorkspace::restore(
        &session,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(Some(context), None),
    )
    .expect("mixed workspace");

    let visible = workspace
        .active_app_mut()
        .expect("active app")
        .repository_tabs
        .clone();
    assert_eq!(
        visible
            .iter()
            .map(|tab| (tab.id, tab.machine.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (first, Some("macbook")),
            (remote, Some("tof")),
            (second, Some("macbook")),
        ]
    );
    assert_eq!(
        workspace.activate(remote, Instant::now()),
        Some(SshSwitch {
            index: 1,
            mode: SshProjectOpenMode::Activate,
        })
    );
    assert_eq!(
        workspace
            .ssh_context()
            .and_then(|saved| saved.tabs.active_id()),
        Some(remote)
    );
}

#[test]
fn activating_real_repository_tabs_restores_each_apps_state() {
    let (_first_directory, first_repository) = test_repository("alpha");
    let (_second_directory, second_repository) = test_repository("beta");
    let mut workspace = workspace(&first_repository);
    let now = Instant::now();
    let first = workspace.active_id().expect("first tab is active");
    let second = workspace
        .append_repository(first, &second_repository, now)
        .expect("append second repository")
        .id;

    let first_app = workspace.app_mut(first).expect("first app");
    first_app.view = View::History;
    first_app.history_cursor = 4;
    first_app.content_scroll = 31;
    let second_app = workspace.app_mut(second).expect("second app");
    second_app.view = View::PullRequests;
    second_app.content_scroll = 73;

    assert_eq!(workspace.activate(first, now), None);
    let first_app = workspace.active_app_mut().expect("first app is active");
    assert_eq!(first_app.repository_root, first_repository.root());
    assert_eq!(first_app.view, View::History);
    assert_eq!(first_app.history_cursor, 4);
    assert_eq!(first_app.content_scroll, 31);

    assert_eq!(workspace.activate(second, now), None);
    let second_app = workspace.active_app_mut().expect("second app is active");
    assert_eq!(second_app.repository_root, second_repository.root());
    assert_eq!(second_app.view, View::PullRequests);
    assert_eq!(second_app.content_scroll, 73);
    assert_eq!(
        workspace.app_mut(first).map(|app| app.content_scroll),
        Some(31)
    );
}

#[test]
fn replacement_keeps_tab_identity_and_new_tabs_follow_the_close_lifecycle() {
    let (_first_directory, first_repository) = test_repository("first");
    let (_second_directory, second_repository) = test_repository("second");
    let (_replacement_directory, replacement_repository) = test_repository("replacement");
    let mut workspace = workspace(&first_repository);
    let now = Instant::now();
    let first = workspace.active_id().expect("first tab is active");
    let second_effects = workspace
        .append_repository(first, &second_repository, now)
        .expect("append second repository");
    let second = second_effects.id;

    assert_ne!(first, second);
    assert_eq!(workspace.active_id(), Some(second));
    assert_eq!(workspace.tabs.len(), 2);
    assert_eq!(workspace.activate(first, now), None);
    let replacement_effects = workspace
        .replace_repository(first, &replacement_repository, now)
        .expect("replace first repository");

    assert_eq!(replacement_effects.id, first);
    assert_eq!(workspace.active_id(), Some(first));
    assert_eq!(workspace.tabs.len(), 2);
    assert_eq!(workspace.tabs.id_for_root(first_repository.root()), None);
    assert_eq!(
        workspace.tabs.id_for_root(replacement_repository.root()),
        Some(first)
    );
    assert_eq!(
        workspace
            .tabs
            .infos()
            .into_iter()
            .map(|tab| tab.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );

    assert_eq!(workspace.close(first, now), (true, None));
    assert_eq!(workspace.active_id(), Some(second));
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(workspace.close(second, now), (false, None));
    assert!(workspace.tabs.is_empty());
}

#[test]
fn failed_project_open_returns_to_the_picker() {
    let (_directory, repository) = test_repository("first");
    let mut workspace = workspace(&repository);
    let source = workspace.active_id().expect("active tab");
    workspace.app_mut(source).expect("source app").modal = Some(Modal::Projects {
        groups: Vec::new(),
        selected: 0,
        query: TextBuffer::default(),
        collapsed: HashSet::new(),
        loading: false,
        opening: Some("/missing/project".into()),
        mode: ProjectOpenMode::CurrentTab,
    });

    assert!(
        workspace
            .switch_repository(source, Path::new("/missing/project"), Instant::now())
            .is_none()
    );
    assert!(matches!(
        workspace.app_mut(source).and_then(|app| app.modal.as_ref()),
        Some(Modal::Projects { opening: None, .. })
    ));
}

#[test]
fn edith_workspace_refuses_to_close_its_managed_session() {
    let (_directory, repository) = test_repository("managed");
    let mut workspace = RepositoryWorkspace::new(
        &repository,
        ThemeName::Quinjet.into(),
        AppearanceChoice::Dark,
        false,
        false,
        WorkspaceContext::new(None, Some(Client::Edith)),
    );
    let active = workspace.active_id().expect("managed tab is active");

    assert!(workspace.exit_locked());
    let (keep_running, handoff) = workspace.close(active, Instant::now());
    assert!(keep_running);
    assert!(handoff.is_none());
    assert_eq!(workspace.active_id(), Some(active));
}

#[test]
fn worker_events_update_only_the_repository_tab_that_owns_them() {
    let (_first_directory, first_repository) = test_repository("worker-alpha");
    let (_second_directory, second_repository) = test_repository("worker-beta");
    let mut workspace = workspace(&first_repository);
    let now = Instant::now();
    let first = workspace.active_id().expect("first tab is active");
    let second = workspace
        .append_repository(first, &second_repository, now)
        .expect("append second repository")
        .id;
    workspace
        .app_mut(first)
        .expect("first app")
        .status_generation = 101;
    workspace
        .app_mut(second)
        .expect("second app")
        .status_generation = 202;

    assert!(workspace.send(first, WorkerCommand::Refresh { generation: 101 }));
    assert!(workspace.send(second, WorkerCommand::Refresh { generation: 202 }));

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut routed = HashSet::new();
    loop {
        routed.extend(
            workspace
                .drain_worker_events(Instant::now())
                .into_iter()
                .map(|effects| effects.id),
        );
        let first_branch = workspace
            .app_mut(first)
            .map(|app| app.status.branch.head.clone());
        let second_branch = workspace
            .app_mut(second)
            .map(|app| app.status.branch.head.clone());
        if first_branch.as_deref() == Some("worker-alpha")
            && second_branch.as_deref() == Some("worker-beta")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "workers did not return repository-specific status before the deadline"
        );
        thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(routed, HashSet::from([first, second]));
    assert_eq!(
        workspace
            .app_mut(first)
            .map(|app| app.status.branch.head.as_str()),
        Some("worker-alpha")
    );
    assert_eq!(
        workspace
            .app_mut(second)
            .map(|app| app.status.branch.head.as_str()),
        Some("worker-beta")
    );
}

mod projects;
