use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;

#[test]
fn repository_pick_lists_render_loaded_rows() {
    let branch = Branch {
        name: "topic".to_owned(),
        current: false,
        upstream: Some("origin/topic".to_owned()),
        relative_date: "2026-08-20T10:00:00Z".to_owned(),
        short_id: "abc1234".to_owned(),
    };
    let history_branch = HistoryBranch {
        name: "origin/topic".to_owned(),
        reference: "refs/remotes/origin/topic".to_owned(),
        current: false,
        remote: true,
        relative_date: "2026-08-20T10:00:00Z".to_owned(),
        short_id: "abc1234".to_owned(),
    };
    let stash = Stash {
        reference: "stash@{0}".to_owned(),
        message: "save topic work".to_owned(),
        branch: "topic".to_owned(),
        relative_date: "2026-08-20T10:00:00Z".to_owned(),
        short_id: "def5678".to_owned(),
    };
    let query = crate::app::TextBuffer::default();
    let cases = [
        (
            Modal::Branches {
                items: vec![branch],
                selected: 0,
                query: query.clone(),
                loading: false,
            },
            "topic",
        ),
        (
            Modal::HistoryBranches {
                items: vec![history_branch.clone()],
                selected: 0,
                query: query.clone(),
                loading: false,
            },
            "origin/topic",
        ),
        (
            Modal::CompareBranches {
                items: vec![history_branch],
                selected: 0,
                query: query.clone(),
                loading: false,
            },
            "origin/topic",
        ),
        (
            Modal::Stashes {
                items: vec![stash],
                selected: 0,
                query,
                loading: false,
            },
            "save topic work",
        ),
    ];

    for (modal, expected) in cases {
        let mut app = App::new("/tmp/repo", "repo");
        app.modal = Some(modal);
        let mut terminal = Terminal::new(TestBackend::new(120, 36)).unwrap();
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
        assert!(rendered.contains(expected));
        assert_eq!(app.geometry.modal_list_len, 1);
        assert_eq!(app.geometry.modal_list_hits.len(), 1);
    }
}

#[test]
fn pull_request_action_picker_renders_every_choice_as_a_hit_target() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::PullRequestActions {
        title: "Submit Review".to_owned(),
        items: vec![
            PrActionItem::Review(crate::git::github::PullRequestReviewKind::Approve),
            PrActionItem::Review(crate::git::github::PullRequestReviewKind::Comment),
            PrActionItem::Review(crate::git::github::PullRequestReviewKind::RequestChanges),
        ],
        selected: 1,
    });
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("Submit Review"));
    assert!(rendered.contains("Approve pull request"));
    assert!(rendered.contains("Submit review comment"));
    assert!(rendered.contains("Request changes"));
    assert_eq!(app.geometry.modal_action_hits.len(), 3);
    assert_eq!(app.geometry.modal_list_len, 3);
    assert_eq!(app.geometry.modal_list_hits.len(), 3);
    assert!(
        app.geometry
            .modal_action_hits
            .iter()
            .any(|(_, action)| *action == ModalAction::PullRequestAction(1))
    );
}

#[test]
fn modal_free_scroll_keeps_selection_independent_from_the_viewport() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::CommandPalette {
        query: crate::app::TextBuffer::default(),
        selected: 0,
    });
    app.modal_scroll = 3;
    app.modal_free_scroll = true;
    let mut terminal = Terminal::new(TestBackend::new(90, 18)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert_eq!(
        app.geometry
            .modal_list_hits
            .first()
            .map(|(_, index)| *index),
        Some(3)
    );
    assert!(matches!(
        app.modal.as_ref(),
        Some(Modal::CommandPalette { selected: 0, .. })
    ));
}
