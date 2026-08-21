use super::*;

#[test]
fn pull_request_and_check_urls_share_clickable_link_hits() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    let conversation_url = "https://github.com/acme/widget/pull/42#issuecomment-123".to_owned();
    app.pull_request_conversation.entries = vec![ConversationEntry {
        kind: ConversationKind::Comment,
        actor: "reviewer".to_owned(),
        timestamp: "2026-08-02T11:00:00Z".to_owned(),
        detail: String::new(),
        body: "Looks good".to_owned(),
        url: conversation_url.clone(),
        reference: String::new(),
        context: String::new(),
    }];
    let mut terminal = Terminal::new(TestBackend::new(140, 32)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let pull_request_url = "https://github.com/acme/widget/pull/42";
    assert_eq!(
        app.geometry
            .link_hits
            .iter()
            .filter(|hit| matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == pull_request_url
            ))
            .count(),
        3
    );
    for expected in [
        "https://github.com/octocat",
        "https://github.com/reviewer",
        "https://github.com/acme/widget/tree/feature/rocket",
        "https://github.com/acme/widget/tree/main",
        conversation_url.as_str(),
    ] {
        assert!(
            app.geometry
                .link_hits
                .iter()
                .any(|hit| { matches!(&hit.target, OpenTarget::Browser(url) if url == expected) })
        );
    }
    let pull_request_url_area = app
        .geometry
        .link_hits
        .iter()
        .filter(|hit| {
            matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == pull_request_url
            )
        })
        .max_by_key(|hit| hit.area.width)
        .map(|hit| hit.area)
        .unwrap();
    let effects = app.handle_mouse(
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: pull_request_url_area.x,
            row: pull_request_url_area.y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        std::time::Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [crate::app::AppEffect::Open(OpenTarget::Browser(url))]
            if url == pull_request_url
    ));

    app.pull_request_check_cursor = Some(0);
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let check_url = "https://github.com/acme/widget/actions/runs/9/job/12";
    assert_eq!(
        app.geometry
            .link_hits
            .iter()
            .filter(|hit| matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == check_url
            ))
            .count(),
        2
    );
    let check_url_area = app
        .geometry
        .link_hits
        .iter()
        .filter(|hit| {
            matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == check_url
            )
        })
        .max_by_key(|hit| hit.area.width)
        .map(|hit| hit.area)
        .unwrap();
    let effects = app.handle_mouse(
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: check_url_area.x,
            row: check_url_area.y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        std::time::Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [crate::app::AppEffect::Open(OpenTarget::Browser(url))] if url == check_url
    ));
}

#[test]
fn a_long_conversation_stays_bounded_to_render() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    let body = "Some reasonably long review comment body that wraps across several \
terminal rows because that is what real pull-request comments look like in practice."
        .to_owned();
    app.pull_request_conversation = crate::git::github::PullRequestConversation {
        truncated: true,
        from_cache: false,
        entries: (0..500)
            .map(|index| ConversationEntry {
                kind: if index % 3 == 0 {
                    ConversationKind::Comment
                } else {
                    ConversationKind::Commit
                },
                actor: "octocat".to_owned(),
                timestamp: "2026-08-02T09:10:00Z".to_owned(),
                detail: "abc1234".to_owned(),
                body: body.clone(),
                url: String::new(),
                reference: String::new(),
                context: String::new(),
            })
            .collect(),
    };

    let rows = conversation_rows(&app, 120, &Theme::default());

    assert!(
        rows.len() < 3_000,
        "a thread at the fetch cap still builds a bounded number of rows: {}",
        rows.len()
    );
    assert!(
        rows.iter()
            .any(|row| row.line.to_string().contains("Older activity was omitted")),
        "a truncated thread says so rather than silently dropping history"
    );

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let cache_key = app.pull_request_content_rows_key;
    let cache_pointer = app.pull_request_content_rows.as_ptr();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(app.pull_request_content_rows_key, cache_key);
    assert_eq!(app.pull_request_content_rows.as_ptr(), cache_pointer);
}

#[test]
fn a_large_check_log_scrolls_from_a_cached_layout() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    assert!(app.expanded_check_steps.insert(1));
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        steps: vec![CheckStep {
            number: 1,
            name: "Large build".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
            lines: (0..50_000)
                .map(|index| CheckLogLine {
                    timestamp: String::new(),
                    text: format!("output line {index}"),
                    severity: CheckLogSeverity::Normal,
                })
                .collect(),
        }],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(app.pull_request_content_rows.len() > 50_000);
    let cache_pointer = app.pull_request_content_rows.as_ptr();
    app.content_scroll = usize::MAX;
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

    assert_eq!(app.pull_request_content_rows.as_ptr(), cache_pointer);
    assert!(rendered.contains("output line 49999"));
}

#[test]
fn an_expanded_step_can_be_scrolled_past_to_reach_the_steps_below_it() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    let line = |text: &str| CheckLogLine {
        timestamp: "2026-08-02T10:00:01Z".to_owned(),
        text: text.to_owned(),
        severity: CheckLogSeverity::Normal,
    };
    let step = |number: usize, lines: usize| CheckStep {
        number,
        name: format!("Step {number}"),
        status: PullRequestCheckStatus::Passed,
        conclusion: "success".to_owned(),
        started_at: "2026-08-02T10:00:00Z".to_owned(),
        completed_at: "2026-08-02T10:00:05Z".to_owned(),
        lines: (0..lines)
            .map(|index| line(&format!("output line {index}")))
            .collect(),
    };
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        steps: vec![step(1, 300), step(2, 0), step(3, 0)],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    app.expanded_check_steps.insert(1);
    app.pull_request_step_cursor = 1;
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    app.content_scroll = 120;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(
        app.content_scroll, 120,
        "a redraw must not drag the view back to the selected step"
    );

    app.content_scroll = usize::MAX;
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
    assert!(
        rendered.contains("Step 2") && rendered.contains("Step 3"),
        "the steps below a long expanded step are reachable by scrolling"
    );
    assert!(
        !rendered.contains("output line 0 "),
        "the view really moved past the expanded output"
    );
}

#[test]
fn the_selected_step_is_the_row_that_is_highlighted() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    let theme = Theme::default();
    app.pull_request_check_cursor = Some(0);
    let step = |number: usize| CheckStep {
        number,
        name: format!("Run step {number}"),
        status: PullRequestCheckStatus::Passed,
        conclusion: "success".to_owned(),
        started_at: "2026-08-02T10:00:00Z".to_owned(),
        completed_at: "2026-08-02T10:00:01Z".to_owned(),
        lines: Vec::new(),
    };
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        steps: (1..=4).map(step).collect(),
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

    let highlighted = |app: &mut App, terminal: &mut Terminal<TestBackend>| {
        terminal.draw(|frame| draw(frame, app, &theme)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..30)
            .filter(|y| buffer[(60, *y)].style().bg == Some(theme.selected))
            .map(|y| {
                (44..99)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };

    for cursor in [2, 4, 1] {
        app.pull_request_step_cursor = cursor;
        let rows = highlighted(&mut app, &mut terminal);
        assert_eq!(
            rows.len(),
            1,
            "exactly one row is highlighted, not a range: {rows:?}"
        );
        assert!(
            rows[0].contains(&format!("Run step {cursor}")),
            "the highlight marks the step the cursor is on: {rows:?}"
        );
    }
}

#[test]
fn a_running_check_shows_the_step_it_is_on_before_any_log_exists() {
    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    app.expanded_check_steps.insert(2);
    app.pull_request_step_cursor = 2;
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        truncated: false,
        unavailable: None,
        log_pending: true,
        loose_lines: Vec::new(),
        steps: vec![
            CheckStep {
                number: 1,
                name: "Set up job".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: "2026-08-02T10:00:00Z".to_owned(),
                completed_at: "2026-08-02T10:00:02Z".to_owned(),
                lines: Vec::new(),
            },
            CheckStep {
                number: 2,
                name: "Run cargo test".to_owned(),
                status: PullRequestCheckStatus::Pending,
                conclusion: String::new(),
                started_at: "2026-08-02T10:00:02Z".to_owned(),
                completed_at: String::new(),
                lines: Vec::new(),
            },
        ],
    });

    let rendered = check_run_rows(&app, 100, &Theme::default())
        .iter()
        .map(|row| row.line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("2 steps"));
    assert!(
        rendered.contains("Set up job") && rendered.contains("2s"),
        "a finished step still reports how long it took"
    );
    assert!(
        rendered.contains("Run cargo test"),
        "the step in progress is visible rather than hidden behind the missing log"
    );
    assert!(rendered.contains("waiting for output…"));
    assert!(
        rendered.contains("Waiting for the runner"),
        "the view says why there is no output yet instead of looking broken"
    );
}
