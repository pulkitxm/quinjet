#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn handle_review_modal_key(
        &mut self,
        mut modal: Modal,
        key: KeyEvent,
    ) -> Vec<AppEffect> {
        if matches!(modal, Modal::PullRequestReviewThreadActions { .. }) {
            return self.handle_review_thread_modal_key(modal, key);
        }
        let mut effects = Vec::new();
        match &mut modal {
            Modal::PullRequestReviewComment { input, target } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                if key.code == KeyCode::Enter
                    && key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    let body = input.value.trim().to_owned();
                    if body.is_empty() {
                        self.modal = Some(modal);
                        return effects;
                    }
                    let operation = match target {
                        PullRequestReviewTarget::Line(anchor) => {
                            PullRequestReviewOperation::AddThread {
                                body,
                                path: anchor.path.clone(),
                                line: Some(anchor.line),
                                side: Some(anchor.side),
                                start_line: None,
                                start_side: None,
                                subject: PullRequestReviewThreadSubject::Line,
                            }
                        }
                        PullRequestReviewTarget::File(path) => {
                            PullRequestReviewOperation::AddThread {
                                body,
                                path: path.clone(),
                                line: None,
                                side: None,
                                start_line: None,
                                start_side: None,
                                subject: PullRequestReviewThreadSubject::File,
                            }
                        }
                        PullRequestReviewTarget::Reply(thread_id) => {
                            PullRequestReviewOperation::Reply {
                                thread_id: thread_id.clone(),
                                body,
                            }
                        }
                        PullRequestReviewTarget::Edit { comment_id } => {
                            PullRequestReviewOperation::UpdateComment {
                                comment_id: comment_id.clone(),
                                body,
                            }
                        }
                    };
                    self.queue_pull_request_review_operation(operation, &mut effects);
                    return effects;
                }
                edit_text(input, key, true);
                self.modal = Some(modal);
            }
            Modal::PullRequestReviewSubmit { input, decision } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                if matches!(key.code, KeyCode::Tab | KeyCode::Right) {
                    let current = PullRequestReviewDecision::ALL
                        .iter()
                        .position(|candidate| candidate == decision)
                        .unwrap_or_default();
                    if let Some(next) = PullRequestReviewDecision::ALL
                        .get((current + 1) % PullRequestReviewDecision::ALL.len())
                    {
                        *decision = *next;
                    }
                    self.modal = Some(modal);
                    return effects;
                }
                if matches!(key.code, KeyCode::BackTab | KeyCode::Left) {
                    let current = PullRequestReviewDecision::ALL
                        .iter()
                        .position(|candidate| candidate == decision)
                        .unwrap_or_default();
                    if let Some(previous) = PullRequestReviewDecision::ALL.get(
                        (current + PullRequestReviewDecision::ALL.len() - 1)
                            % PullRequestReviewDecision::ALL.len(),
                    ) {
                        *decision = *previous;
                    }
                    self.modal = Some(modal);
                    return effects;
                }
                if key.code == KeyCode::Enter
                    && key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    let body = input.value.trim().to_owned();
                    if body.is_empty() {
                        self.modal = Some(modal);
                        return effects;
                    }
                    self.queue_pull_request_review_operation(
                        PullRequestReviewOperation::Submit {
                            body,
                            decision: *decision,
                        },
                        &mut effects,
                    );
                    return effects;
                }
                edit_text(input, key, true);
                self.modal = Some(modal);
            }
            _ => self.modal = Some(modal),
        }
        effects
    }

    fn handle_review_thread_modal_key(
        &mut self,
        mut modal: Modal,
        key: KeyEvent,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        let Modal::PullRequestReviewThreadActions { items, selected } = &mut modal else {
            return effects;
        };
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Up | KeyCode::Char('k') => {
                *selected = previous_list_index(*selected, items.len());
                self.modal = Some(modal);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                *selected = next_list_index(*selected, items.len());
                self.modal = Some(modal);
            }
            KeyCode::Enter => {
                if let Some(item) = items.get(*selected).cloned() {
                    self.handle_review_thread_action(item, &mut effects);
                }
            }
            _ => self.modal = Some(modal),
        }
        effects
    }
}
