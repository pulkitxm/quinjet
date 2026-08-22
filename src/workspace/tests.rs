use std::collections::HashSet;
use std::thread;
use std::time::{Duration, Instant};

use super::*;
use crate::app::View;
use crate::git::tests::TestRepository;

fn test_repository(branch: &str) -> (TestRepository, Repository) {
    let fixture = TestRepository::with_branch(branch);
    let repository = fixture.repository();
    (fixture, repository)
}

fn workspace(repository: &Repository) -> RepositoryWorkspace {
    RepositoryWorkspace::new(
        repository,
        ThemeName::Quinjet,
        AppearanceChoice::Dark,
        false,
        false,
        None,
    )
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

    workspace.activate(first, now);
    let first_app = workspace.active_app_mut().expect("first app is active");
    assert_eq!(first_app.repository_root, first_repository.root());
    assert_eq!(first_app.view, View::History);
    assert_eq!(first_app.history_cursor, 4);
    assert_eq!(first_app.content_scroll, 31);

    workspace.activate(second, now);
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
    workspace.activate(first, now);
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

    assert!(workspace.close(first, now));
    assert_eq!(workspace.active_id(), Some(second));
    assert_eq!(workspace.tabs.len(), 1);
    assert!(!workspace.close(second, now));
    assert!(workspace.tabs.is_empty());
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
