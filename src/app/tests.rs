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

mod basics;
mod checks;
mod diffs;
mod discovery;
mod interaction;
mod prefetch;
mod projects;
mod refresh;
mod workflows;
