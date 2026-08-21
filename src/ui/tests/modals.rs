use super::*;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

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
    }
}
