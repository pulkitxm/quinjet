#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive action match keeps mouse and keyboard behavior aligned"
    )]
    pub(super) fn handle_modal_action(
        &mut self,
        action: ModalAction,
        effects: &mut Vec<AppEffect>,
        now: Instant,
    ) {
        match action {
            ModalAction::CommitSubmit => {
                let Some(Modal::Commit { input, amend }) = self.modal.take() else {
                    return;
                };
                let message = input.value.trim().to_owned();
                if message.is_empty() {
                    self.modal = Some(Modal::Commit { input, amend });
                    return;
                }
                if amend {
                    self.modal = Some(Modal::Confirm {
                        title: "Amend Commit?".to_owned(),
                        message: "Replace the previous commit with this message?".to_owned(),
                        action: ConfirmAction::Operate(GitOperation::Commit {
                            message,
                            amend: true,
                        }),
                    });
                } else {
                    self.queue_operation(
                        GitOperation::Commit {
                            message,
                            amend: false,
                        },
                        effects,
                    );
                }
            }
            ModalAction::CommitCancel | ModalAction::ConfirmNo => {
                self.modal = None;
            }
            ModalAction::CommitToggleAmend => {
                if let Some(Modal::Commit { amend, .. }) = &mut self.modal {
                    *amend = !*amend;
                }
            }
            ModalAction::ConfirmYes => {
                let Some(Modal::Confirm { action, .. }) = self.modal.take() else {
                    return;
                };
                match action {
                    ConfirmAction::Operate(operation) => {
                        self.queue_operation(operation, effects);
                    }
                    ConfirmAction::OpenPrompt { title, kind } => {
                        self.modal = Some(Modal::Prompt {
                            title,
                            input: TextBuffer::default(),
                            kind,
                        });
                    }
                    ConfirmAction::PullRequest {
                        pull_request,
                        operation,
                    } => {
                        self.queue_pull_request_operation(*pull_request, operation, effects);
                    }
                    ConfirmAction::PullRequestReview(operation) => {
                        self.queue_pull_request_review_operation(operation, effects);
                    }
                }
            }
            ModalAction::SwitchSshMachine(index) => {
                let Some(Modal::Projects { mode, .. }) = self.modal.as_ref() else {
                    return;
                };
                let Some(context) = self.ssh_context.as_ref() else {
                    return;
                };
                if let Some(machine) = context.machines.get(index)
                    && machine.accessible
                    && machine.target != context.current
                {
                    effects.push(AppEffect::SwitchSshMachine(crate::ssh::SshSwitch {
                        index,
                        mode: if *mode == ProjectOpenMode::NewTab {
                            crate::ssh::SshProjectOpenMode::New
                        } else {
                            crate::ssh::SshProjectOpenMode::Current
                        },
                    }));
                }
            }
            ModalAction::PullRequestAction(index) => {
                let Some(Modal::PullRequestActions { items, .. }) = self.modal.take() else {
                    return;
                };
                if let Some(item) = items.get(index).copied() {
                    self.handle_pr_action_item(item);
                }
            }
            ModalAction::PullRequestReviewThreadAction(index) => {
                let Some(Modal::PullRequestReviewThreadActions { items, .. }) = self.modal.take()
                else {
                    return;
                };
                if let Some(item) = items.get(index).cloned() {
                    self.handle_review_thread_action(item, effects, now);
                }
            }
            ModalAction::PullRequestReviewDecision(index) => {
                if let Some(Modal::PullRequestReviewSubmit { decision, .. }) = &mut self.modal
                    && let Some(selected) = PullRequestReviewDecision::ALL.get(index)
                {
                    *decision = *selected;
                }
            }
            ModalAction::ConflictOurs
            | ModalAction::ConflictTheirs
            | ModalAction::ConflictResolved => {
                let Some(Modal::Conflict { change }) = self.modal.take() else {
                    return;
                };
                let operation = match action {
                    ModalAction::ConflictOurs => GitOperation::ResolveConflict {
                        path: change.path,
                        choice: ConflictChoice::Ours,
                    },
                    ModalAction::ConflictTheirs => GitOperation::ResolveConflict {
                        path: change.path,
                        choice: ConflictChoice::Theirs,
                    },
                    ModalAction::ConflictResolved => GitOperation::Stage(vec![change.path]),
                    _ => return,
                };
                self.queue_operation(operation, effects);
            }
        }
    }
}
