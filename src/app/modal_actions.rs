#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn handle_modal_action(
        &mut self,
        action: ModalAction,
        effects: &mut Vec<AppEffect>,
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
                    self.handle_review_thread_action(item, effects);
                }
            }
        }
    }
}
