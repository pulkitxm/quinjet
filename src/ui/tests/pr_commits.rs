use super::*;
use crate::git::github::PullRequestCommit;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::time::Instant;

fn commits_app() -> App {
    let mut app = overview_app();
    app.pr_commits.loaded = true;
    app.pr_commits.snapshot.total_commits = 2;
    app.pr_commits.snapshot.commits = vec![
        PullRequestCommit {
            oid: "a".repeat(40),
            abbreviated_oid: "aaaaaaa".to_owned(),
            subject: "Add launch countdown".to_owned(),
            author: "octocat".to_owned(),
            ..PullRequestCommit::default()
        },
        PullRequestCommit {
            oid: "b".repeat(40),
            abbreviated_oid: "bbbbbbb".to_owned(),
            subject: "Add rocket engines".to_owned(),
            author: "octocat".to_owned(),
            ..PullRequestCommit::default()
        },
    ];
    app
}

#[test]
fn commit_rows_are_clickable_and_open_a_hash_scoped_file_view() {
    let mut app = commits_app();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    drop(app.handle_key(
        KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT),
        Instant::now(),
    ));
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let text = rendered_text(terminal.backend().buffer());
    assert!(text.contains("Commits"));
    assert!(text.contains("aaaaaaa"));
    let hit = app
        .geometry
        .sidebar_hits
        .iter()
        .find(|hit| matches!(hit.target, SidebarHit::PullRequestCommit(1)))
        .unwrap()
        .area;
    drop(app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x + 1,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        },
        Instant::now(),
    ));
    assert_eq!(app.selected_pr_commit().unwrap().oid, "b".repeat(40));
    assert_eq!(app.pull_request_section, PullRequestSection::Files);
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let text = rendered_text(terminal.backend().buffer());
    assert!(text.contains("bbbbbbb"));
    assert!(text.contains("All commits"));
    assert!(
        app.geometry
            .sidebar_hits
            .iter()
            .any(|hit| matches!(hit.target, SidebarHit::PullRequestAllCommits))
    );
}

#[test]
fn commit_ci_summary_keeps_the_hash_visible_without_pr_conversation() {
    let mut app = commits_app();
    app.pr_commits.selected = Some(0);
    app.pull_request_section = PullRequestSection::Overview;
    for width in [80, 120] {
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(text.contains("aaaaaaa"));
        assert!(text.contains("Commit checks"));
        assert!(!text.contains("Conversation"));
        assert!(
            app.geometry
                .sidebar_hits
                .iter()
                .any(|hit| matches!(hit.target, SidebarHit::PullRequestOverview))
        );
        assert!(
            app.geometry
                .sidebar_hits
                .iter()
                .any(|hit| matches!(hit.target, SidebarHit::PullRequestFiles))
        );
    }
}
