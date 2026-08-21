use super::*;

#[test]
fn a_comment_shows_its_code_intact_and_only_that_code_scrolls() {
    let mut app = overview_app();
    let wide = "│ ✓ Format, lint, and test (ubuntu-latest)   CI   passed in 33s   https://github.com/acme/widget/actions/runs/1/job/2 │";
    app.pull_request_conversation = crate::git::github::PullRequestConversation {
        truncated: false,
        from_cache: false,
        entries: vec![ConversationEntry {
            kind: ConversationKind::Comment,
            actor: "pulkitxm".to_owned(),
            timestamp: "2026-08-14T20:08:00Z".to_owned(),
            detail: String::new(),
            body: format!(
                "webhook path, driven against this PR with a body long enough to wrap.\n\n```\n{wide}\n  short line\n```"
            ),
            url: String::new(),
            reference: String::new(),
            context: String::new(),
        }],
    };

    let rows = conversation_rows(&app, 80, &Theme::default());
    let text = |row: &ContentRow| row.line.to_string();

    assert!(
        !rows.iter().any(|row| text(row).contains("```")),
        "fence markers are punctuation for a parser, never shown to a reader"
    );
    let scrollable: Vec<String> = rows.iter().filter(|row| row.wide).map(text).collect();
    assert!(scrollable.iter().any(|row| row.contains("cargo test")));
    assert!(scrollable.iter().any(|row| row.contains("  short line")));
    assert!(
        scrollable.iter().any(|row| row.contains("State")
            && row.contains(&format!(
                "opened {}",
                format_local_timestamp("2026-08-01T09:00:00Z")
            ))),
        "a single-line value that outgrows the pane scrolls rather than being clipped"
    );
    let long = rows
        .iter()
        .find(|row| row.wide && text(row).contains(wide))
        .expect("code keeps its full width rather than being cut at the pane");
    assert!(
        rows.iter()
            .filter(|row| !row.wide)
            .all(|row| row.line.width() <= 80),
        "everything that is not code is wrapped to fit"
    );

    let prose = rows
        .iter()
        .find(|row| !row.wide && text(row).contains("webhook path"))
        .expect("the comment body is rendered");
    assert_eq!(
        shift_line(&prose.line, 0, 80).to_string(),
        prose.line.to_string()
    );
    let shifted = shift_line(&long.line, 60, 80).to_string();
    assert!(
        shifted.contains("33s") && shifted.contains("job/2"),
        "scrolling reaches the tail of a line the pane cannot hold: {shifted:?}"
    );
    assert!(!shifted.contains("Format, lint"));
    assert!(
        shift_line(&long.line, 0, 80).to_string().len() < long.line.to_string().len(),
        "the unscrolled view is still clipped to the pane"
    );
}

#[test]
fn the_pane_says_whether_it_is_refreshing_or_showing_a_cached_answer() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_exact_number = Some(42);
    let theme = Theme::default();
    let render = |app: &mut App, terminal: &mut Terminal<TestBackend>| {
        terminal.draw(|frame| draw(frame, app, &theme)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>()
    };
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

    app.pull_request_conversation_loading = true;
    let refreshing = render(&mut app, &mut terminal);
    assert!(
        refreshing.contains("loading"),
        "a read in flight says that it is loading"
    );
    assert!(!refreshing.contains(['⟳', '↻', '↺']));
    assert!(!refreshing.contains("cached"));

    app.pull_request_conversation_loading = false;
    app.pull_request_from_cache = true;
    app.pull_request_checks_from_cache = true;
    app.pull_request_conversation.from_cache = true;
    let cached = render(&mut app, &mut terminal);
    assert!(
        cached.contains("cached"),
        "an answer served from disk says so rather than pretending to be live"
    );
    assert!(!cached.contains(['⟳', '↻', '↺']));

    app.pull_request_checks_from_cache = false;
    let live_checks = render(&mut app, &mut terminal);
    assert!(
        live_checks.contains("cached"),
        "a freshly read check list does not make the pull request itself live"
    );

    app.pull_request_conversation.from_cache = false;
    let live = render(&mut app, &mut terminal);
    assert!(!live.contains("cached"));
    assert!(!live.contains(['⟳', '↻', '↺']));
}

#[test]
fn a_failed_lookup_stays_readable_after_its_toast_expires() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_exact_number = Some(404);
    app.pull_request_error =
        Some("unable to load pull request: GraphQL: Could not resolve to a PullRequest".into());
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

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

    assert!(rendered.contains("could not be opened"));
    assert!(rendered.contains("Could not resolve to a PullRequest"));
    assert!(rendered.contains("Press r to try again"));
}

#[test]
fn pull_request_overview_reads_as_a_conversation_beside_its_checks() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_conversation = crate::git::github::PullRequestConversation {
        truncated: false,
        from_cache: false,
        entries: vec![
            ConversationEntry {
                kind: ConversationKind::Opened,
                actor: "octocat".to_owned(),
                timestamp: "2026-08-01T09:00:00Z".to_owned(),
                detail: "feature/rocket into main".to_owned(),
                body: "## Summary".to_owned(),
                url: String::new(),
                reference: String::new(),
                context: String::new(),
            },
            ConversationEntry {
                kind: ConversationKind::ForcePush,
                actor: "octocat".to_owned(),
                timestamp: "2026-08-02T09:10:00Z".to_owned(),
                detail: String::new(),
                body: String::new(),
                url: String::new(),
                reference: "deadbeefcafe".to_owned(),
                context: String::new(),
            },
            ConversationEntry {
                kind: ConversationKind::ReviewComment,
                actor: "reviewer".to_owned(),
                timestamp: "2026-08-02T09:30:00Z".to_owned(),
                detail: "src/main.rs:42".to_owned(),
                body: "Extract this into a helper".to_owned(),
                url: String::new(),
                reference: String::new(),
                context: "@@ -1 +1 @@\n-old\n+new".to_owned(),
            },
        ],
    };
    let mut terminal = Terminal::new(TestBackend::new(150, 40)).unwrap();

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

    assert!(rendered.contains("Conversation"));
    assert!(rendered.contains("Format, lint, and test"));
    assert!(rendered.contains("#42"));
    assert!(rendered.contains("acme/widget:main"));
    assert!(rendered.contains(&format_local_timestamp("2026-08-01T09:00:00Z")));
    assert!(rendered.contains("Description"));
    assert!(
        rendered.contains("Launch safely"),
        "the body renders as prose, not as raw Markdown"
    );
    assert!(rendered.contains("cargo test"), "fenced code survives");
    assert!(rendered.contains("opened this pull request"));
    assert!(rendered.contains("force-pushed to deadbee"));
    assert!(rendered.contains("commented on src/main.rs:42"));
    assert!(rendered.contains("Extract this into a helper"));
    assert!(
        !rendered.contains("## Summary"),
        "the opening post never repeats the description above it"
    );
}

#[test]
fn selecting_a_check_shows_its_steps_and_opens_the_failure() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    app.pull_request_step_cursor = 2;
    app.expanded_check_steps.insert(2);
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        truncated: false,
        unavailable: None,
        log_pending: false,
        loose_lines: vec![CheckLogLine {
            timestamp: "2026-08-02T10:02:31Z".to_owned(),
            text: "Cleaning up runner".to_owned(),
            severity: CheckLogSeverity::Normal,
        }],
        steps: vec![
            CheckStep {
                number: 1,
                name: "Set up job".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: "2026-08-02T10:00:00Z".to_owned(),
                completed_at: "2026-08-02T10:00:02Z".to_owned(),
                lines: vec![CheckLogLine {
                    timestamp: "2026-08-02T10:00:01Z".to_owned(),
                    text: "hidden while folded".to_owned(),
                    severity: CheckLogSeverity::Normal,
                }],
            },
            CheckStep {
                number: 2,
                name: "Run cargo test".to_owned(),
                status: PullRequestCheckStatus::Failed,
                conclusion: "failure".to_owned(),
                started_at: "2026-08-02T10:00:02Z".to_owned(),
                completed_at: "2026-08-02T10:02:30Z".to_owned(),
                lines: vec![CheckLogLine {
                    timestamp: "2026-08-02T10:02:29Z".to_owned(),
                    text: "test tests::rockets ... FAILED".to_owned(),
                    severity: CheckLogSeverity::Error,
                }],
            },
        ],
    });
    let mut terminal = Terminal::new(TestBackend::new(150, 40)).unwrap();

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

    assert!(rendered.contains("2 steps"));
    assert!(rendered.contains("Set up job"));
    assert!(rendered.contains("Run cargo test"));
    assert!(
        rendered.contains("2m 28s"),
        "a step reports how long it ran"
    );
    assert!(rendered.contains("test tests::rockets ... FAILED"));
    assert!(
        !rendered.contains("hidden while folded"),
        "a folded step keeps its output out of the way"
    );
    assert!(rendered.contains("Runner output"));
    assert!(rendered.contains("Cleaning up runner"));
    assert!(
        !app.geometry.content_step_hits.is_empty(),
        "step rows stay clickable"
    );

    let rows = check_run_rows(&app, 40, &Theme::default());
    let log = rows
        .iter()
        .find(|row| row.line.to_string().contains("rockets"))
        .expect("the expanded step's output is rendered");
    assert!(log.wide, "a log line scrolls instead of being truncated");
    assert!(
        log.line
            .to_string()
            .ends_with("test tests::rockets ... FAILED"),
        "the line keeps its full text even in a pane far narrower than it"
    );
}

#[test]
fn pull_request_loading_renders_on_demand_progress_and_skeletons() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_loading = true;
    app.pull_request_exact_number = Some(42);
    app.pull_request_lookup = crate::app::TextBuffer::new("42");
    app.pull_request_progress = Some(crate::git::github::PullRequestProgress::FetchingHead);
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rendered: String = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    let bottom = (24..27)
        .flat_map(|row| (0..42).map(move |column| buffer[(column, row)].symbol()))
        .collect::<String>();

    assert!(rendered.contains("50%"));
    assert!(rendered.contains("Fetching the source commit"));
    assert!(rendered.contains('█'));
    assert!(bottom.contains("auto-detect"));
    assert!(bottom.contains("PR #"));
}

#[test]
fn empty_pull_request_view_renders_recent_numbers_and_titles_as_rows() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.recent_pull_requests = vec![RecentPullRequest {
        number: 39,
        title: "Restore selectable previews".to_owned(),
        repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["origin".to_owned()],
        },
    }];
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();

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

    assert!(rendered.contains("Recent Pull Requests"));
    assert!(rendered.contains("#39 Restore selectable previews"));
    assert!(
        app.geometry
            .sidebar_hits
            .iter()
            .any(|hit| { matches!(hit.target, SidebarHit::RecentPullRequest(0)) })
    );
}
