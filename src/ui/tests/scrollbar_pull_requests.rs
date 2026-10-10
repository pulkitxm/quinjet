use std::fmt::Write;

use crossterm::event::{MouseButton, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::scrollbars::{mouse, render};
use super::*;

fn scroll_to_end(app: &mut App, terminal: &mut Terminal<TestBackend>, sidebar: bool) {
    render(app, terminal);
    let scrollbar = if sidebar {
        app.geometry.sidebar_scrollbar.unwrap()
    } else {
        app.geometry.content_scrollbar.unwrap()
    };
    assert!(
        mouse(
            app,
            MouseEventKind::Down(MouseButton::Left),
            scrollbar.thumb.x,
            scrollbar.thumb.y,
        )
        .is_empty()
    );
    assert!(
        mouse(
            app,
            MouseEventKind::Drag(MouseButton::Left),
            scrollbar.track.x,
            scrollbar.track.bottom() - 1,
        )
        .is_empty()
    );
    render(app, terminal);
    assert_eq!(
        if sidebar {
            app.sidebar_offset
        } else {
            app.content_scroll
        },
        scrollbar.max_offset
    );
    assert!(
        mouse(
            app,
            MouseEventKind::Up(MouseButton::Left),
            scrollbar.track.x,
            scrollbar.track.bottom() - 1,
        )
        .is_empty()
    );
    assert!(app.scrollbar_drag.is_none());
    assert!(app.text_selection.is_none());
}

#[test]
fn pull_request_file_tree_scrollbar_preserves_the_selected_file() {
    let mut app = overview_app();
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_files = (0..80)
        .map(|index| PullRequestFile {
            path: format!("file-{index:02}.rs").into(),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    scroll_to_end(&mut app, &mut terminal, true);
    assert_eq!(app.pull_request_tree_cursor, 0);
    assert!(rendered_text(terminal.backend().buffer()).contains("file-79.rs"));
}

#[test]
fn pull_request_check_list_scrollbar_preserves_the_selected_check() {
    let mut app = overview_app();
    let check = app.pull_request_checks.first().unwrap().clone();
    app.pull_request_checks = (0..80)
        .map(|index| PullRequestCheck {
            name: format!("Check {index:02}"),
            ..check.clone()
        })
        .collect();
    let selected = app.pull_request_check_cursor;
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    scroll_to_end(&mut app, &mut terminal, true);
    assert_eq!(app.pull_request_check_cursor, selected);
    assert!(rendered_text(terminal.backend().buffer()).contains("Check 79"));
}

#[test]
fn conversation_scrollbar_reaches_the_last_comment() {
    let mut app = overview_app();
    app.pull_request_conversation.entries = (0..60)
        .map(|index| ConversationEntry {
            kind: ConversationKind::Comment,
            actor: "reviewer".to_owned(),
            timestamp: "2026-08-02T11:00:00Z".to_owned(),
            detail: String::new(),
            body: format!("Comment {index:02}"),
            url: String::new(),
            reference: String::new(),
            context: String::new(),
        })
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    scroll_to_end(&mut app, &mut terminal, false);
    assert!(app.content_at_bottom);
    assert!(rendered_text(terminal.backend().buffer()).contains("Comment 59"));
}

#[test]
fn check_log_scrollbar_reaches_the_last_line_without_folding_the_step() {
    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    app.pull_request_step_cursor = 1;
    app.expanded_check_steps.insert(1);
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        truncated: false,
        unavailable: None,
        log_pending: false,
        loose_lines: Vec::new(),
        steps: vec![CheckStep {
            number: 1,
            name: "Run tests".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: "2026-08-02T10:00:00Z".to_owned(),
            completed_at: "2026-08-02T10:02:30Z".to_owned(),
            lines: (0..100)
                .map(|index| CheckLogLine {
                    timestamp: String::new(),
                    text: format!("Log line {index:02}"),
                    severity: CheckLogSeverity::Normal,
                })
                .collect(),
        }],
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    scroll_to_end(&mut app, &mut terminal, false);
    assert!(app.expanded_check_steps.contains(&1));
    assert_eq!(app.pull_request_step_cursor, 1);
    assert!(rendered_text(terminal.backend().buffer()).contains("Log line 99"));
}

#[test]
fn stack_member_scrollbar_uses_the_detail_body_below_the_header() {
    let mut app = stack::stack_app();
    app.stack_inspector.section = StackMemberSection::Summary;
    let description = &mut app
        .stack_inspector
        .selected_pull_request
        .as_mut()
        .unwrap()
        .description;
    description.clear();
    for index in 0..100 {
        writeln!(description, "Detail {index:02}\n").unwrap();
    }
    let mut terminal = Terminal::new(TestBackend::new(140, 32)).unwrap();
    scroll_to_end(&mut app, &mut terminal, false);
    let scrollbar = app.geometry.content_scrollbar.unwrap();
    assert!(scrollbar.track.y > app.geometry.content.y + 3);
    assert!(app.content_at_bottom);
    assert!(rendered_text(terminal.backend().buffer()).contains("Detail 99"));
}
