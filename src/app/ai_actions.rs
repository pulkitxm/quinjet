use super::graft_tools::GraftResult;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::cli::ServiceDocument;
use crate::cli::ai::{AiAnswer, AiTask};

const AUTOSTART_ENV: &str = "QUINJET_GRAFT_AUTOSTART";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AiTarget {
    #[default]
    Dialog,
    CommitEditor,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AiState {
    pub generation: u64,
    pub target: AiTarget,
    pub title: String,
    pub gateway: Option<ServiceDocument>,
    pub gateway_error: Option<String>,
}

pub(crate) fn autostart_enabled() -> bool {
    std::env::var(AUTOSTART_ENV).map_or(true, |value| value != "0")
}

impl App {
    pub(super) fn gateway_startup(effects: &mut Vec<AppEffect>) {
        if autostart_enabled() {
            effects.push(AppEffect::Git(Box::new(WorkerCommand::EnsureGateway)));
        }
    }

    pub(crate) fn gateway_label(&self) -> Option<&'static str> {
        if self
            .ai
            .gateway
            .as_ref()
            .is_some_and(|gateway| gateway.running)
        {
            Some("Graftcode ●")
        } else if self.ai.gateway_error.is_some() {
            Some("Graftcode ○")
        } else {
            None
        }
    }

    pub(super) fn run_ai_palette(
        &mut self,
        command: PaletteCommand,
        effects: &mut Vec<AppEffect>,
        now: Instant,
    ) {
        let pull_request = self
            .selected_pull_request()
            .map(|pull_request| pull_request.number);
        let task = match command {
            PaletteCommand::AiCommitMessage => AiTask::CommitMessage,
            PaletteCommand::AiReview => AiTask::Review,
            PaletteCommand::AiInvestigate => match pull_request {
                Some(pull_request) => AiTask::Investigate { pull_request },
                None => {
                    return self.show_toast(
                        "Select a pull request first".to_owned(),
                        ToastLevel::Error,
                        now,
                    );
                }
            },
            PaletteCommand::AiExplainCheck => {
                let log = self
                    .selected_pull_request_check()
                    .zip(self.pull_request_check_log.as_ref())
                    .map(|(check, log)| crate::cli::render_check_log(check, log));
                match (pull_request, log) {
                    (Some(pull_request), Some(log)) => AiTask::ExplainCheck { pull_request, log },
                    (None, _) => {
                        return self.show_toast(
                            "Select a pull request first".to_owned(),
                            ToastLevel::Error,
                            now,
                        );
                    }
                    (Some(_), None) => {
                        return self.show_toast(
                            "Open the failing check's log first".to_owned(),
                            ToastLevel::Error,
                            now,
                        );
                    }
                }
            }
            PaletteCommand::ConnectAgents => {
                self.ai.generation = self.ai.generation.wrapping_add(1);
                self.ai.target = AiTarget::Dialog;
                "Connect AI Agents".clone_into(&mut self.ai.title);
                self.modal = Some(Modal::GraftResult(GraftResult::pending(
                    "Connect AI Agents",
                    "Connecting agents and preparing the Graftcode gateway…",
                )));
                effects.push(AppEffect::Git(Box::new(WorkerCommand::ConnectAgents {
                    generation: self.ai.generation,
                })));
                return;
            }
            _ => {
                self.modal = Some(Modal::GraftResult(GraftResult::finished(
                    "Graftcode Gateway",
                    gateway_text(self.ai.gateway.as_ref(), self.ai.gateway_error.as_deref()),
                    false,
                )));
                effects.push(AppEffect::Git(Box::new(WorkerCommand::EnsureGateway)));
                return;
            }
        };
        self.start_ai(task, AiTarget::Dialog, effects, now);
    }

    pub(super) fn start_ai(
        &mut self,
        task: AiTask,
        target: AiTarget,
        effects: &mut Vec<AppEffect>,
        now: Instant,
    ) {
        self.ai.generation = self.ai.generation.wrapping_add(1);
        self.ai.target = target;
        self.ai.title = task.title();
        match target {
            AiTarget::Dialog => {
                self.modal = Some(Modal::GraftResult(GraftResult::pending(
                    &task.title(),
                    "Asking Claude…",
                )));
            }
            AiTarget::CommitEditor => {
                self.show_toast(
                    "Drafting a commit message with Claude…".to_owned(),
                    ToastLevel::Info,
                    now,
                );
            }
        }
        effects.push(AppEffect::Git(Box::new(WorkerCommand::RunAi {
            generation: self.ai.generation,
            task: Box::new(task),
        })));
    }

    pub(super) fn receive_ai_answer(
        &mut self,
        generation: u64,
        result: Result<AiAnswer, String>,
        now: Instant,
    ) {
        if generation != self.ai.generation {
            return;
        }
        if self.ai.target == AiTarget::CommitEditor {
            match result {
                Ok(answer) => {
                    let message = TextBuffer::new(answer.text.trim().to_owned());
                    match self.modal.as_mut() {
                        Some(Modal::Commit { input, .. }) => *input = message,
                        _ => {
                            self.modal = Some(Modal::Commit {
                                input: message,
                                amend: false,
                            });
                        }
                    }
                    self.show_toast("Drafted with Claude".to_owned(), ToastLevel::Success, now);
                }
                Err(error) => self.show_toast(error, ToastLevel::Error, now),
            }
            return;
        }
        let (text, is_error, commit_message) = match result {
            Ok(answer) => (answer.text, false, answer.task == "commit-message"),
            Err(error) => (error, true, false),
        };
        self.finish_pending(text, is_error, commit_message, now);
    }

    pub(super) fn receive_agents_connected(
        &mut self,
        generation: u64,
        result: Result<String, String>,
        now: Instant,
    ) {
        if generation != self.ai.generation {
            return;
        }
        let (text, is_error) = match result {
            Ok(text) => (text, false),
            Err(error) => (error, true),
        };
        self.finish_pending(text, is_error, false, now);
    }

    pub(super) fn receive_gateway(&mut self, result: Result<Box<ServiceDocument>, String>) {
        match result {
            Ok(document) => {
                self.ai.gateway = Some(*document);
                self.ai.gateway_error = None;
            }
            Err(error) => {
                self.ai.gateway = None;
                self.ai.gateway_error = Some(error);
            }
        }
        if let Some(Modal::GraftResult(view)) = self.modal.as_mut()
            && view.title == "Graftcode Gateway"
        {
            view.text = gateway_text(self.ai.gateway.as_ref(), self.ai.gateway_error.as_deref());
        }
    }

    fn finish_pending(&mut self, text: String, is_error: bool, commit_message: bool, now: Instant) {
        if self.modal.is_none() {
            self.modal = Some(Modal::GraftResult(GraftResult::pending(&self.ai.title, "")));
        }
        let Some(Modal::GraftResult(view)) = self
            .modal
            .as_mut()
            .filter(|modal| matches!(modal, Modal::GraftResult(view) if view.pending))
        else {
            let level = if is_error {
                ToastLevel::Error
            } else {
                ToastLevel::Success
            };
            self.show_toast(format!("{} finished", self.ai.title), level, now);
            return;
        };
        view.text = text;
        view.is_error = is_error;
        view.commit_message = commit_message;
        view.pending = false;
        view.scroll = 0;
    }

    pub(super) const fn cancel_pending_ai(&mut self) {
        self.ai.generation = self.ai.generation.wrapping_add(1);
    }
}

pub(crate) fn gateway_text(gateway: Option<&ServiceDocument>, error: Option<&str>) -> String {
    match (gateway, error) {
        (Some(gateway), _) if gateway.running => {
            let mut lines = vec![
                "The shared Graftcode gateway is running. AI agents reach it through `quinjet mcp`."
                    .to_owned(),
                String::new(),
            ];
            for (label, value) in [
                ("Vision", gateway.vision.as_deref()),
                ("MCP", gateway.mcp.as_deref()),
                ("WebSocket", gateway.websocket.as_deref()),
                ("Log", gateway.log.as_deref()),
            ] {
                if let Some(value) = value {
                    lines.push(format!("{label:<10} {value}"));
                }
            }
            lines.push(format!("{:<10} {}", "Tools", gateway.tools));
            lines.push(format!("{:<10} {}", "Clients", gateway.clients));
            lines.join("\n")
        }
        (_, Some(error)) => format!(
            "The Graftcode gateway is not running.\n\n{error}\n\nRun Connect AI Agents to Quinjet to download it and connect your agents."
        ),
        _ => "Checking the Graftcode gateway…".to_owned(),
    }
}

#[cfg(test)]
#[path = "ai_actions_tests.rs"]
mod tests;
