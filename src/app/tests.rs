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
        action_state: crate::git::github::PullRequestActionState {
            viewer_can_close: true,
            viewer_can_reopen: true,
            viewer_can_update: true,
            viewer_can_update_branch: true,
            viewer_can_subscribe: true,
            viewer_did_author: true,
            viewer_subscription: "SUBSCRIBED".to_owned(),
            ..crate::git::github::PullRequestActionState::default()
        },
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
    let items = app.pr_menu_items();
    assert!(items.contains(&PrMenuItem::Merge(PullRequestMergeMethod::Merge)));
    assert!(items.contains(&PrMenuItem::Merge(PullRequestMergeMethod::Rebase)));
    assert!(items.contains(&PrMenuItem::AutoMerge));
    assert!(items.contains(&PrMenuItem::Review));
    assert!(items.contains(&PrMenuItem::Comments));
    assert!(items.contains(&PrMenuItem::Edit));
    assert!(items.contains(&PrMenuItem::UpdateBranch));
    assert!(items.contains(&PrMenuItem::Stage));
    assert!(items.contains(&PrMenuItem::Unsubscribe));
    assert!(items.contains(&PrMenuItem::AllowMaintainerEdits));
    assert!(items.contains(&PrMenuItem::Close));

    app.preferred_merge_method = PullRequestMergeMethod::Rebase;
    assert_eq!(
        app.pr_primary_action(),
        Some(PrPrimaryAction::Merge(PullRequestMergeMethod::Rebase))
    );
    let items = app.pr_menu_items();
    assert!(items.contains(&PrMenuItem::Merge(PullRequestMergeMethod::Merge)));
    assert!(items.contains(&PrMenuItem::Merge(PullRequestMergeMethod::Squash)));

    if let Some(pull_request) = app.pull_request.as_mut() {
        pull_request.is_draft = true;
    }
    assert_eq!(app.pr_primary_action(), Some(PrPrimaryAction::Ready));
    let items = app.pr_menu_items();
    assert!(items.contains(&PrMenuItem::Stage));
    assert!(
        !items
            .iter()
            .any(|item| matches!(item, PrMenuItem::Merge(_)))
    );

    if let Some(pull_request) = app.pull_request.as_mut() {
        pull_request.is_draft = false;
        pull_request.action_state.auto_merge_method = "SQUASH".to_owned();
        pull_request.action_state.maintainer_can_modify = true;
    }
    assert_eq!(
        app.pr_primary_action(),
        Some(PrPrimaryAction::DisableAutoMerge)
    );
    assert!(
        app.pr_menu_items()
            .contains(&PrMenuItem::DisallowMaintainerEdits)
    );

    if let Some(pull_request) = app.pull_request.as_mut() {
        pull_request.action_state.auto_merge_method.clear();
        pull_request.action_state.merge_queue_entry_id = "MQE_node".to_owned();
    }
    assert_eq!(app.pr_primary_action(), Some(PrPrimaryAction::Dequeue));

    if let Some(pull_request) = app.pull_request.as_mut() {
        pull_request.state = "CLOSED".to_owned();
        pull_request.action_state.merge_queue_entry_id.clear();
    }
    assert_eq!(app.pr_primary_action(), Some(PrPrimaryAction::Reopen));
    let items = app.pr_menu_items();
    assert!(items.contains(&PrMenuItem::Comments));
    assert!(items.contains(&PrMenuItem::Edit));
    assert!(items.contains(&PrMenuItem::Unsubscribe));
    assert!(items.contains(&PrMenuItem::OpenInBrowser));
    assert!(!items.contains(&PrMenuItem::Close));

    if let Some(pull_request) = app.pull_request.as_mut() {
        pull_request.state = "MERGED".to_owned();
    }
    assert_eq!(
        app.pr_primary_action(),
        Some(PrPrimaryAction::OpenInBrowser)
    );
    let items = app.pr_menu_items();
    assert!(items.contains(&PrMenuItem::Revert));
    assert!(items.contains(&PrMenuItem::Comments));
    assert!(items.contains(&PrMenuItem::OpenInBrowser));
}

#[test]
fn pull_request_action_pickers_route_review_and_edit_flows() {
    let now = Instant::now();
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request = Some(pull_request(12, "Ship it", "acme/widget"));
    let mut effects = Vec::new();

    app.handle_pr_menu_item(PrMenuItem::Review, &mut effects);
    assert!(matches!(
        &app.modal,
        Some(Modal::PullRequestActions { items, .. })
            if items.as_slice() == [
                PrActionItem::Review(PullRequestReviewKind::Approve),
                PrActionItem::Review(PullRequestReviewKind::Comment),
                PrActionItem::Review(PullRequestReviewKind::RequestChanges),
            ]
    ));
    drop(app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now));
    drop(app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now));
    assert!(matches!(
        app.modal,
        Some(Modal::Prompt {
            kind: PromptKind::PullRequest {
                action: PrActionItem::Review(PullRequestReviewKind::Comment),
                ..
            },
            ..
        })
    ));
    app.handle_paste("Looks good overall");
    let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    assert!(effects.iter().any(|effect| {
        matches!(
            effect,
            AppEffect::Git(command)
                if matches!(
                    command.as_ref(),
                    WorkerCommand::OperatePullRequest {
                        operation: PullRequestOperation::Review {
                            kind: PullRequestReviewKind::Comment,
                            body,
                        },
                        ..
                    } if body == "Looks good overall"
                )
        )
    }));

    app.busy = None;
    app.handle_pr_menu_item(PrMenuItem::Edit, &mut Vec::new());
    drop(app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now));
    drop(app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now));
    assert!(matches!(
        &app.modal,
        Some(Modal::Prompt {
            input,
            kind: PromptKind::PullRequest {
                action: PrActionItem::Edit(PullRequestEditField::Body),
                ..
            },
            ..
        }) if input.value == "A detailed pull-request description"
    ));
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

mod basics;
mod checks;
mod diffs;
mod discovery;
mod interaction;
mod prefetch;
mod projects;
mod refresh;
mod refresh_preview;
mod refresh_preview_state;
mod reviews;
mod support;
mod workflows;
