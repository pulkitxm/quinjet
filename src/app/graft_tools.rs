#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::graft::call::{CALL_TIMEOUT, GatewayTools, GraftCall, GraftReply};
use crate::graft::client::Tool;
use crate::graft::context::context_key;
use crate::graft::registry::Gateway;

const RESULT_PAGE: usize = 10;

#[derive(Debug, Clone)]
pub(crate) struct GraftRow {
    pub gateway: Gateway,
    pub tool: Tool,
}

impl GraftRow {
    pub(crate) fn label(&self) -> String {
        format!("{} › {}", self.gateway.name, self.tool.name)
    }

    pub(crate) fn fills_everything(&self) -> bool {
        self.tool
            .params
            .iter()
            .all(|param| context_key(&param.name).is_some())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GraftPicker {
    pub rows: Vec<GraftRow>,
    pub selected: usize,
    pub ready: bool,
    pub notes: Vec<String>,
    pub running: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GraftResult {
    pub title: String,
    pub text: String,
    pub is_error: bool,
    pub scroll: usize,
    pub commit_message: bool,
    pub pending: bool,
}

impl GraftResult {
    pub(crate) fn pending(title: &str, text: &str) -> Self {
        Self {
            title: title.to_owned(),
            text: text.to_owned(),
            is_error: false,
            scroll: 0,
            commit_message: false,
            pending: true,
        }
    }

    pub(crate) fn finished(title: &str, text: String, is_error: bool) -> Self {
        Self {
            title: title.to_owned(),
            text,
            is_error,
            scroll: 0,
            commit_message: false,
            pending: false,
        }
    }
}

pub(crate) fn drafts_commit_message(tool: &str) -> bool {
    let folded: String = tool
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect();
    folded.contains("commitmessage")
}

impl App {
    pub(super) fn open_graft_tools(&mut self, effects: &mut Vec<AppEffect>) {
        self.graft_generation = self.graft_generation.wrapping_add(1);
        self.modal = Some(Modal::Graft(GraftPicker::default()));
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadGraftTools {
            generation: self.graft_generation,
        })));
    }

    pub(super) fn receive_graft_tools(
        &mut self,
        generation: u64,
        result: Result<Vec<GatewayTools>, String>,
    ) {
        if generation != self.graft_generation {
            return;
        }
        let Some(Modal::Graft(picker)) = self.modal.as_mut() else {
            return;
        };
        picker.ready = true;
        match result {
            Ok(listed) => {
                let own: Vec<String> = crate::cli::graft_receiver(true)
                    .methods
                    .into_iter()
                    .map(|method| method.name)
                    .collect();
                for gateway in listed {
                    if !gateway.tools.is_empty()
                        && gateway.tools.iter().all(|tool| own.contains(&tool.name))
                    {
                        continue;
                    }
                    if let Some(error) = &gateway.error {
                        picker
                            .notes
                            .push(format!("{} is unavailable: {error}", gateway.gateway));
                    }
                    let source = gateway.source();
                    picker
                        .rows
                        .extend(gateway.tools.into_iter().map(|tool| GraftRow {
                            gateway: source.clone(),
                            tool,
                        }));
                }
                if picker.rows.is_empty() && picker.notes.is_empty() {
                    picker.notes.push(
                        "No other gateways. Quinjet's own actions are in the palette; register another with quinjet graft add <name> <url>."
                            .to_owned(),
                    );
                }
            }
            Err(error) => picker.notes.push(error),
        }
    }

    pub(super) fn handle_graft_key(
        &mut self,
        mut picker: GraftPicker,
        key: KeyEvent,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match key.code {
            KeyCode::Esc => {
                if picker.running.is_some() {
                    self.graft_generation = self.graft_generation.wrapping_add(1);
                }
                return effects;
            }
            KeyCode::Up | KeyCode::Char('k') if picker.running.is_none() => {
                picker.selected = previous_list_index(picker.selected, picker.rows.len());
            }
            KeyCode::Down | KeyCode::Char('j') if picker.running.is_none() => {
                picker.selected = next_list_index(picker.selected, picker.rows.len());
            }
            KeyCode::Home if picker.running.is_none() => picker.selected = 0,
            KeyCode::End if picker.running.is_none() => {
                picker.selected = picker.rows.len().saturating_sub(1);
            }
            KeyCode::Enter if picker.ready && picker.running.is_none() => {
                if let Some(row) = picker.rows.get(picker.selected).cloned() {
                    picker.running = Some(row.label());
                    self.graft_generation = self.graft_generation.wrapping_add(1);
                    effects.push(AppEffect::Git(Box::new(WorkerCommand::CallGraft {
                        generation: self.graft_generation,
                        call: Box::new(self.graft_call(row)),
                    })));
                }
            }
            _ => {}
        }
        self.modal = Some(Modal::Graft(picker));
        effects
    }

    fn graft_call(&self, row: GraftRow) -> GraftCall {
        let check_log = self
            .selected_pull_request_check()
            .zip(self.pull_request_check_log.as_ref())
            .map(|(check, log)| crate::cli::render_check_log(check, log));
        GraftCall {
            gateway: row.gateway,
            tool: row.tool.name,
            arguments: serde_json::Map::new(),
            fill_context: true,
            pull_request: self
                .selected_pull_request()
                .map(|pull_request| pull_request.number),
            check_log,
            timeout: CALL_TIMEOUT,
        }
    }

    pub(super) fn receive_graft_reply(
        &mut self,
        generation: u64,
        result: Result<GraftReply, String>,
        now: Instant,
    ) {
        if generation != self.graft_generation {
            return;
        }
        let title = match self.modal.as_ref() {
            Some(Modal::Graft(picker)) => picker.running.clone().unwrap_or_default(),
            _ => String::new(),
        };
        let view = match result {
            Ok(reply) => GraftResult {
                title: format!("{} › {}", reply.gateway, reply.tool),
                commit_message: !reply.is_error && drafts_commit_message(&reply.tool),
                is_error: reply.is_error,
                text: reply.text,
                scroll: 0,
                pending: false,
            },
            Err(error) => GraftResult::finished(&title, error, true),
        };
        if matches!(self.modal, None | Some(Modal::Graft(_))) {
            self.modal = Some(Modal::GraftResult(view));
        } else {
            let level = if view.is_error {
                ToastLevel::Error
            } else {
                ToastLevel::Success
            };
            self.show_toast(format!("{} finished", view.title), level, now);
        }
    }

    pub(super) fn handle_graft_result_key(
        &mut self,
        mut view: GraftResult,
        key: KeyEvent,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        if view.pending {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                self.cancel_pending_ai();
            } else {
                self.modal = Some(Modal::GraftResult(view));
            }
            return effects;
        }
        match key.code {
            KeyCode::Enter if view.commit_message => {
                self.modal = Some(Modal::Commit {
                    input: TextBuffer::new(view.text.trim().to_owned()),
                    amend: false,
                });
                return effects;
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => return effects,
            KeyCode::Char('c') => effects.push(AppEffect::Copy(view.text.clone())),
            KeyCode::Up | KeyCode::Char('k') => view.scroll = view.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => view.scroll = view.scroll.saturating_add(1),
            KeyCode::PageUp => view.scroll = view.scroll.saturating_sub(RESULT_PAGE),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                view.scroll = view.scroll.saturating_add(RESULT_PAGE);
            }
            KeyCode::Home => view.scroll = 0,
            _ => {}
        }
        self.modal = Some(Modal::GraftResult(view));
        effects
    }
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;
    use crate::graft::client::ToolParam;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn listing(tools: &[(&str, &[&str])]) -> Vec<GatewayTools> {
        vec![GatewayTools {
            gateway: "agent".to_owned(),
            url: "http://localhost:7390/mcp".to_owned(),
            token_env: Some("AGENT_TOKEN".to_owned()),
            tools: tools
                .iter()
                .map(|(name, params)| Tool {
                    name: (*name).to_owned(),
                    title: None,
                    description: Some("Does a thing\nmore".to_owned()),
                    params: params
                        .iter()
                        .map(|param| ToolParam {
                            name: (*param).to_owned(),
                            description: None,
                            required: true,
                        })
                        .collect(),
                })
                .collect(),
            error: None,
        }]
    }

    fn opened() -> (App, u64) {
        let mut app = App::new("/tmp/repo", "repo");
        let mut effects = Vec::new();
        app.execute_palette(PaletteCommand::GraftTools, &mut effects, Instant::now());
        assert!(matches!(effects.as_slice(), [AppEffect::Git(command)]
            if matches!(command.as_ref(), WorkerCommand::LoadGraftTools { .. })));
        let generation = app.graft_generation;
        (app, generation)
    }

    fn reply(tool: &str, text: &str, is_error: bool) -> GraftReply {
        GraftReply {
            gateway: "agent".to_owned(),
            tool: tool.to_owned(),
            filled: vec!["staged_diff".to_owned()],
            empty: Vec::new(),
            text: text.to_owned(),
            is_error,
            structured: None,
        }
    }

    fn run_selected(app: &mut App) -> u64 {
        let effects = app.handle_key(key(KeyCode::Enter), Instant::now());
        let Some(AppEffect::Git(command)) = effects.first() else {
            panic!("Enter did not queue a gateway call");
        };
        let WorkerCommand::CallGraft { generation, call } = command.as_ref() else {
            panic!("Enter queued another worker command");
        };
        assert!(call.fill_context);
        assert_eq!(call.gateway.token_env.as_deref(), Some("AGENT_TOKEN"));
        assert_eq!(call.pull_request, None);
        assert_eq!(call.check_log, None);
        *generation
    }

    #[test]
    fn tools_load_lazily_and_ignore_stale_listings() {
        let (mut app, generation) = opened();
        app.receive_graft_tools(generation.wrapping_sub(1), Ok(listing(&[("stale", &[])])));
        assert!(matches!(&app.modal, Some(Modal::Graft(picker)) if !picker.ready));
        app.receive_graft_tools(
            generation,
            Ok(listing(&[
                ("draft_commit_message", &["staged_diff"]),
                ("echo", &["text"]),
            ])),
        );
        let Some(Modal::Graft(picker)) = &app.modal else {
            panic!("the picker closed");
        };
        assert!(picker.ready);
        assert_eq!(picker.rows.len(), 2);
        assert_eq!(picker.rows[0].label(), "agent › draft_commit_message");
        assert!(picker.rows[0].fills_everything());
        assert!(!picker.rows[1].fills_everything());
    }

    #[test]
    fn an_empty_registry_explains_how_to_add_a_gateway() {
        let (mut app, generation) = opened();
        app.receive_graft_tools(generation, Ok(Vec::new()));
        assert!(matches!(&app.modal, Some(Modal::Graft(picker))
            if picker.notes == ["No other gateways. Quinjet's own actions are in the palette; register another with quinjet graft add <name> <url>."]));
        assert!(
            app.handle_key(key(KeyCode::Enter), Instant::now())
                .is_empty()
        );
    }

    #[test]
    fn a_drafted_commit_message_opens_the_commit_editor() {
        let (mut app, generation) = opened();
        app.receive_graft_tools(
            generation,
            Ok(listing(&[("draft_commit_message", &["staged_diff"])])),
        );
        let call = run_selected(&mut app);
        assert!(matches!(&app.modal, Some(Modal::Graft(picker))
            if picker.running.as_deref() == Some("agent › draft_commit_message")));
        assert!(
            app.handle_key(key(KeyCode::Down), Instant::now())
                .is_empty()
        );
        app.receive_graft_reply(
            call,
            Ok(reply("draft_commit_message", "fix: keep it\n", false)),
            Instant::now(),
        );
        assert!(matches!(&app.modal, Some(Modal::GraftResult(view)) if view.commit_message));
        let copied = app.handle_key(key(KeyCode::Char('c')), Instant::now());
        assert!(matches!(copied.as_slice(), [AppEffect::Copy(text)] if text == "fix: keep it\n"));
        app.handle_key(key(KeyCode::Enter), Instant::now());
        assert!(
            matches!(&app.modal, Some(Modal::Commit { input, amend: false })
            if input.value == "fix: keep it")
        );
    }

    #[test]
    fn closing_while_running_drops_the_late_answer() {
        let (mut app, generation) = opened();
        app.receive_graft_tools(
            generation,
            Ok(listing(&[("explain_check_failure", &["check_log"])])),
        );
        let call = run_selected(&mut app);
        app.handle_key(key(KeyCode::Esc), Instant::now());
        app.receive_graft_reply(
            call,
            Ok(reply("explain_check_failure", "late", false)),
            Instant::now(),
        );
        assert!(app.modal.is_none());
        assert!(app.toast.is_none());
    }

    #[test]
    fn failures_and_other_answers_show_without_a_commit_action() {
        let (mut app, generation) = opened();
        app.receive_graft_tools(generation, Ok(listing(&[("review_diff", &["diff"])])));
        let call = run_selected(&mut app);
        app.receive_graft_reply(
            call,
            Err("the `agent` gateway failed".to_owned()),
            Instant::now(),
        );
        assert!(matches!(&app.modal, Some(Modal::GraftResult(view))
            if view.is_error && !view.commit_message && view.title == "agent › review_diff"));
        app.handle_key(key(KeyCode::PageDown), Instant::now());
        assert!(matches!(&app.modal, Some(Modal::GraftResult(view)) if view.scroll == 10));
        app.handle_key(key(KeyCode::Enter), Instant::now());
        assert!(app.modal.is_none());
    }

    #[test]
    fn answers_arriving_behind_another_dialog_become_a_toast() {
        let (mut app, generation) = opened();
        app.receive_graft_tools(generation, Ok(listing(&[("review_diff", &["diff"])])));
        let call = run_selected(&mut app);
        app.modal = Some(Modal::Commit {
            input: TextBuffer::default(),
            amend: false,
        });
        app.receive_graft_reply(
            call,
            Ok(reply("review_diff", "No findings.", false)),
            Instant::now(),
        );
        assert!(matches!(&app.modal, Some(Modal::Commit { .. })));
        assert!(
            matches!(&app.toast, Some(toast) if toast.message == "agent › review_diff finished")
        );
    }

    #[test]
    fn commit_message_tools_are_recognized_in_any_case_style() {
        assert!(drafts_commit_message("draft_commit_message"));
        assert!(drafts_commit_message("draftCommitMessage"));
        assert!(!drafts_commit_message("review_diff"));
    }
}
