use super::*;
use crate::git::diff::CommitDetails;

fn overview_app() -> App {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_exact_number = Some(42);
    app.pull_request = Some(PullRequest {
        number: 42,
        title: "Ship the rocket".to_owned(),
        description: "## Summary\n- Launch **safely**\n\n```sh\ncargo test\n```".to_owned(),
        author: "octocat".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: "2026-08-01T09:00:00Z".to_owned(),
        updated_at: "2026-08-02T10:30:00Z".to_owned(),
        url: "https://github.com/acme/widget/pull/42".to_owned(),
        base_ref: "main".to_owned(),
        base_oid: String::new(),
        head_ref: "feature/rocket".to_owned(),
        head_oid: String::new(),
        base_repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["origin".to_owned()],
        },
        head_repository: Some("acme/widget".to_owned()),
        head_remotes: vec!["origin".to_owned()],
        is_cross_repository: false,
        additions: 101,
        deletions: 20,
        changed_files: 3,
        action_state: crate::git::github::PullRequestActionState::default(),
    });
    app.pull_request_checks = vec![PullRequestCheck {
        name: "Format, lint, and test".to_owned(),
        workflow: "CI".to_owned(),
        state: "FAILURE".to_owned(),
        status: PullRequestCheckStatus::Failed,
        description: String::new(),
        link: "https://github.com/acme/widget/actions/runs/9/job/12".to_owned(),
        started_at: "2026-08-02T10:00:00Z".to_owned(),
        completed_at: "2026-08-02T10:02:30Z".to_owned(),
    }];
    app
}

fn test_file_header(path: &str, additions: usize, deletions: usize) -> DiffLine {
    DiffLine {
        kind: DiffLineKind::FileHeader,
        old_line: None,
        new_line: None,
        spans: vec![
            HighlightSpan {
                text: path.to_owned(),
                foreground: None,
                bold: false,
                italic: false,
            },
            HighlightSpan {
                text: format!("+{additions}"),
                foreground: None,
                bold: false,
                italic: false,
            },
            HighlightSpan {
                text: format!("-{deletions}"),
                foreground: None,
                bold: false,
                italic: false,
            },
        ],
    }
}

fn test_line(kind: DiffLineKind, text: &str) -> DiffLine {
    DiffLine {
        kind,
        old_line: None,
        new_line: None,
        spans: vec![HighlightSpan {
            text: text.to_owned(),
            foreground: None,
            bold: false,
            italic: false,
        }],
    }
}

mod controls;
mod diff;
mod layout;
mod logs;
mod modals;
mod overview;
mod rendering;
