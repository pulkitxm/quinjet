use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;
use crate::git::github::{PullRequestStack, PullRequestStackMember};

fn stack() -> PullRequestStack {
    PullRequestStack {
        node_id: "stack-node".to_owned(),
        number: 19,
        base_ref: "main".to_owned(),
        size: 3,
        selected_position: 3,
        members: (1..=3)
            .map(|position| PullRequestStackMember {
                node_id: format!("pr-node-{position}"),
                entry_id: format!("entry-{position}"),
                position,
                number: 40 + u64::try_from(position).unwrap_or_default(),
                title: format!("Build layer {position}"),
                author: "octocat".to_owned(),
                state: if position == 1 { "MERGED" } else { "OPEN" }.to_owned(),
                is_draft: false,
                updated_at: "2026-08-20T10:00:00Z".to_owned(),
                url: format!("https://github.com/acme/widget/pull/{}", 40 + position),
                base_ref: "main".to_owned(),
                base_oid: format!("{position:040x}"),
                head_ref: format!("layer-{position}"),
                head_oid: format!("{:040x}", position + 10),
                head_repository: Some("acme/widget".to_owned()),
                is_cross_repository: false,
                additions: position * 10,
                deletions: position,
                changed_files: position,
                merge_state: "CLEAN".to_owned(),
                mergeable: "MERGEABLE".to_owned(),
                review_decision: "APPROVED".to_owned(),
                checks_state: "SUCCESS".to_owned(),
                is_queued: false,
            })
            .collect(),
        truncated: false,
        repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["origin".to_owned()],
        },
    }
}

#[test]
fn pull_request_stack_tab_renders_the_selected_range_and_member_hits() {
    let mut app = overview_app();
    app.pull_request_stack = Some(stack());
    app.pull_request_stack_anchor = Some(2);
    app.pull_request_stack_cursor = Some(3);
    app.pull_request_section = PullRequestSection::Stack;
    let mut terminal = Terminal::new(TestBackend::new(120, 28)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("Stack 3"));
    assert!(rendered.contains("range 2..3"));
    assert!(rendered.contains("#41"));
    assert!(rendered.contains("#43"));
    assert!(
        app.geometry
            .sidebar_hits
            .iter()
            .any(|hit| matches!(hit.target, SidebarHit::PullRequestStack))
    );
    assert_eq!(
        app.geometry
            .sidebar_hits
            .iter()
            .filter(|hit| matches!(hit.target, SidebarHit::PullRequestStackMember(_)))
            .count(),
        3
    );
}

#[test]
fn pull_request_stack_tab_is_hidden_without_stack_metadata() {
    let mut app = overview_app();
    let mut terminal = Terminal::new(TestBackend::new(120, 28)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(!app.geometry.sidebar_hits.iter().any(|hit| matches!(
        hit.target,
        SidebarHit::PullRequestStack | SidebarHit::PullRequestStackMember(_)
    )));
}
