#![expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]

use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn answer(task: &str, text: &str) -> AiAnswer {
    AiAnswer {
        task: task.to_owned(),
        text: text.to_owned(),
        model: "claude-opus-5-5".to_owned(),
    }
}

fn queued(effects: &[AppEffect]) -> (u64, AiTask) {
    let Some(AppEffect::Git(command)) = effects.last() else {
        panic!("no worker command was queued");
    };
    let WorkerCommand::RunAi { generation, task } = command.as_ref() else {
        panic!("another worker command was queued");
    };
    (*generation, task.as_ref().clone())
}

fn service(running: bool) -> ServiceDocument {
    ServiceDocument {
        running,
        port: Some(7381),
        mcp: Some("http://127.0.0.1:7381/mcp".to_owned()),
        vision: Some("http://localhost:7381/GV".to_owned()),
        websocket: Some("ws://localhost:7381/ws".to_owned()),
        gateway: Some("/opt/gg".to_owned()),
        log: Some("/cache/gateway.log".to_owned()),
        clients: 2,
        tools: 27,
    }
}

#[test]
fn a_drafted_message_from_the_palette_can_be_committed() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut effects = Vec::new();
    app.execute_palette(
        PaletteCommand::AiCommitMessage,
        &mut effects,
        Instant::now(),
    );
    let (generation, task) = queued(&effects);
    assert_eq!(task, AiTask::CommitMessage);
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view))
        if view.pending && view.title == "Draft Commit Message"));
    app.receive_ai_answer(
        generation.wrapping_sub(1),
        Ok(answer("commit-message", "stale")),
        Instant::now(),
    );
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view)) if view.pending));
    app.receive_ai_answer(
        generation,
        Ok(answer("commit-message", "feat: retry fetches\n")),
        Instant::now(),
    );
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view))
        if !view.pending && view.commit_message && view.text == "feat: retry fetches\n"));
    app.handle_key(key(KeyCode::Enter), Instant::now());
    assert!(
        matches!(&app.modal, Some(Modal::Commit { input, amend: false })
        if input.value == "feat: retry fetches")
    );
}

#[test]
fn control_g_drafts_into_the_open_commit_editor() {
    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::Commit {
        input: TextBuffer::default(),
        amend: false,
    });
    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
        Instant::now(),
    );
    let (generation, task) = queued(&effects);
    assert_eq!(task, AiTask::CommitMessage);
    assert!(matches!(&app.modal, Some(Modal::Commit { .. })));
    app.receive_ai_answer(
        generation,
        Ok(answer("commit-message", "fix: keep the cursor\n")),
        Instant::now(),
    );
    assert!(matches!(&app.modal, Some(Modal::Commit { input, .. })
        if input.value == "fix: keep the cursor"));
    let effects = app.handle_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
        Instant::now(),
    );
    let (generation, _) = queued(&effects);
    app.receive_ai_answer(
        generation,
        Err("nothing is staged".to_owned()),
        Instant::now(),
    );
    assert!(matches!(&app.modal, Some(Modal::Commit { input, .. })
        if input.value == "fix: keep the cursor"));
    assert!(matches!(&app.toast, Some(toast) if toast.message == "nothing is staged"));
}

#[test]
fn closing_a_pending_answer_drops_it() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut effects = Vec::new();
    app.execute_palette(PaletteCommand::AiReview, &mut effects, Instant::now());
    let (generation, task) = queued(&effects);
    assert_eq!(task, AiTask::Review);
    assert!(
        app.handle_key(key(KeyCode::Enter), Instant::now())
            .is_empty()
    );
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view)) if view.pending));
    app.handle_key(key(KeyCode::Esc), Instant::now());
    assert!(app.modal.is_none());
    app.receive_ai_answer(generation, Ok(answer("review", "late")), Instant::now());
    assert!(app.modal.is_none());
    assert!(app.toast.is_none());
}

#[test]
fn pull_request_actions_need_a_selected_pull_request() {
    let mut app = App::new("/tmp/repo", "repo");
    for command in [
        PaletteCommand::AiExplainCheck,
        PaletteCommand::AiInvestigate,
    ] {
        let mut effects = Vec::new();
        app.execute_palette(command, &mut effects, Instant::now());
        assert!(effects.is_empty());
        assert!(
            matches!(&app.toast, Some(toast) if toast.message == "Select a pull request first")
        );
    }
}

#[test]
fn connecting_agents_reports_each_agent() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut effects = Vec::new();
    app.execute_palette(PaletteCommand::ConnectAgents, &mut effects, Instant::now());
    let Some(AppEffect::Git(command)) = effects.last() else {
        panic!("no worker command was queued");
    };
    let WorkerCommand::ConnectAgents { generation } = command.as_ref() else {
        panic!("another worker command was queued");
    };
    app.receive_agents_connected(
        *generation,
        Ok("Claude Code  connected\n".to_owned()),
        Instant::now(),
    );
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view))
        if !view.pending && view.title == "Connect AI Agents" && view.text.starts_with("Claude Code")));
}

#[test]
fn the_footer_and_gateway_dialog_follow_the_shared_gateway() {
    let mut app = App::new("/tmp/repo", "repo");
    assert_eq!(app.gateway_label(), None);
    let mut effects = Vec::new();
    app.execute_palette(PaletteCommand::GraftGateway, &mut effects, Instant::now());
    assert!(matches!(effects.last(), Some(AppEffect::Git(command))
        if matches!(command.as_ref(), WorkerCommand::EnsureGateway)));
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view))
        if view.text == "Checking the Graftcode gateway…"));
    app.receive_gateway(Ok(Box::new(service(true))));
    assert_eq!(app.gateway_label(), Some("Graftcode ●"));
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view))
        if view.text.contains("Vision     http://localhost:7381/GV")
            && view.text.contains("Clients    2")));
    app.receive_gateway(Err("the Graftcode gateway `gg` is not installed".to_owned()));
    assert_eq!(app.gateway_label(), Some("Graftcode ○"));
    assert!(matches!(&app.modal, Some(Modal::GraftResult(view))
        if view.text.contains("Connect AI Agents to Quinjet")));
}
