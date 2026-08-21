#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn queue_operation(
        &mut self,
        operation: GitOperation,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.busy.is_some() {
            return;
        }
        self.operation_id = self.operation_id.wrapping_add(1);
        self.busy = Some(operation.label().to_owned());
        self.operation_frame = 0;
        match &operation {
            GitOperation::Remove(paths) => {
                self.checked_change_paths
                    .retain(|path| !paths.contains(path));
            }
            GitOperation::Discard(changes) => {
                self.checked_change_paths
                    .retain(|path| !changes.iter().any(|change| &change.path == path));
            }
            _ => {}
        }
        effects.push(AppEffect::Git(Box::new(WorkerCommand::Operate {
            id: self.operation_id,
            operation,
        })));
    }

    pub(super) fn queue_pull_request_operation(
        &mut self,
        pull_request: PullRequest,
        operation: PullRequestOperation,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.busy.is_some() {
            return;
        }
        if let PullRequestOperation::Merge { method, .. } = &operation {
            self.preferred_merge_method = *method;
        }
        self.operation_id = self.operation_id.wrapping_add(1);
        self.busy = Some(operation.label().to_owned());
        self.operation_frame = 0;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::OperatePullRequest {
                id: self.operation_id,
                pull_request: Box::new(pull_request),
                operation,
            },
        )));
    }

    pub(super) fn refresh_loaded_pull_request(&mut self, effects: &mut Vec<AppEffect>) {
        let Some(number) = self.pull_request_exact_number.or_else(|| {
            self.pull_request
                .as_ref()
                .map(|pull_request| pull_request.number)
        }) else {
            return;
        };
        self.request_pull_request_lookup(number, true, true, effects);
    }

    pub(crate) fn pr_primary_action(&self) -> Option<PrPrimaryAction> {
        let pull_request = self.selected_pull_request()?;
        let action = &pull_request.action_state;
        Some(match pull_request.state.as_str() {
            "CLOSED" if action.viewer_can_reopen => PrPrimaryAction::Reopen,
            "MERGED" | "CLOSED" => PrPrimaryAction::OpenInBrowser,
            _ if pull_request.is_draft
                && (action.viewer_can_update || action.viewer_did_author) =>
            {
                PrPrimaryAction::Ready
            }
            _ if !action.merge_queue_entry_id.is_empty() => PrPrimaryAction::Dequeue,
            _ if !action.auto_merge_method.is_empty() => PrPrimaryAction::DisableAutoMerge,
            _ => PrPrimaryAction::Merge(self.preferred_merge_method),
        })
    }

    pub(crate) fn pr_menu_items(&self) -> Vec<PrMenuItem> {
        let Some(pull_request) = self.selected_pull_request() else {
            return Vec::new();
        };
        let action = &pull_request.action_state;
        let mut items = Vec::new();
        match pull_request.state.as_str() {
            "MERGED" => items.push(PrMenuItem::Revert),
            "CLOSED" => {}
            _ if pull_request.is_draft => items.push(PrMenuItem::Stage),
            _ => {
                items.extend(
                    PullRequestMergeMethod::ALL
                        .into_iter()
                        .filter(|method| *method != self.preferred_merge_method)
                        .map(PrMenuItem::Merge),
                );
                if action.auto_merge_method.is_empty() && action.merge_queue_entry_id.is_empty() {
                    items.push(PrMenuItem::AutoMerge);
                } else if !action.auto_merge_method.is_empty() {
                    items.push(PrMenuItem::DisableAutoMerge);
                }
                if !action.merge_queue_entry_id.is_empty() {
                    items.push(PrMenuItem::Dequeue);
                }
                if action.viewer_can_merge_as_admin {
                    items.push(PrMenuItem::AdminMerge);
                }
                if action.viewer_can_update_branch {
                    items.push(PrMenuItem::UpdateBranch);
                }
                if action.viewer_can_update || action.viewer_did_author {
                    items.push(PrMenuItem::Stage);
                }
            }
        }
        if pull_request.state == "OPEN" {
            items.push(PrMenuItem::Review);
        }
        items.push(PrMenuItem::Comments);
        if action.viewer_can_update || action.viewer_did_author {
            items.push(PrMenuItem::Edit);
        }
        items.push(if action.is_locked {
            PrMenuItem::Unlock
        } else {
            PrMenuItem::Lock
        });
        if action.viewer_can_subscribe {
            items.push(if action.viewer_subscription == "SUBSCRIBED" {
                PrMenuItem::Unsubscribe
            } else {
                PrMenuItem::Subscribe
            });
        }
        if action.viewer_did_author && pull_request.is_cross_repository {
            items.push(if action.maintainer_can_modify {
                PrMenuItem::DisallowMaintainerEdits
            } else {
                PrMenuItem::AllowMaintainerEdits
            });
        }
        if pull_request.state == "OPEN" && action.viewer_can_close {
            items.push(PrMenuItem::Close);
        }
        items.push(PrMenuItem::OpenInBrowser);
        items
    }

    pub(super) fn handle_pr_primary(
        &mut self,
        action: PrPrimaryAction,
        effects: &mut Vec<AppEffect>,
    ) {
        match action {
            PrPrimaryAction::OpenInBrowser => self.open_selected_pull_request_in_browser(effects),
            PrPrimaryAction::Merge(method) => {
                self.confirm_pull_request_operation(PullRequestOperation::Merge {
                    method,
                    mode: PullRequestMergeMode::Direct,
                    delete_branch: false,
                });
            }
            PrPrimaryAction::Ready => {
                self.confirm_pull_request_operation(PullRequestOperation::SetDraft(false));
            }
            PrPrimaryAction::Dequeue => {
                self.confirm_pull_request_operation(PullRequestOperation::Dequeue);
            }
            PrPrimaryAction::DisableAutoMerge => {
                self.confirm_pull_request_operation(PullRequestOperation::DisableAutoMerge);
            }
            PrPrimaryAction::Reopen => {
                self.confirm_pull_request_operation(PullRequestOperation::Reopen);
            }
        }
    }

    pub(super) fn handle_pr_menu_item(&mut self, item: PrMenuItem, effects: &mut Vec<AppEffect>) {
        match item {
            PrMenuItem::OpenInBrowser => self.open_selected_pull_request_in_browser(effects),
            PrMenuItem::Merge(method) => {
                self.preferred_merge_method = method;
                self.confirm_pull_request_operation(PullRequestOperation::Merge {
                    method,
                    mode: PullRequestMergeMode::Direct,
                    delete_branch: false,
                });
            }
            PrMenuItem::Stage => {
                if let Some(pull_request) = self.selected_pull_request() {
                    self.confirm_pull_request_operation(PullRequestOperation::SetDraft(
                        !pull_request.is_draft,
                    ));
                }
            }
            PrMenuItem::AutoMerge => self.open_pr_action_picker(
                "Enable Auto-Merge",
                PullRequestMergeMethod::ALL
                    .map(PrActionItem::AutoMerge)
                    .to_vec(),
            ),
            PrMenuItem::DisableAutoMerge => {
                self.confirm_pull_request_operation(PullRequestOperation::DisableAutoMerge);
            }
            PrMenuItem::Dequeue => {
                self.confirm_pull_request_operation(PullRequestOperation::Dequeue);
            }
            PrMenuItem::AdminMerge => self.open_pr_action_picker(
                "Administrator Merge",
                PullRequestMergeMethod::ALL
                    .map(PrActionItem::AdminMerge)
                    .to_vec(),
            ),
            PrMenuItem::Review => self.open_pr_action_picker(
                "Submit Review",
                vec![
                    PrActionItem::Review(PullRequestReviewKind::Approve),
                    PrActionItem::Review(PullRequestReviewKind::Comment),
                    PrActionItem::Review(PullRequestReviewKind::RequestChanges),
                ],
            ),
            PrMenuItem::Comments => {
                let mut items = Vec::new();
                if self
                    .selected_pull_request()
                    .is_some_and(|pull_request| !pull_request.action_state.is_locked)
                {
                    items.push(PrActionItem::Comment(PullRequestCommentMode::Create));
                }
                items.push(PrActionItem::Comment(PullRequestCommentMode::EditLast));
                items.push(PrActionItem::Comment(PullRequestCommentMode::DeleteLast));
                self.open_pr_action_picker("Manage Comments", items);
            }
            PrMenuItem::Edit => self.open_pr_action_picker(
                "Edit Pull Request",
                PullRequestEditField::ALL.map(PrActionItem::Edit).to_vec(),
            ),
            PrMenuItem::UpdateBranch => self.open_pr_action_picker(
                "Update Branch",
                vec![
                    PrActionItem::UpdateBranch(PullRequestUpdateMethod::Merge),
                    PrActionItem::UpdateBranch(PullRequestUpdateMethod::Rebase),
                ],
            ),
            PrMenuItem::Lock => self.open_pr_action_picker(
                "Lock Conversation",
                vec![
                    PrActionItem::Lock(None),
                    PrActionItem::Lock(Some(PullRequestLockReason::OffTopic)),
                    PrActionItem::Lock(Some(PullRequestLockReason::Resolved)),
                    PrActionItem::Lock(Some(PullRequestLockReason::Spam)),
                    PrActionItem::Lock(Some(PullRequestLockReason::TooHeated)),
                ],
            ),
            PrMenuItem::Unlock => {
                self.confirm_pull_request_operation(PullRequestOperation::Unlock);
            }
            PrMenuItem::Subscribe => {
                self.confirm_pull_request_operation(PullRequestOperation::Subscribe(true));
            }
            PrMenuItem::Unsubscribe => {
                self.confirm_pull_request_operation(PullRequestOperation::Subscribe(false));
            }
            PrMenuItem::AllowMaintainerEdits => {
                self.confirm_pull_request_operation(PullRequestOperation::SetMaintainerEdits(true));
            }
            PrMenuItem::DisallowMaintainerEdits => {
                self.confirm_pull_request_operation(PullRequestOperation::SetMaintainerEdits(
                    false,
                ));
            }
            PrMenuItem::Revert => self.open_pr_action_picker(
                "Create Revert Pull Request",
                vec![PrActionItem::Revert(false), PrActionItem::Revert(true)],
            ),
            PrMenuItem::Close => {
                self.confirm_pull_request_operation(PullRequestOperation::Close);
            }
        }
    }

    fn open_pr_action_picker(&mut self, title: &str, items: Vec<PrActionItem>) {
        self.modal = Some(Modal::PullRequestActions {
            title: title.to_owned(),
            items,
            selected: 0,
        });
    }

    pub(super) fn handle_pr_action_item(&mut self, item: PrActionItem) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        if item.needs_input() {
            let initial = match item {
                PrActionItem::Edit(PullRequestEditField::Title) => pull_request.title.clone(),
                PrActionItem::Edit(PullRequestEditField::Body) => pull_request.description.clone(),
                PrActionItem::Edit(PullRequestEditField::Base) => pull_request.base_ref.clone(),
                _ => String::new(),
            };
            self.modal = Some(Modal::Prompt {
                title: item.label().to_owned(),
                input: TextBuffer::new(initial),
                kind: PromptKind::PullRequest {
                    pull_request: Box::new(pull_request),
                    action: item,
                },
            });
        } else {
            self.confirm_pull_request_operation(Self::operation_for_pr_action(item, String::new()));
        }
    }

    pub(super) fn operation_for_pr_action(
        item: PrActionItem,
        value: String,
    ) -> PullRequestOperation {
        match item {
            PrActionItem::AutoMerge(method) => PullRequestOperation::Merge {
                method,
                mode: PullRequestMergeMode::Auto,
                delete_branch: false,
            },
            PrActionItem::AdminMerge(method) => PullRequestOperation::Merge {
                method,
                mode: PullRequestMergeMode::Admin,
                delete_branch: false,
            },
            PrActionItem::Review(kind) => PullRequestOperation::Review { kind, body: value },
            PrActionItem::Comment(mode) => PullRequestOperation::Comment { mode, body: value },
            PrActionItem::Edit(field) => PullRequestOperation::Edit(field.edit(value)),
            PrActionItem::UpdateBranch(method) => PullRequestOperation::UpdateBranch(method),
            PrActionItem::Lock(reason) => PullRequestOperation::Lock(reason),
            PrActionItem::Revert(draft) => PullRequestOperation::Revert {
                draft,
                title: String::new(),
                body: String::new(),
            },
        }
    }

    pub(super) fn confirm_pull_request_operation(&mut self, operation: PullRequestOperation) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        self.modal = Some(Modal::Confirm {
            title: operation.confirm_title(),
            message: operation.confirm_message(&pull_request),
            action: ConfirmAction::PullRequest {
                pull_request: Box::new(pull_request),
                operation,
            },
        });
    }
}
